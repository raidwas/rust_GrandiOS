use utils::spinlock;
use core::ptr::{write_volatile, read_volatile};
use core::fmt;
pub use core::fmt::Write;
use alloc::vec;

const DUMM_BASE_ADRESS : u32 = 0xFFFFF200;

//lots of consts for all the register bits
const CR_RSTRX: u32 = 1 << 2;
const CR_RSTTX: u32 = 1 << 3;
const CR_RXEN:  u32 = 1 << 4;
const CR_RXDIS: u32 = 1 << 5;
const CR_TXEN:  u32 = 1 << 6;
const CR_TXDIS: u32 = 1 << 7;
const CR_RSTSTA:u32 = 1 << 8;

const SR_RXRDY:   u32 = 1 << 0;
const SR_TXRDY:   u32 = 1 << 1;
const SR_ENDRX:   u32 = 1 << 3;
const SR_ENDTX:   u32 = 1 << 4;
const SR_OVRE:    u32 = 1 << 5;
const SR_FRAME:   u32 = 1 << 6;
const SR_PARE:    u32 = 1 << 7;
const SR_TXEMPTY: u32 = 1 << 9;
const SR_TXBUFE:  u32 = 1 << 11;
const SR_RXBUFF:  u32 = 1 << 12;
const SR_COMMTX:  u32 = 1 << 30;
const SR_COMMRX:  u32 = 1 << 31;

#[repr(C)]
struct DebugUnitMemoryMap{
	cr:  u32,	//control register
	mr:  u32,	//mode register
	ier: u32,	//interrupt enable reister
	idr: u32,	//interrupt disable register
	imr: u32,	//interrupt mask register
	sr:  u32,	//status register
	rhr: u8,	//receive holding register
	reserved_0: [u8; 3],
	thr: u8,	//transmit holding register
	reserved_1: [u8; 3],
	brgr:u32,	//baud rate generator
	reserved_2: [u32; 3],
	cidr:u32,	//chip id register
	exid:u32,	//chip id extension register
}

pub struct DebugUnit{
	dumm: *mut DebugUnitMemoryMap,
}

unsafe impl Send for DebugUnit { }

impl DebugUnit {
	//Marked unsafe because self.on is only safe assuming the base_adress and pin are correct
	pub const unsafe fn new(base_adress: u32) -> Self{
		DebugUnit{
			dumm: base_adress as *mut DebugUnitMemoryMap,
		}
	}
	fn transmitter_enable(&mut self) {
		unsafe{
		write_volatile(&mut (*(self.dumm)).cr, CR_TXEN);
		}
	}
	fn transmitter_disable(&mut self) {
		unsafe{
		write_volatile(&mut (*(self.dumm)).cr, CR_TXDIS);
		}
	}
	fn transmitter_reset(&mut self) {
		unsafe{
		write_volatile(&mut (*(self.dumm)).cr, CR_RSTTX);
		}
	}
	fn receiver_enable(&mut self) {
		unsafe{
		write_volatile(&mut (*(self.dumm)).cr, CR_RXEN);
		}
	}
	fn receiver_disable(&mut self) {
		unsafe{
		write_volatile(&mut (*(self.dumm)).cr, CR_RXDIS);
		}
	}
	fn receiver_reset(&mut self) {
		unsafe{
		write_volatile(&mut (*(self.dumm)).cr, CR_RSTRX);
		}
	}
    pub fn read(&mut self) -> u8 {
        unsafe{
        while (read_volatile(&mut (*(self.dumm)).sr) & (SR_RXRDY)) == 0 {}
        read_volatile(&mut (*(self.dumm)).rhr)
        }
    }
    pub fn readln(&mut self) -> vec::Vec<u8> {
        // Aktuell kein Support für \r\n line endings.
        unsafe{
        let mut ln = vec!();
        loop {
            let c = self.read();
            ln.push(c);
            if (c as char) == '\r' || (c as char) == '\n' {
                break;
            }
        }
        ln
        }
    }
}

impl fmt::Write for DebugUnit{
	fn write_char(&mut self, c: char) -> fmt::Result {
		unsafe{
		//make sure the last character has been written or moved to the shift register
		while (read_volatile(&mut (*(self.dumm)).sr) & (SR_TXRDY)) == 0 {}
		//write new character
		write_volatile(&mut (*(self.dumm)).thr, c as u8);
		}
		Ok(())
	}
	fn write_str(&mut self, s: &str) -> fmt::Result {
		for c in s.chars(){
			self.write_char(c).unwrap();
		}
		Ok(())
	}
	fn write_fmt(&mut self, args: fmt::Arguments) -> fmt::Result {
		fmt::write(self, args)
	}
}

//We need a wrapper for for DebugUnit to lock it when calling the write and read functions
pub struct DebugUnitWrapper{
    lock: spinlock::Spinlock<DebugUnit>,
}

impl DebugUnitWrapper{
    pub fn enable(& self){
        let mut debug_unit = self.lock.lock();
        debug_unit.transmitter_enable();
	debug_unit.receiver_enable();
    }
    pub fn disable(& self){
        let mut debug_unit = self.lock.lock();
        debug_unit.transmitter_disable();
        debug_unit.receiver_disable();
    }
    pub fn reset(& self){
        let mut debug_unit = self.lock.lock();
        debug_unit.transmitter_reset();
	debug_unit.receiver_reset();
    }
    pub fn read(& self) -> u8 {
        let mut debug_unit = self.lock.lock();
        debug_unit.read()
    }
    pub fn readln(& self) -> vec::Vec<u8> {
        let mut debug_unit = self.lock.lock();
        debug_unit.readln()
    }
    pub fn write_char(& self, c: char) -> fmt::Result {
        let mut debug_unit = self.lock.lock();
		debug_unit.write_char(c)
	}
	pub fn write_str(& self, s: &str) -> fmt::Result {
		let mut debug_unit = self.lock.lock();
		(*debug_unit).write_str(s)
	}
	pub fn write_fmt(& self, args: fmt::Arguments) -> fmt::Result {
		let mut debug_unit = self.lock.lock();
		debug_unit.write_fmt(args)
	}
}

pub static DEBUG_UNIT : DebugUnitWrapper = DebugUnitWrapper{lock: spinlock::Spinlock::new(unsafe { DebugUnit::new(DUMM_BASE_ADRESS) })};

#[allow(unused_macros)]
macro_rules! read {
    () => {{
        DEBUG_UNIT.read()
    }};
}
#[allow(unused_macros)]
macro_rules! readln {
    () => {{
        DEBUG_UNIT.readln()
    }};
}
#[allow(unused_macros)]
macro_rules! print {
	( $x:expr ) => {{
		write!(DEBUG_UNIT, $x).unwrap();
	}};
	( $x:expr, $( $y:expr ),* ) => {{
		write!(DEBUG_UNIT, $x, $($y),*).unwrap();
	}};
}
#[allow(unused_macros)]
macro_rules! println {
	( $x:expr ) => {{
		write!(DEBUG_UNIT, $x).unwrap();
		write!(DEBUG_UNIT, "\n").unwrap();
	}};
	( $x:expr, $( $y:expr ),* ) => {{
		write!(DEBUG_UNIT, $x, $($y),*).unwrap();
		write!(DEBUG_UNIT, "\n").unwrap();
	}};
}
