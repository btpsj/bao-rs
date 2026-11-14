//! PB01 (Dabao header pin 29) as a push-pull GPIO output, through the IOX block.

use core::convert::Infallible;

use embedded_hal::digital::{ErrorType, OutputPin};
use utralib::utra::iox::{HW_IOX_BASE, SFR_AFSEL_CRAFSEL2, SFR_GPIOOE_CRGOE1, SFR_GPIOOUT_CRGO1, SFR_PIOSEL};
use utralib::{CSR, Field};

// CR = Control Register
// GO = GPIO out
// GOE = GPIO Output Enable

/// PB[7:0] alternate function, 2 bits per pin; 0b00 = GPIO.
const PB01_AFSEL: Field = Field::new(2, 2, SFR_AFSEL_CRAFSEL2);
/// GPIO Output Control Register Bit for PB01
const PB01_OUT: Field = Field::new(1, 1, SFR_GPIOOUT_CRGO1);
/// GPIO Output Enable Control Register Bit for PB01
const PB01_OE: Field = Field::new(1, 1, SFR_GPIOOE_CRGOE1);
/// BIO Function Selection Register Bit for PB01
/// Seems to be named in utralib due to a legacy implementation
/// 1 bit; setting it 1 will override AFSEL AF1 so must be set to 0
const PB01_BIOSEL: Field = Field::new(1, 1, SFR_PIOSEL);

const AFSEL_GPIO: u32 = 0b00;

pub struct PB01 {
    iox: CSR<u32>,
}

impl PB01 {
    /// Takes PB01 back from the BIO, selects GPIO, drives it low, then enables the output.
    pub fn new() -> Self {
        let mut iox = CSR::new(HW_IOX_BASE as *mut u32);
        iox.rmwf(PB01_BIOSEL, 0);
        iox.rmwf(PB01_AFSEL, AFSEL_GPIO);
        // Low before OE, so enabling the driver doesn't glitch the pin high.
        iox.rmwf(PB01_OUT, 0);
        iox.rmwf(PB01_OE, 1);
        Self { iox }
    }
}

impl ErrorType for PB01 {
    type Error = Infallible;
}

impl OutputPin for PB01 {
    fn set_high(&mut self) -> Result<(), Infallible> {
        self.iox.rmwf(PB01_OUT, 1);
        Ok(())
    }

    fn set_low(&mut self) -> Result<(), Infallible> {
        self.iox.rmwf(PB01_OUT, 0);
        Ok(())
    }
}
