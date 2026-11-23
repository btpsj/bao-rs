#![no_std]
#![no_main]

mod print;
mod task;

use core::sync::atomic::Ordering::Relaxed;

use firmware::{
    peripherals::Peripherals,
    timer0::{Periodic, Timer0},
    uart::{Config, Uart},
    vexriscv,
};
use panic_halt as _;
use riscv::register::mie;
use riscv_rt::core_interrupt;

use crate::{
    print::UART,
    task::{CURRENT, N_TASKS, TASKS, context_switch},
};

const CPU_HZ: u32 = 350_000_000;

#[core_interrupt(riscv::interrupt::Interrupt::MachineExternal)]
unsafe fn timer0_irq() {
    let mut p = unsafe { Peripherals::steal() };
    p.TIMER0.clear_ev_pending();

    let curr = CURRENT.load(Relaxed);
    let next = (curr + 1) % N_TASKS;
    CURRENT.store(next, Relaxed);
    unsafe {
        context_switch(&raw mut TASKS[curr].context, &raw mut TASKS[next].context);
    }
}

/// Setup interrupts for TIMER0
///
/// 0. Enable TIMER0 interrupt registers
/// 1. Enable the TIMER0 interrupt line on VexRiscv
/// 2. Enable machine external interrupts with RISC-V mie register
/// 3. Enable global registers for RISC-V
fn timer_irq_setup(timer: &mut Timer0<Periodic>) {
    timer.set_ev_enable(true);
    unsafe {
        let mut mim = vexriscv::mim::read();
        mim.set_timer0(true);
        vexriscv::mim::write(mim);

        mie::set_mext();
        riscv::interrupt::enable();
    }
}

#[riscv_rt::entry]
fn main() -> ! {
    let mut p = Peripherals::take().unwrap();
    let uart = Uart::new(
        p.UART2,
        &mut p.UDMA_CTRL,
        p.PB13,
        p.PB14,
        p.IFRAM0,
        &p.CGU,
        Config {
            baud: 1_000_000,
            ..Default::default()
        },
    );
    UART.init(uart);
    let mut timer = p.TIMER0.periodic(CPU_HZ);
    timer_irq_setup(&mut timer);

    println!("Hello world!");
    unsafe {
        TASKS[1].seed();
        (TASKS[0].entry)();
    }
}
