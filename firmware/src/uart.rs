//! UDMA UART register definitions. Transmit is DMA-only; receive can be polled.

use bitbybit::{bitenum, bitfield};
use derive_mmio::Mmio;

pub const UART2_BASE: usize = 0x5010_3000;

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
