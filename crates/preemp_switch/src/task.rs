use core::{
    arch::naked_asm,
    mem::offset_of,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

use crate::println;

pub static CURRENT: AtomicUsize = AtomicUsize::new(0);
pub const N_TASKS: usize = 2;
pub static mut TASKS: [Task; N_TASKS] = [Task::new(0, task_a), Task::new(1, task_b)];

const FRAME_SIZE: usize = 64;
const STACK_SIZE: usize = 4096;

#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(curr_a0: *mut Context, next_a1: *const Context) {
    naked_asm!(
        // Store context for current task
        "addi sp,sp,-{frame_size}",
        "sw ra, {ra}(a0)",
        "sw s0, {s0}(a0)",
        "sw s1, {s1}(a0)",
        "sw s2, {s2}(a0)",
        "sw s3, {s3}(a0)",
        "sw s4, {s4}(a0)",
        "sw s5, {s5}(a0)",
        "sw s6, {s6}(a0)",
        "sw s7, {s7}(a0)",
        "sw s8, {s8}(a0)",
        "sw s9, {s9}(a0)",
        "sw s10, {s10}(a0)",
        "sw s11, {s11}(a0)",
        "csrr t0, mepc",
        "sw t0, {mepc}(a0)",
        // Swap pointers
        "sw sp,{sp}(a0)",
        "lw sp,{sp}(a1)",
        // Load context for next task
        "lw ra, {ra}(a1)",
        "lw s0, {s0}(a1)",
        "lw s1, {s1}(a1)",
        "lw s2, {s2}(a1)",
        "lw s3, {s3}(a1)",
        "lw s4, {s4}(a1)",
        "lw s5, {s5}(a1)",
        "lw s6, {s6}(a1)",
        "lw s7, {s7}(a1)",
        "lw s8, {s8}(a1)",
        "lw s9, {s9}(a1)",
        "lw s10, {s10}(a1)",
        "lw s11, {s11}(a1)",
        "lw t0, {mepc}(a1)",
        "csrw mepc, t0",
        "addi sp,sp,{frame_size}",
        "ret",
        frame_size = const FRAME_SIZE,
        sp = const offset_of!(Context, sp),
        ra = const offset_of!(Context, ra),
        s0 = const offset_of!(Context, s0),
        s1 = const offset_of!(Context, s1),
        s2 = const offset_of!(Context, s2),
        s3 = const offset_of!(Context, s3),
        s4 = const offset_of!(Context, s4),
        s5 = const offset_of!(Context, s5),
        s6 = const offset_of!(Context, s6),
        s7 = const offset_of!(Context, s7),
        s8 = const offset_of!(Context, s8),
        s9 = const offset_of!(Context, s9),
        s10 = const offset_of!(Context, s10),
        s11 = const offset_of!(Context, s11),
        mepc = const offset_of!(Context, mepc),
    )
}

#[repr(C)]
pub struct Context {
    sp: usize,
    mepc: usize,
    ra: usize,
    s0: usize,
    s1: usize,
    s2: usize,
    s3: usize,
    s4: usize,
    s5: usize,
    s6: usize,
    s7: usize,
    s8: usize,
    s9: usize,
    s10: usize,
    s11: usize,
}

impl Context {
    const fn new() -> Self {
        Self {
            ra: 0,
            mepc: 0,
            sp: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
        }
    }
}

pub struct Task {
    id: usize,
    stack: [u8; STACK_SIZE],
    pub entry: fn() -> !,
    pub context: Context,
}

impl Task {
    pub const fn new(id: usize, entry: fn() -> !) -> Self {
        Task {
            id,
            stack: [0u8; _],
            entry,
            context: Context::new(),
        }
    }
    pub fn seed(&mut self) {
        // Seed task B
        // unsafe {
        // RISCV calling convention is that sp must be a multiple of 16
        // "& !15" clears the lower bits to make sure that it is always a multiple of 16
        let top = ((&raw mut self.stack) as usize + STACK_SIZE) & !15;
        // top points to the start of the stack but the stack grows downwards
        // addi sp, sp, {negative}, sw {reg}, {positive}(sp)
        let frame = top - FRAME_SIZE;
        // ra slot, offset 0
        unsafe {
            (frame as *mut usize).write(self.entry as *const () as usize);
        }
        self.context.sp = frame;
        self.context.ra = task_start as *const () as usize;
    }
    // }
}

extern "C" fn task_start() -> ! {
    unsafe { riscv::interrupt::enable() };
    let entry = unsafe { TASKS[CURRENT.load(Relaxed)].entry };
    entry()
}

fn task_a() -> ! {
    loop {
        println!("A");
    }
}

fn task_b() -> ! {
    loop {
        println!("B");
    }
}
