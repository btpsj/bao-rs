//! UDMA UART register definitions. Transmit is DMA-only; receive can be polled.

use core::hint::spin_loop;

use bitbybit::{bitenum, bitfield};
use derive_mmio::Mmio;

use crate::gpio::{PB13, PB14};
use crate::udma::{IframBuf, Periph, UdmaCtrl};

const UART2_BASE: usize = 0x5010_3000;

/// The clock the UDMA peripherals run from, which the baud divisor is derived from.
// shortcut: assumed from bao1x-api, not measured; correct it if the baud rate is off on the wire.
const PERCLK_HZ: u32 = 100_000_000;

#[bitenum(u2, exhaustive = true)]
pub enum DataBits {
    Five = 0b00,
    Six = 0b01,
    Seven = 0b10,
    Eight = 0b11,
}

#[bitenum(u1, exhaustive = true)]
pub enum StopBits {
    One = 0,
    Two = 1,
}

/// Line format, enables and baud divisor.
#[bitfield(u32, default = 0)]
struct Setup {
    #[bit(0, rw)]
    parity_en: bool,
    #[bits(1..=2, rw)]
    data_bits: DataBits,
    #[bits(3..=3, rw)]
    stop_bits: StopBits,
    /// Makes received bytes available through `valid`/`data` instead of DMA.
    #[bit(4, rw)]
    rx_polling_en: bool,
    #[bit(5, rw)]
    rx_clean_fifo: bool,
    #[bit(8, rw)]
    en_tx: bool,
    #[bit(9, rw)]
    en_rx: bool,
    /// Peripheral clock / baud rate.
    #[bits(16..=31, rw)]
    div: u16,
}

/// DMA channel control; the same layout serves `rx_cfg` and `tx_cfg`.
#[bitfield(u32, default = 0)]
struct DmaCfg {
    #[bit(0, rw)]
    continuous: bool,
    /// Set to start a transfer; reads back 1 until the transfer completes.
    #[bit(4, rw)]
    en: bool,
    #[bit(6, w)]
    clr: bool,
}

#[bitfield(u32)]
struct Status {
    /// The transmitter is still shifting bytes out.
    #[bit(0, r)]
    tx_busy: bool,
    #[bit(1, r)]
    rx_busy: bool,
}

#[bitfield(u32)]
struct Valid {
    /// `data` holds a received byte.
    #[bit(0, r)]
    rx_data_valid: bool,
}

#[derive(Mmio)]
#[repr(C)]
struct Regs {
    /// DMA receive buffer address (in IFRAM).
    rx_saddr: u32,
    rx_size: u32,
    rx_cfg: DmaCfg,
    _reserved0: u32,
    /// DMA transmit buffer address (in IFRAM).
    tx_saddr: u32,
    tx_size: u32,
    tx_cfg: DmaCfg,
    _reserved1: u32,
    #[mmio(PureRead)]
    status: Status,
    setup: Setup,
    /// Overflow / parity flags; reading clears them.
    #[mmio(Read)]
    error: u32,
    irq_en: u32,
    #[mmio(PureRead)]
    valid: Valid,
    /// Received byte; reading takes it out of the receiver.
    #[mmio(Read)]
    data: u32,
}

// Word offsets from the chip's register map.
const _: () = assert!(core::mem::offset_of!(Regs, tx_cfg) == 6 * 4);
const _: () = assert!(core::mem::offset_of!(Regs, setup) == 9 * 4);
const _: () = assert!(core::mem::offset_of!(Regs, data) == 13 * 4);

/// Ownership token for the UART2 registers.
pub struct Uart2 {
    _private: (),
}

impl Uart2 {
    /// # Safety
    /// At most one `Uart2` may exist.
    pub const unsafe fn steal() -> Self {
        Self { _private: () }
    }
}

/// A pin that can be UART2's receive line.
pub trait RxPin {
    /// Routes the pin to the UART.
    fn setup(&mut self);
}

