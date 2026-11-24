//! Notes
//!
//! ## `riscv::interrupt::enable` not in `task_start`
//!
//! In this code, we need to call riscv::interrupt::enable in [`task_start`]. This is because:
//! 1) If we got a context switch between enabling global interrupts and seeding B, we'd get an error
//!     Without seeding B, the context is all zeros
//! 2) RISC-V disables interrupts in traps, and interrupts are reenabled without manually touch CSRs by calling mret
//! As a result, we'd context_switch to task B, then never get an interrupt again because interrupts remain disabled
//!
//! ## why notuse mret inside context_switch instead of ret
//!
//! mret jumps to mepc.
//! The riscv-rt provides a _start_trap, which is what we currently use to for all the macro annotated functions that handle traps
//! This function also handles saving all the caller saved registers in the trapframe
//! On trap, riscv-rt runs _start_trap, then calls the annotated function
//! To get back to the _start_trap at the end of the annotated function (so it can restore the registers) is a ret
//! If I used an mret, it would not finish the rest of _start_trap
//! Potential solution is having a custom _start_trap, so all of the Context is saved, but also means we can't use the nice macros that
//!     riscv-rt provides for trap handling

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
    task::{CURRENT, Context, N_TASKS, TASKS, context_switch},
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
/// ~3. Enable global registers for RISC-V~
///     DONT enable global interrupts just yet
fn timer_irq_setup(timer: &mut Timer0<Periodic>) {
    // boot1 leaves event pending so if we don't clear it
    // An interrupt fires immediately after enabling global interrupts
    timer.clear_ev_pending();

    // Enable TIMER0 interrupts
    timer.set_ev_enable(true);
    unsafe {
        // Machine interrupt mask is specific to the VexRiscv / bao1x-chip
        // this is used instead of the CLINT / PLIC which this core doesn't have
        let mut mim = vexriscv::mim::read();
        mim.set_timer0(true);
        vexriscv::mim::write(mim);

        // Machine Interrupts Enable
        mie::set_mext();
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
        for i in 0..N_TASKS {
            TASKS[i].seed();
        }
        let mut boot = Context::new();
        context_switch(&raw mut boot, &raw const TASKS[0].context);
    }
    unreachable!()
}
