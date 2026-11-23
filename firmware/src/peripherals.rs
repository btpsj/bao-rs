//! The chip's peripherals and pins as a single set of ownership tokens.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::uart::Uart2;
use crate::udma::{Ifram0, UdmaCtrl};
use crate::{
    clock::Clock,
    gpio::{PB01, PB13, PB14, Pin},
    timer0::Timer0,
};

/// Every token is zero-sized; moving one into a driver is what grants access.
#[allow(non_snake_case)]
pub struct Peripherals {
    pub CGU: Clock,
    pub TIMER0: Timer0,
    pub UDMA_CTRL: UdmaCtrl,
    pub IFRAM0: Ifram0,
    pub UART2: Uart2,
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
                CGU: Clock::steal(),
                TIMER0: Timer0::steal(),
                UDMA_CTRL: UdmaCtrl::steal(),
                IFRAM0: Ifram0::steal(),
                UART2: Uart2::steal(),
                PB01: Pin::steal(),
                PB13: Pin::steal(),
                PB14: Pin::steal(),
            }
        }
    }
}
