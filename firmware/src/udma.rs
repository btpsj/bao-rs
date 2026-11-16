//! UDMA: the DMA-backed peripheral group (UARTs, SPI, I2C, ...) and its shared control block.

use core::sync::atomic::{AtomicBool, Ordering};

use derive_mmio::Mmio;

const CTRL_BASE: usize = 0x5010_0000;

/// One bit per peripheral in both `cg` and `rst`.
#[derive(Mmio)]
#[repr(C)]
struct CtrlRegs {
    /// Clock gate: 1 = clock running.
    cg: u32,
    /// Event routing, unused for now.
    cfg_evt: u32,
    /// Reset: 1 = held in reset.
    rst: u32,
}

/// A UDMA peripheral; the discriminant is its bit position in `cg` and `rst`.
#[derive(Clone, Copy)]
pub enum Periph {
    Uart2 = 2,
}

impl Periph {
    const fn mask(self) -> u32 {
        1 << self as u32
    }
}

/// Owner of the UDMA control registers. Zero-sized: the address lives in the type.
pub struct Ctrl {
    regs: OwnedCtrlRegs<CTRL_BASE>,
}

impl Ctrl {
    /// Returns the control block the first time it is called, `None` after.
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
    /// At most one `Ctrl` may exist: two would race on the read-modify-writes below.
    pub const unsafe fn steal() -> Self {
        Self { regs: unsafe { OwnedCtrlRegs::new() } }
    }

    /// Pulses the peripheral's reset line.
    pub fn reset(&mut self, periph: Periph) {
        let mut regs = self.regs.borrow_mut();
        regs.modify_rst(|rst| rst | periph.mask());
        regs.modify_rst(|rst| rst & !periph.mask());
    }

    pub fn enable_clock(&mut self, periph: Periph) {
        self.regs.borrow_mut().modify_cg(|cg| cg | periph.mask());
    }
}
