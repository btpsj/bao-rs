#![no_std]
#![no_main]

use panic_halt as _;

#[riscv_rt::entry]
fn main() -> ! {
    loop {}
}
