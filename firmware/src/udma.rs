//! UDMA: the DMA-backed peripheral group (UARTs, SPI, I2C, ...) and its shared control block.

use core::mem::MaybeUninit;
use core::slice;

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
pub struct UdmaCtrl {
    regs: OwnedCtrlRegs<CTRL_BASE>,
}

impl UdmaCtrl {
    /// # Safety
    /// At most one `UdmaCtrl` may exist: two would race on the read-modify-writes below.
    pub const unsafe fn steal() -> Self {
        Self {
            regs: unsafe { OwnedCtrlRegs::new() },
        }
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

const IFRAM0_BASE: usize = 0x5000_0000;
const IFRAM0_LEN: usize = 128 * 1024;

/// Ownership token for IFRAM0, the RAM that UDMA peripherals transfer from and into.
pub struct Ifram0 {
    _private: (),
}

impl Ifram0 {
    /// # Safety
    /// At most one `Ifram0` may exist, and nothing else may use that memory.
    pub const unsafe fn steal() -> Self {
        Self { _private: () }
    }

    /// The whole region as one buffer of words, to be divided with [`IframBuf::split_at`].
    pub fn into_buf(self) -> IframBuf<u32> {
        // SAFETY: the token is the region's only owner; the region is plain RAM that starts
        // word-aligned and is a whole number of words; `MaybeUninit` is valid for whatever
        // it holds at boot.
        IframBuf(unsafe {
            slice::from_raw_parts_mut(
                IFRAM0_BASE as *mut MaybeUninit<u32>,
                IFRAM0_LEN / size_of::<u32>(),
            )
        })
    }
}

/// An element size a UDMA channel can transfer.
pub trait DmaWord: Copy + 'static {}
impl DmaWord for u8 {}
impl DmaWord for u32 {}

/// A piece of IFRAM holding elements of type `W`, exclusively owned. Only [`Ifram0`] can
/// make one, so holding an `IframBuf` proves the memory is somewhere a UDMA peripheral can
/// reach, aligned for `W`, and a whole number of `W`s long.
pub struct IframBuf<W: DmaWord>(&'static mut [MaybeUninit<W>]);

impl<W: DmaWord> IframBuf<W> {
    /// Splits into the first `len` elements and the rest; `None` if there aren't that many.
    pub fn split_at(self, len: usize) -> Option<(Self, Self)> {
        let (head, tail) = self.0.split_at_mut_checked(len)?;
        Some((IframBuf(head), IframBuf(tail)))
    }

    /// Capacity in elements.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The address to give a peripheral's DMA start-address register.
    /// IFRAM is mapped virtual == physical, so this is simply where the buffer starts.
    pub fn dma_addr(&self) -> u32 {
        self.0.as_ptr() as u32
    }

    /// Copies as much of `src` as fits to the start of the buffer; returns the element count.
    pub fn fill_from(&mut self, src: &[W]) -> usize {
        let count = src.len().min(self.0.len());
        self.0[..count].write_copy_of_slice(&src[..count]);
        count
    }
}

/// The same memory as bytes, for byte-oriented channels such as a UART.
/// There is no way back: bytes can't promise word alignment.
impl From<IframBuf<u32>> for IframBuf<u8> {
    fn from(words: IframBuf<u32>) -> Self {
        let len = words.0.len() * size_of::<u32>();
        // SAFETY: same memory and lifetime, still exclusively owned; bytes have no
        // alignment requirement and `len` covers exactly the words given up.
        IframBuf(unsafe { slice::from_raw_parts_mut(words.0.as_mut_ptr().cast(), len) })
    }
}

/// The whole region, for a program with a single DMA user.
impl From<Ifram0> for IframBuf<u32> {
    fn from(ifram: Ifram0) -> Self {
        ifram.into_buf()
    }
}

/// The whole region as bytes. `Into` doesn't chain, so this hop is spelled out.
impl From<Ifram0> for IframBuf<u8> {
    fn from(ifram: Ifram0) -> Self {
        ifram.into_buf().into()
    }
}
