use core::marker::PhantomData;

use bitbybit::bitfield;
use derive_mmio::Mmio;

const TIMER0_IRQ: u32 = 30;
const TIMER0_BASE: usize = 0xe001_c000;

#[bitfield(u32)]
struct Enable {
    #[bit(0, rw)]
    en: bool,
}

#[bitfield(u32)]
struct UpdateValue {
    #[bit(0, rw)]
    update_value: bool,
}

#[bitfield(u32)]
struct EventStatus {
    #[bit(0, r)]
    zero_lvl: bool,
}

#[bitfield(u32)]
struct EventPending {
    #[bit(0, r)]
    zero_occured: bool,
}

#[bitfield(u32)]
struct EventEnable {
    #[bit(0, rw)]
    enable_zero_event: bool,
}

#[derive(Mmio)]
#[repr(C)]
struct TimerRegs {
    load: u32,
    reload: u32,
    enable: Enable,
    update_value: UpdateValue,
    value: u32,
    event_status: EventStatus,
    event_pending: EventPending,
    event_enable: EventEnable,
}

#[derive(Debug)]
pub struct Polled;
#[derive(Debug)]
pub struct Periodic;
#[derive(Debug)]
pub struct OneShot;

#[derive(Debug)]
pub struct Timer0<MODE = Polled> {
    regs: OwnedTimerRegs<TIMER0_BASE>,
    mode: PhantomData<MODE>,
}

impl Timer0<Polled> {
    pub const unsafe fn steal() -> Self {
        Self {
            regs: unsafe { OwnedTimerRegs::new() },
            mode: PhantomData,
        }
    }

    pub fn periodic(mut self, n_cycles: u32) -> Timer0<Periodic> {
        let mut regs = self.regs.borrow_mut();
        // Disable timer
        regs.write_enable(Enable { raw_value: 0 });
        // Set load register to 0
        regs.write_load(0);
        // Set reload register to the expected duration in clock cycles
        regs.write_reload(n_cycles);
        regs.write_enable(Enable { raw_value: 1 });
        Timer0 {
            regs: self.regs,
            mode: PhantomData,
        }
    }

    pub fn one_shot(mut self, n_cycles: u32) -> Timer0<OneShot> {
        let mut regs = self.regs.borrow_mut();
        // Disable timer
        regs.write_enable(Enable { raw_value: 0 });
        // Set load register to the expected duration in clock cycles
        regs.write_load(n_cycles);
        regs.write_reload(0);
        regs.write_enable(Enable { raw_value: 1 });
        Timer0 {
            regs: self.regs,
            mode: PhantomData,
        }
    }
}

impl<MODE> Timer0<MODE> {
    fn write_update_value(&mut self) {
        self.regs
            .borrow_mut()
            .write_update_value(UpdateValue { raw_value: 1 });
    }
    pub fn read_load(&self) -> u32 {
        self.regs.borrow().read_load()
    }

    pub fn read_reload(&self) -> u32 {
        self.regs.borrow().read_reload()
    }

    pub fn read_value(&mut self) -> u32 {
        self.write_update_value();
        self.regs.borrow().read_value()
    }

    pub fn read_enable(&self) -> bool {
        self.regs.borrow().read_enable().en()
    }

    pub fn disable(&mut self) {
        self.regs.borrow_mut().write_enable(Enable { raw_value: 0 });
    }

    pub fn read_ev_status(&self) -> bool {
        self.regs.borrow().read_event_status().zero_lvl()
    }

    pub fn read_ev_pending(&self) -> bool {
        self.regs.borrow().read_event_pending().zero_occured()
    }

    pub fn read_ev_enable(&self) -> bool {
        self.regs.borrow().read_event_enable().enable_zero_event()
    }

    pub fn set_ev_enable(&mut self, zero: bool) {
        self.regs.borrow_mut().write_event_enable(EventEnable {
            raw_value: zero as u32,
        });
    }

    pub fn clear_ev_pending(&mut self) {
        self.regs
            .borrow_mut()
            .write_event_pending(EventPending { raw_value: 1 });
    }
}
