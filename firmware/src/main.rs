#![no_std]
#![no_main]

mod pb01;

use embedded_hal::digital::OutputPin;
use panic_halt as _;

#[riscv_rt::entry]
fn main() -> ! {
    let mut pin = pb01::PB01::new();
    pin.set_high().unwrap();
    loop {}
}
