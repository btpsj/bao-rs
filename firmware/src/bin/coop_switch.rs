#![no_std]
#![no_main]

use core::{arch::naked_asm, cell::UnsafeCell};
use core::{
    fmt::Write,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};

use embedded_hal::delay::DelayNs;
use firmware::{
    peripherals::Peripherals,
    uart::{Config, Uart},
};
use panic_halt as _;
use riscv::delay::McycleDelay;

const FRAME_SIZE: usize = 64;

#[unsafe(naked)]
unsafe extern "C" fn context_switch(curr_a0: *mut usize, next_a1: *mut usize) {
    naked_asm!(
        // Store context for current task
        "addi sp,sp,-{frame_size}",
        "sw ra, 0(sp)",
        "sw s0, 4(sp)",
        "sw s1, 8(sp)",
        "sw s2, 12(sp)",
        "sw s3, 16(sp)",
        "sw s4, 20(sp)",
        "sw s5, 24(sp)",
        "sw s6, 28(sp)",
        "sw s7, 32(sp)",
        "sw s8, 36(sp)",
        "sw s9, 40(sp)",
        "sw s10, 44(sp)",
        "sw s11, 48(sp)",
        // Swap pointers
        "sw sp,(a0)",
        "lw sp,(a1)",
        // Load context for next task
        "lw ra, 0(sp)",
        "lw s0, 4(sp)",
        "lw s1, 8(sp)",
        "lw s2, 12(sp)",
        "lw s3, 16(sp)",
        "lw s4, 20(sp)",
        "lw s5, 24(sp)",
        "lw s6, 28(sp)",
        "lw s7, 32(sp)",
        "lw s8, 36(sp)",
        "lw s9, 40(sp)",
        "lw s10, 44(sp)",
        "lw s11, 48(sp)",
        "addi sp,sp,{frame_size}",
        "ret",
        frame_size = const FRAME_SIZE,
    )
}

const STACK_SIZE: usize = 4096 * 4;

struct Task {
    id: u32,
    running: bool,
    stack: [u8; 4096 * 4],
    sp: usize,
    entry: fn() -> !,
}

static CURRENT: AtomicUsize = AtomicUsize::new(0);

fn yield_now() {
    let curr = CURRENT.load(Relaxed);
    let next = (curr + 1) % 2 as usize;
    CURRENT.store(next, Relaxed);
    unsafe {
        context_switch(&raw mut TASKS[curr].sp, &raw mut TASKS[next].sp);
    }
}

fn task_a() -> ! {
    loop {
        let _ = write!(uart(), "A");
        yield_now();
        McycleDelay::new(350_000_000).delay_ms(1000)
    }
}

fn task_b() -> ! {
    loop {
        let _ = write!(uart(), "B");
        yield_now();
        McycleDelay::new(350_000_000).delay_ms(1000)
    }
}

impl Task {
    pub const fn new(id: u32, entry: fn() -> !) -> Self {
        Task {
            id,
            running: false,
            stack: [0u8; _],
            entry,
            sp: 0,
        }
    }
}

static mut TASKS: [Task; 2] = [Task::new(0, task_a), Task::new(1, task_b)];

struct SharedUart(UnsafeCell<Option<Uart>>);

unsafe impl Sync for SharedUart {}

static UART: SharedUart = SharedUart(UnsafeCell::new(None));

fn uart() -> &'static mut Uart {
    unsafe { UART.0.as_mut_unchecked().as_mut().unwrap() }
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
    unsafe {
        UART.0.replace(Some(uart));
    }

    // Seed task B
    unsafe {
        // RISCV calling convention is that sp must be a multiple of 16
        // "& !15" clears the lower bits to make sure that it is always a multiple of 16
        let top = ((&raw mut TASKS[1].stack) as usize + STACK_SIZE) & !15;
        // top points to the start of the stack but the stack grows downwards
        // addi sp, sp, {negative}, sw {reg}, {positive}(sp)
        let frame = top - FRAME_SIZE;
        (frame as *mut usize).write(task_b as *const () as usize); // ra slot, offset 0
        TASKS[1].sp = frame;
    }

    unsafe {
        (TASKS[0].entry)();
    }
}
