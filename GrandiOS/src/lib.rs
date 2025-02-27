#![no_std]
#![feature(lang_items)]
#![feature(asm)]
#![feature(naked_functions)]
#![feature(const_fn)]
#![feature(const_unsafe_cell_new)]
#![feature(range_contains)]
#![feature(slice_concat_ext)]
//disable some warnings
#![allow(unused_variables)]
//#![allow(unused_imports)]
#![allow(unused_unsafe)]
#![allow(unused_mut)]
#![allow(dead_code)]
//alloc needs lots of features
#![feature(alloc, global_allocator, allocator_api, heap_api)]
#![feature(compiler_builtins_lib)]
//Include other parts of the kernal

#[macro_use]
mod driver{
    #[macro_use]
    pub mod serial;
    pub mod led;
    pub mod memory_controller;
    pub mod rtc;
    pub mod system_timer;
    pub mod interrupts;

    pub use serial::*;
    pub use led::*;
    pub use memory_controller::*;
    pub use interrupts::*;
}
mod utils{
    pub mod spinlock;
    pub mod allocator;
    pub mod exceptions{
        #[macro_use]
        pub mod common_code;
        pub mod data_abort;
        pub mod undefined_instruction;
        pub mod prefetch_abort;
        pub mod software_interrupt;
        pub mod irq;
    }
    pub mod thread;
    pub mod scheduler;
    pub mod registers;
    pub mod ring;
    pub extern crate vt;
}
use driver::*;
use alloc::string::ToString;
use core::fmt;

#[global_allocator]
pub static GLOBAL: utils::allocator::Allocator = utils::allocator::Allocator::new(0x22000000, 0x23ffffff);
#[macro_use]
extern crate alloc;
extern crate serde;
#[allow(unused_imports)]
#[macro_use]
extern crate serde_derive;
extern crate corepack;
extern crate compiler_builtins;
extern crate rlibc;
#[macro_use]
extern crate swi;

extern {
    fn _shell_start();
}

//#[no_mangle]
//keep the function name so we can call it from assembler
//pub extern
//make the function use the standard C calling convention
#[no_mangle]
#[naked]
pub extern fn _start() {
    init_stacks();
    main();//call another function to make sure rust correctly does its stack stuff
    loop{};
}
#[inline(never)]
fn main(){
    //make interupt table writable
    let mut mc = unsafe { MemoryController::new(MC_BASE_ADRESS) } ;
    mc.remap();

    //Initialise the DebugUnit
    DEBUG_UNIT.reset();
    DEBUG_UNIT.enable();
    
    println!("Waiting for keypress before continue");
    //read!();

    //Initialisieren der Ausnahmen
    println!("Initialisiere Ausnahmen");
    let mut ic = unsafe { InterruptController::new(IT_BASE_ADDRESS, AIC_BASE_ADDRESS) } ;
    utils::exceptions::software_interrupt::init(&mut ic);
    utils::exceptions::data_abort::init(&mut ic);
    utils::exceptions::undefined_instruction::init(&mut ic);
    utils::exceptions::prefetch_abort::init(&mut ic);

    //Initialisieren der Interrupts
    println!("Initialisiere Interrupts");
    utils::exceptions::irq::init(&mut ic);
    DEBUG_UNIT.interrupt_set_rxrdy(true);
    let mut st = unsafe{ driver::system_timer::init(); driver::system_timer::get_system_timer()};
    st.set_piv(0x0800); //0x8000 ist eine sekunde, da die slowclock 0x8000 Hz hat.
    st.interrupt_enable_pits();

    //Initialisieren des Schedulers
    println!("Initialisiere Scheduler");
    println!("Adresse der switch routine ist: 0x{:x}, {}", swi::switch::call as *const () as u32, swi::switch::call as *const () as u32);
    
    //We assume that the idle thread is always the first one created!
    let mut tcb_idle = utils::thread::TCB::new("Idle Thread".to_string(), utils::scheduler::idle as *const () , 0x0, 0);
    tcb_idle.set_priority(0);
    //Anlegen des Schedulers
    unsafe{ utils::scheduler::init(tcb_idle) };
    let mut sched = unsafe {utils::scheduler::get_scheduler()};
    
    //Add shell
    let mut tcb_shell = utils::thread::TCB::new("Shell Thread".to_string(), _shell_start as *const (), 0x40000, utils::registers::CPSR_MODE_USER | utils::registers::CPSR_IMPRECISE_ABORT); //function, memory, and cpsr will be set when calling the switch interrupt
    tcb_shell.set_priority(10);
    sched.add_thread(tcb_shell);
    //Add small thread that 
    //let mut tcb_sleep = utils::thread::TCB::new("Sleep Thread".to_string(), p_sleep as *const (), 0x800, utils::registers::CPSR_MODE_USER | utils::registers::CPSR_IMPRECISE_ABORT);
    //tcb_sleep.set_priority(15);
    //sched.add_thread(tcb_sleep);

    //switch into user mode before starting the shell + enable interrupts, from this moment on the entire os stuff that needs privileges is done from syscalls (which might start privileged threads)
    unsafe{asm!("
        msr CPSR, r0"
        :
        :"{r0}"(utils::registers::CPSR_MODE_SYS | utils::registers::CPSR_IMPRECISE_ABORT) //interrupts are enabled if the bits are 0
        :"memory"
        :"volatile"
    );}
    //call switch before entering the idle thread to swich to the scheduler.
    let input      = swi::switch::Input{};
    let mut output = swi::switch::Output{};
    swi::switch::call(& input, &mut output);
    utils::scheduler::idle();
}

