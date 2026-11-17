//! The chip's peripherals and pins as a single set of ownership tokens.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::gpio::{PB01, PB13, PB14, Pin};
use crate::udma::UdmaCtrl;

/// Every token is zero-sized; moving one into a driver is what grants access.
#[allow(non_snake_case)]
pub struct Peripherals {
    pub UDMA_CTRL: UdmaCtrl,
    pub PB01: PB01,
    pub PB13: PB13,
    pub PB14: PB14,
}

impl Peripherals {
    /// Returns the peripherals the first time it is called, `None` after.
    pub fn take() -> Option<Self> {
        static TAKEN: AtomicBool = AtomicBool::new(false);
        if TAKEN.swap(true, Ordering::Relaxed) {
            None
        } else {
            // SAFETY: the flag guarantees this runs at most once.
            Some(unsafe { Self::steal() })
        }
    }

    /// # Safety
    /// At most one token may exist for each peripheral and pin.
    pub const unsafe fn steal() -> Self {
        // SAFETY: the caller upholds each token's "at most one" contract.
        unsafe {
            Self {
                UDMA_CTRL: UdmaCtrl::steal(),
                PB01: Pin::steal(),
                PB13: Pin::steal(),
                PB14: Pin::steal(),
            }
        }
    }
}
