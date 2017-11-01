#![no_std]
#![feature(lang_items)]
#![feature(asm)]
#![feature(naked_functions)]
#![feature(const_fn)]
#![feature(const_unsafe_cell_new)]
//disable some warnings
#![allow(unused_variables)]
#![allow(unused_imports)]
//alloc needs lots of features
#![feature(alloc, global_allocator, allocator_api, heap_api)]
#![feature(compiler_builtins_lib)] 
//Include other parts of the kernal

mod utils{
	pub mod spinlock;
	pub mod allocator;
}
#[macro_use]
mod driver{
	#[macro_use]
	pub mod serial;
	pub mod led;
	
	pub use serial::*;
	pub use led::*;
}
use driver::*;

#[global_allocator]
static GLOBAL: utils::allocator::Allocator = utils::allocator::Allocator::new(0x23000000, 1<<10);
#[macro_use]
extern crate alloc;
extern crate compiler_builtins;
use alloc::boxed::Box;

//#[no_mangle]
//keep the function name so we can call it from assembler
//pub extern
//make the function use the standard C calling convention
#[no_mangle]
#[naked]
pub extern fn _start() {
	let mut led_yellow = unsafe { driver::led::PIO::new(driver::led::PIO_LED_YELLOW) };
	let mut led_red    = unsafe { driver::led::PIO::new(driver::led::PIO_LED_RED)    };
	let mut led_green  = unsafe { driver::led::PIO::new(driver::led::PIO_LED_GREEN)  };
	led_yellow.off();
	led_red.off();
	led_green.off();

    {
        let a = Box::new("Hallo");
        let b = Box::new("Welt!");
        println!("{} at {:p}", a, a);
        println!("{} at {:p}", b, b);
    }
    let a = Box::new("Test");
    println!("{} at {:p}", a, a);

	let lock = utils::spinlock::Spinlock::new(0u32);
	{
		//lock is hold until data goes out of scope
		let mut data = lock.lock();
		*data += 1;
		
		led_yellow.on();
		let mut data2 = lock.try_lock();
		match data2{
			Some(guard) => {
				//we got the lock, but it should have been locked already..............
				led_red.on();},
			None => {
				led_green.on();},
		}
	}
	loop { }
}

// These functions and traits are used by the compiler, but not
// for a bare-bones hello world. These are normally
// provided by libstd.
#[lang = "eh_personality"]
extern fn eh_personality() {}
#[lang = "panic_fmt"]
#[no_mangle]
pub fn panic_fmt() -> ! { loop {} }

// We need this to remove a linking error for the allocator
#[no_mangle]
pub unsafe fn __aeabi_unwind_cpp_pr0() { loop {} }