/// A pin that can be UART2's transmit line.
pub trait TxPin {
    /// Routes the pin to the UART and lets it drive the line.
    fn setup(&mut self);
}

impl RxPin for PB13 {
    fn setup(&mut self) {
        self.set_as_af(1, false);
    }
}

impl TxPin for PB14 {
    fn setup(&mut self) {
        self.set_as_af(1, true);
    }
}

pub struct Config {
    pub baud: u32,
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: bool,
}

impl Default for Config {
    /// 115200 baud, 8 data bits, no parity, 1 stop bit.
    fn default() -> Self {
        Self { baud: 115_200, data_bits: DataBits::Eight, stop_bits: StopBits::One, parity: false }
    }
}

/// UART2: DMA transmit from `tx_buf`, polled receive.
pub struct Uart {
    regs: OwnedRegs<UART2_BASE>,
    tx_buf: IframBuf<u8>,
}

impl Uart {
    /// Resets and clocks the UART, takes over the pins, and applies `config`.
    ///
    /// `tx_buf` is where outgoing bytes are staged for DMA; its size caps one transfer.
    /// It accepts a byte buffer, a word buffer, or the whole `IFRAM0` token.
    pub fn new(
        _uart: Uart2,
        ctrl: &mut UdmaCtrl,
        mut rx: impl RxPin,
        mut tx: impl TxPin,
        tx_buf: impl Into<IframBuf<u8>>,
        config: Config,
    ) -> Self {
        ctrl.reset(Periph::Uart2);
        ctrl.enable_clock(Periph::Uart2);
        rx.setup();
        tx.setup();

        // SAFETY: the `Uart2` token proves nothing else owns these registers.
        let mut owned = unsafe { OwnedRegs::<UART2_BASE>::new() };
        let mut regs = owned.borrow_mut();
        // Rounded to the nearest divisor.
        let div = (PERCLK_HZ + config.baud / 2) / config.baud;
        let setup = Setup::DEFAULT
            .with_parity_en(config.parity)
            .with_data_bits(config.data_bits)
            .with_stop_bits(config.stop_bits)
            .with_rx_polling_en(true)
            .with_en_tx(true)
            .with_en_rx(true)
            .with_div(div as u16);
        regs.write_setup(setup);
        // Pulse the flush bit so stale bytes from before the reset aren't read back.
        regs.write_setup(setup.with_rx_clean_fifo(true));
        regs.write_setup(setup);

        Self { regs: owned, tx_buf: tx_buf.into() }
    }

    /// Reads a byte from the Uart RX buffer. Blocks until a byte is read.
    pub fn read_byte(&mut self) -> u8 {
        let mut regs = self.regs.borrow_mut();
        while !regs.read_valid().rx_data_valid() {
            spin_loop();
        }
        regs.read_data() as u8
    }

    /// Starts sending `bytes` and returns how many were accepted, which is fewer than
    /// `bytes.len()` when they don't fit in the transmit buffer. Returns as soon as the
    /// transfer is started; call again for the rest, or [`flush`](Self::flush) to wait.
    pub fn write(&mut self, bytes: &[u8]) -> usize {
        let mut regs = self.regs.borrow_mut();
        // The previous transfer is still reading the buffer; let it finish first.
        while regs.read_tx_cfg().en() {
            spin_loop();
        }
        let count = self.tx_buf.fill_from(bytes);
        if count == 0 {
            // Nothing to send; a zero-length transfer is not known to be safe to start.
            return 0;
        }
        regs.write_tx_saddr(self.tx_buf.dma_addr());
        regs.write_tx_size(count as u32);
        regs.write_tx_cfg(DmaCfg::DEFAULT.with_en(true));
        count
    }

    /// Waits until everything written has left the wire.
    pub fn flush(&mut self) {
        let regs = self.regs.borrow();
        // First the DMA handing bytes to the UART, then the UART shifting the last one out.
        while regs.read_tx_cfg().en() {
            spin_loop();
        }
        while regs.read_status().tx_busy() {
            spin_loop();
        }
    }
}