/*
extern fn p_sleep(){
    for i in 0..10 {
        let input      = swi::sleep::Input{t: 1000};
        let mut output = swi::sleep::Output{};
        swi::sleep::call(& input, &mut output);
        let input      = swi::write::Input{c: 'p' as u8};
        let mut output = swi::write::Output{};
        swi::write::call(& input, &mut output);
    }
    let input = swi::exit::Input{};
    let mut output = swi::exit::Output{};
    swi::exit::call(& input, &mut output);
}
*/

#[inline(always)]
#[naked]
fn init_stacks(){
    //initialise the stack pointers for all modes.
    //each stack gets around 2kbyte, except the fiq which has a bit less (vector table+ jump addresses) and the system/user stack which has (16-2*6 = 4)kbyte
    unsafe{asm!("
        mov     r2, #0x200000
        mrs     r0, CPSR	//auslaesen vom status register
        bic     r0, r0, #0x1F	//set all mode bits to zero
        orr     r1, r0, #0x11	//ARM_MODE_FIQ
        msr     CPSR, r1
        add     r2, #0x2000
        mov     sp, r2		//set stack pointer for fiq mode
        orr     r1, r0, #0x12	//ARM_MODE_IRQ
        msr     CPSR, r1
        add     r2, #0x0
        mov     sp, r2		//set stack pointer for irq mode
        orr     r1, r0, #0x13	//ARM_MODE_ABORT
        msr     CPSR, r1
        add     r2, #0x0
        mov     sp, r2		//set stack pointer for abort mode
        orr     r1, r0, #0x17	//ARM_MODE_supervisor
        msr     CPSR, r1
        add     r2, #0x0
        mov     sp, r2		//set stack pointer for supervisor mode
        orr     r1, r0, #0x1B	//ARM_MODE_UNDEFINED
        msr     CPSR, r1
        add     r2, #0x0
        mov     sp, r2		//set stack pointer for undefined mode
        orr     r1, r0, #0x1F	//ARM_MODE_SYS
        msr     CPSR, r1
        add     r2, #0x2000
        mov     sp, r2		//set stack pointer for system/user mode
        "
        :
        :
        :"memory"
        :"volatile"
    )}
}

// These functions and traits are used by the compiler, but not
// for a bare-bones hello world. These are normally
// provided by libstd.
#[lang = "eh_personality"]
extern fn eh_personality() {}
#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(msg: core::fmt::Arguments, file: &'static str, line: u32) -> ! {
    println!("Unhandled panic in {}/{} on line {}{}{}{}:\n{}!!!{} {}{}", env!("CARGO_PKG_NAME"), file, &utils::vt::CF_BLUE, &utils::vt::ATT_BRIGHT ,line, &utils::vt::CF_STANDARD, &utils::vt::CF_RED, &utils::vt::CF_STANDARD,msg, &utils::vt::ATT_RESET);
    loop {}
}
// We need this to remove a linking error for the allocator
#[no_mangle]
pub unsafe fn __aeabi_unwind_cpp_pr0() { loop {} }
