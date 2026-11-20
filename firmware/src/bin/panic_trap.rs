#![no_std]
#![no_main]

use core::fmt::Write;
use firmware::{
    peripherals::Peripherals,
    uart::{Config, Uart},
};
use panic_halt as _;
use riscv::{
    register::{mcause, mepc, mtval},
};

#[riscv_rt::exception(riscv::interrupt::Exception::IllegalInstruction)]
fn illegal_instruction(_trap_frame: &mut riscv_rt::TrapFrame) -> ! {
    let mut p = unsafe { Peripherals::steal() };
    let mut uart = Uart::new(
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
    let _ = write!(uart, "Hello world!\r\n");
    let _ = write!(uart, "{:?}\r\n", mcause::read());
    let _ = write!(uart, "{:#010x}\r\n", mepc::read());
    let _ = write!(uart, "{:#010x}\r\n", mtval::read());
    loop {}
}

#[riscv_rt::entry]
fn main() -> ! {
    unsafe { core::arch::asm!("unimp") };
    loop {}
}
