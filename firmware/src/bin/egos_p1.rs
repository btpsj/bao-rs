#![no_std]
#![no_main]

use core::arch::{asm, naked_asm};
use panic_halt as _;
use riscv::delay::McycleDelay;

const STACK_SIZE: usize = 16_000;
static mut STACK_POOL: [[u8; STACK_SIZE]; 2] = [[0u8; STACK_SIZE]; 2];

#[unsafe(naked)]
unsafe extern "C" fn ctx_start(curr: u32, sp: u32) {
    naked_asm!(
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
        "sw sp,0(a0)",
        "mv sp,a1",
    )
}

#[derive(Debug, Default, Copy, Clone)]
enum Status {
    #[default]
    Stopped,
    Running,
}

#[derive(Debug)]
struct Thread {
    id: u32,
    status: Status,
    sp: *mut u8,
    entry: fn(usize),
}

impl Thread {
    pub const fn new(id: u32) -> Self {
        Self {
            id,
            entry: child,
            sp: unsafe { STACK_POOL[id as usize].as_mut_ptr() },
            status: Status::Stopped,
        }
    }
}

fn child(id: usize) {}

#[riscv_rt::entry]
fn main() -> ! {
    let current_idx = 0;
    let tcb = [Thread::new(0), Thread::new(1)];
    loop {}
}
