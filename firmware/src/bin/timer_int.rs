//! Timer Interrupt testing
//! 
//! Configures the TIMER0 to generate interrupts
#![no_std]
#![no_main]

use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};
use embedded_hal::delay::DelayNs;
use firmware::{
    peripherals::Peripherals,
    uart::{Config, Uart},
    vexriscv,
};
use panic_halt as _;
use riscv::{delay::McycleDelay, register::mie};

const CPU_HZ: u32 = 350_000_000;

static TIMER0_FIRED: AtomicBool = AtomicBool::new(false);

#[riscv_rt::core_interrupt(riscv::interrupt::Interrupt::MachineExternal)]
unsafe fn timer0_irq() {
    let mut p = unsafe { Peripherals::steal() };
    p.TIMER0.clear_ev_pending();
    TIMER0_FIRED.store(true, Relaxed);
}

#[riscv_rt::entry]
fn main() -> ! {
    let mut delay = McycleDelay::new(CPU_HZ);
    let mut p = Peripherals::take().unwrap();
    let mut timer = p.TIMER0.periodic(CPU_HZ);
    timer.set_ev_enable(true);
    unsafe {
        let mut mim = vexriscv::mim::read();
        mim.set_timer0(true);
        vexriscv::mim::write(mim);

        mie::set_mext();
        riscv::interrupt::enable();
    }
    let mut uart = Uart::new(
        p.UART2,
        &mut p.UDMA_CTRL,
        p.PB13,
        p.PB14,
        p.IFRAM0,
        &p.CGU,
        Config::default(),
    );
    loop {
        let _ = writeln!(uart, "TIMER0");
        let _ = writeln!(uart, "load: {:?}", timer.read_load());
        let _ = writeln!(uart, "reload: {:?}", timer.read_reload());
        let _ = writeln!(uart, "enable: {:?}", timer.read_enable());
        let _ = writeln!(uart, "value: {:?}", timer.read_value());
        let _ = writeln!(uart, "ev_enable: {:?}", timer.read_ev_enable());
        let _ = writeln!(uart, "ev_status: {:?}", timer.read_ev_status());
        let _ = writeln!(uart, "ev_pending: {:?}", timer.read_ev_pending());
        let _ = writeln!(uart, "mie: {:?}", mie::read());
        let _ = writeln!(uart, "vexriscv mip: {:?}", vexriscv::mip::read());
        let _ = writeln!(uart, "vexriscv mim: {:?}", vexriscv::mim::read());
        let _ = writeln!(uart, "timer fired: {}", TIMER0_FIRED.swap(false, Relaxed));
        delay.delay_ms(300);
    }
}
