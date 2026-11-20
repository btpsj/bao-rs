#[no_std]

use panic_halt as _;
use riscv::delay::McycleDelay;
use core::arch::asm;

unsafe fn ctx_start() {
    asm!(
        "addi sp,sp,-52",
        "sw ra, 0(a0)",
        "sw sp, 4(a0)",
        "sw s0, 8(a0)",
        "sw s1, 12(a0)",
        "sw s2, 16(a0)",
        "sw s3, 20(a0)",
        "sw s4, 24(a0)",
        "sw s5, 28(a0)",
        "sw s6, 32(a0)",
        "sw s7, 36(a0)",
        "sw s8, 40(a0)",
        "sw s9, 44(a0)",
        "sw s10, 48(a0)",
        "sw s11, 52(a0)",
    )
}

#[derive(Debug, Default)]
enum Status {
    #[default]
    Stopped,
    Running,
}

#[derive(Debug, Default)]
struct Thread {
    id: u32,
    status: Status,
    sp: Option<*mut u8>,
    entry: fn(),
}

#[riscv_rt::entry]
fn main() -> ! {
    let current_idx = 0;
    let tcb = [Thread::default(); 32];
    loop {}
}