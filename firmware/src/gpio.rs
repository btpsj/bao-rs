//! Pins through the IOX block: ownership tokens, GPIO output, and alternate-function setup.

use core::convert::Infallible;

use derive_mmio::Mmio;
use embedded_hal::digital::{ErrorType, OutputPin};

const IOX_BASE: usize = 0x5012_f000;

/// Only the port B registers are named; the gaps cover the other ports.
#[derive(Mmio)]
#[repr(C)]
struct IoxRegs {
    _reserved0: [u32; 2],
    /// PB[7:0] alternate function, 2 bits per pin; 0b00 = GPIO.
    afsel_pb_lo: u32,
    /// PB[15:8] alternate function, 2 bits per pin.
    afsel_pb_hi: u32,
    _reserved1: [u32; 73],
    /// PB output level, 1 bit per pin.
    gpio_out_pb: u32,
    _reserved2: [u32; 5],
    /// PB output enable, 1 bit per pin.
    gpio_oe_pb: u32,
    _reserved3: [u32; 44],
    /// 1 hands the pin to the BIO block, overriding `afsel`.
    piosel: u32,
}

// Word offsets from the chip's register map.
const _: () = assert!(core::mem::offset_of!(IoxRegs, afsel_pb_lo) == 2 * 4);
const _: () = assert!(core::mem::offset_of!(IoxRegs, gpio_out_pb) == 77 * 4);
const _: () = assert!(core::mem::offset_of!(IoxRegs, gpio_oe_pb) == 83 * 4);
const _: () = assert!(core::mem::offset_of!(IoxRegs, piosel) == 128 * 4);

// shortcut: every pin read-modify-writes these shared registers with no critical section;
// add one when interrupts start touching pins.
fn iox() -> MmioIoxRegs<'static> {
    // SAFETY: IOX_BASE is the IOX block, and pins only touch their own bits.
    unsafe { IoxRegs::new_mmio_at(IOX_BASE) }
}

fn set_bit(value: u32, bit: u32, on: bool) -> u32 {
    if on { value | bit } else { value & !bit }
}

/// Pin `N` of port `P`, as a zero-sized ownership token. Holding one says nothing about
/// how the pin is configured; hand it to [`Output::new`] or a peripheral driver.
pub struct Pin<const P: char, const N: u8> {
    _private: (),
}

pub type PB01 = Pin<'B', 1>;
pub type PB13 = Pin<'B', 13>;
pub type PB14 = Pin<'B', 14>;

impl<const P: char, const N: u8> Pin<P, N> {
    /// Fails the build for pins this module can't drive yet.
    const BIT: u32 = {
        // Only port B's registers and BIO mapping are known so far.
        assert!(P == 'B');
        assert!(N < 16);
        1 << N
    };

    /// # Safety
    /// At most one token may exist for each pin.
    pub const unsafe fn steal() -> Self {
        Self { _private: () }
    }

    /// Takes the pin back from the BIO and selects function `af` (0 = GPIO, 1 to 3 = alternate).
    fn set_function(&mut self, af: u8) {
        iox().modify_piosel(|sel| sel & !Self::BIT);
        let shift = (N % 8) * 2;
        let update = |afsel: u32| afsel & !(0b11 << shift) | (af as u32 & 0b11) << shift;
        if N < 8 {
            iox().modify_afsel_pb_lo(update);
        } else {
            iox().modify_afsel_pb_hi(update);
        }
    }

    fn write_oe(&mut self, on: bool) {
        iox().modify_gpio_oe_pb(|oe| set_bit(oe, Self::BIT, on));
    }

    fn write_out(&mut self, high: bool) {
        iox().modify_gpio_out_pb(|out| set_bit(out, Self::BIT, high));
    }

    /// For peripheral drivers: routes the pin to alternate function `af`.
    /// `output_enable` is whether the pad drives the line (UART TX) or only listens (UART RX).
    pub(crate) fn set_as_af(&mut self, af: u8, output_enable: bool) {
        self.set_function(af);
        self.write_oe(output_enable);
    }
}

/// A pin configured as a push-pull GPIO output. Zero-sized, like the pin it owns.
pub struct Output<PIN> {
    pin: PIN,
}

impl<const P: char, const N: u8> Output<Pin<P, N>> {
    /// Selects GPIO, drives the pin low, then enables the output.
    pub fn new(mut pin: Pin<P, N>) -> Self {
        pin.set_function(0);
        // Low before OE, so enabling the driver doesn't glitch the pin high.
        pin.write_out(false);
        pin.write_oe(true);
        Self { pin }
    }
}

impl<PIN> ErrorType for Output<PIN> {
    type Error = Infallible;
}

impl<const P: char, const N: u8> OutputPin for Output<Pin<P, N>> {
    fn set_high(&mut self) -> Result<(), Infallible> {
        self.pin.write_out(true);
        Ok(())
    }

    fn set_low(&mut self) -> Result<(), Infallible> {
        self.pin.write_out(false);
        Ok(())
    }
}
