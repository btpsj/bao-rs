#![no_std]
#![no_main]

mod print;

use firmware::{
    peripherals::Peripherals,
    uart::{Config, Uart},
};
use panic_halt as _;
use riscv_rt::core_interrupt;

#[core_interrupt(riscv::interrupt::Interrupt::MachineExternal)]
unsafe fn timer0_irq() {
    let mut p = unsafe { Peripherals::steal() };
    p.TIMER0.clear_ev_pending();

    // TODO context switch
}

struct Context {}

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
    println!("Hello world!");
    loop {}
}
