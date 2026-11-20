use bitbybit::bitfield;
use derive_mmio::Mmio;
use fugit::{HertzU32, HertzU64, Megahertz};

const CGU_BASE: usize = 0x4004_0000;

#[bitfield(u32)]
struct FreqDivider {
    #[bits(0..=7, rw)]
    normal: u8,
    #[bits(8..=15, rw)]
    low_power: u8,
    #[bits(16..=23, rw)]
    min_cycle: u8,
    #[bits(24..=31, rw)]
    min_cycle_low_power: u8,
}

#[bitfield(u32)]
struct PllSensorOutput {
    #[bits(0..=15, r)]
    pll1: u16,
    #[bits(16..=31, r)]
    pll0: u16,
}

#[bitfield(u32)]
struct ClkTopSrc {
    #[bit(0, r)]
    normal: bool,
    #[bit(1, r)]
    lp: bool,
}

#[bitfield(u32)]
struct ClkSysSrc {
    #[bit(0, r)]
    is_xtal: bool,
}

#[bitfield(u32)]
struct FreqValid {
    #[bits(0..=31, r)]
    valid: u32,
}

impl FreqValid {
    fn is_valid(&self) -> bool {
        self.valid() == 0x0f
    }
}

#[derive(Mmio)]
#[repr(C)]
struct CguRegs {
    cgusec: u32,
    cgulp: u32,
    cgu_seed: u32,
    #[mmio(Write)]
    cgu_seedar: u32,
    cgusel0_clktop_src: ClkTopSrc,
    cgufd_fclk: FreqDivider,
    cgufd_aclk: FreqDivider,
    cgufd_hclk: FreqDivider,
    cgufd_iclk: FreqDivider,
    cgufd_pclk: FreqDivider,
    cgufd_aoclk: FreqDivider,
    #[mmio(Write)]
    cguset: u32,
    cgusel1_clksys_src: ClkSysSrc,
    cgufd_pkeclk: u32,
    cgufd_aoram: u32,
    cgufd_per: FreqDivider,
    #[mmio(PureRead)]
    cgufsfreq0: u32,
    #[mmio(PureRead)]
    cgufsfreq1: u32,
    #[mmio(PureRead)]
    cgufsfreq2: u32,
    #[mmio(PureRead)]
    cgufsfreq3: PllSensorOutput,
    #[mmio(PureRead)]
    cgufsvld: FreqValid,
    cgufscr: u32,
    _reserved0: u32,
    _reserved1: u32,
    cguaclkgr: u32,
    cguhclkgr: u32,
    cguiclkgr: u32,
    cgupclkgr: u32,
    _reserved3: [u32; 20],
    cguowr: u32,
}

const _: () = assert!(core::mem::offset_of!(CguRegs, cgufscr) == 0x54);
const _: () = assert!(core::mem::offset_of!(CguRegs, cguaclkgr) == 0x60);
const _: () = assert!(core::mem::offset_of!(CguRegs, cgufd_per) == 0x3c);
const _: () = assert!(core::mem::offset_of!(CguRegs, cgufsfreq3) == 0x4c);
const _: () = assert!(core::mem::offset_of!(CguRegs, cguowr) == 0xc0);

pub struct Clock {
    regs: OwnedCguRegs<CGU_BASE>,
}

impl Clock {
    /// # Safety
    /// At most one `Clock` may exist
    pub const unsafe fn steal() -> Self {
        Self {
            regs: unsafe { OwnedCguRegs::new() },
        }
    }

    pub fn cgufd_per(&self) -> u32 {
        self.regs.borrow().read_cgufd_per().raw_value
    }

    pub fn clksys(&self) -> Megahertz<u32> {
        let regs = self.regs.borrow();
        let clksys_src = regs.read_cgusel1_clksys_src();
        if clksys_src.is_xtal() {
            Megahertz::<u32>::MHz(48)
        } else {
            Megahertz::<u32>::MHz(32)
        }
    }

    pub fn clktop(&self) -> Megahertz<u32> {
        let regs = self.regs.borrow();
        let clktop_src = regs.read_cgusel0_clktop_src();
        let pll_out = regs.read_cgufsfreq3();
        if clktop_src.normal() {
            Megahertz::<u32>::MHz(pll_out.pll0() as u32)
        } else {
            self.clksys()
        }
    }

    pub fn sensor_valid(&self) -> u32 {
        self.regs.borrow().read_cgufsvld().raw_value
    }

    pub fn perclk_hz(&self) -> HertzU32 {
        let regs = self.regs.borrow();
        let per_fd = regs.read_cgufd_per();
        let fclktop = HertzU64::from_raw(self.clktop().to_Hz() as u64);
        let fd = per_fd.normal() as u32;
        let requested = fclktop * (fd + 1) / 512;
        let ceiling = fclktop / ((per_fd.min_cycle() as u32 + 1) * 2);
        HertzU32::from_raw(requested.min(ceiling).to_Hz() as u32)
    }
}
