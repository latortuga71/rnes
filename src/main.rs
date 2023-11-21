
use std::collections::{HashMap};
use std::fs;
use std::num::Wrapping;
use std::ops::{Add, Shl,Sub};
use std::string::ToString;
use crate::Mode::*;
use crate::Operation::*;
use lazy_static::lazy_static;

/* Memory Layout for NES
    0x0
    -- SYSTEM RAM ZERO PAGE
    0x800
    --- RAM MIRRORS
    0x2000
    -- PPU PORTS
    0x4000
    -- APU PORTS IO REGISTERS
    0x4020
    -- CARTRIDGE WRAM
    0x8000
    -- PRG-ROM
    0xFFFA
    --- Vectors
    0xFFFF
*/

// LOOK UP TABLE FOR OPCODES
lazy_static! {static ref INSTRUCTION_TABLE:HashMap<u8,Instruction> = HashMap::from([
        //////////////////////////////////
        // FLAG INSTRUCTIONS
        // RTI
        (0x40,Instruction{address_mode:Implied,operation:RTI,cycles:6}),
        //SEI
        (0x78,Instruction{address_mode:Implied,operation:SEI,cycles:2}),
        // CLD
        (0xD8,Instruction{address_mode:Implied,operation:CLD,cycles:2}),
        // BRK
        (0x00,Instruction{address_mode:Implied,operation:BRK,cycles:7}),
        /////////////////////////////////
        // Load X Register
        (0xA2,Instruction{address_mode:Immediate,operation:LDX,cycles:2}),
        // Load A Register
        (0xA9,Instruction{address_mode:Immediate,operation:LDA,cycles:2}),
        // Store Accumulator
        (0x95,Instruction{address_mode:ZeroPageX,operation:STA,cycles:4}),
        ///////////////////////////
        /// Register Instructions
        /// Decrement X
        (0xCA,Instruction{address_mode:Implied,operation:DEX,cycles:2}),
        // INCREMENT X
        (0xE8,Instruction{address_mode:Implied,operation:INX,cycles:2}),

        ///////////////////////////////////
        // Stack Instructions
        // Transfer X to Stack Ptr
        (0x9A,Instruction{address_mode:Implied,operation:TXS,cycles:2}),
        /////////////// BRANCH INSTRUCTIONS
        // BNE
        (0xD0,Instruction{address_mode:Relative,operation:BNE,cycles:2}),


        // Add With Carry
        (0x69,Instruction{address_mode:Immediate,operation:ADC,cycles:2}),
        (0x65,Instruction{address_mode:ZeroPage,operation:ADC,cycles:3}),
        (0x75,Instruction{address_mode:ZeroPageX,operation:ADC,cycles:4}),
        (0x6D,Instruction{address_mode:Absolute,operation:ADC,cycles:4}),
        (0x7D,Instruction{address_mode:AbsoluteX,operation:ADC,cycles:4}),
        (0x79,Instruction{address_mode:AbsoluteY,operation:ADC,cycles:4}),
        (0x61,Instruction{address_mode:IndirectX,operation:ADC,cycles:6}),
        (0x71,Instruction{address_mode:IndirectY,operation:ADC,cycles:5}),
        // AND
    ]);
}


fn get_flag(flags:u8,which_bit:u8) -> u8 {
    return flags & (1 << which_bit);
}
fn set_bit(original_u8:u8,bit_to_set:u8) -> u8 {
    assert!(bit_to_set < 8);
    let mask = 1 << bit_to_set;
    return original_u8 | mask;
}
fn unset_bit(original_u8:u8,bit_to_unset:u8) -> u8 {
    assert!(bit_to_unset < 8);
    let mask = !(1 << bit_to_unset);
    return original_u8 & mask;
}
#[derive(Hash, Eq, PartialEq, Debug)]
enum Mode {
    Null,
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Absolute,
    AbsoluteIndirect,
    AbsoluteX,
    AbsoluteY,
    IndirectX,
    IndirectY,
    Relative,
}
#[derive(Hash, Eq, PartialEq, Debug)]
enum Operation {
    ADC,	AND,	ASL,	BCC,	BCS,	BEQ,	BIT,	BMI,	BNE,	BPL,	BRK,	BVC,	BVS,	CLC,
    CLD,	CLI,	CLV,	CMP,	CPX,	CPY,	DEC,	DEX,	DEY,	EOR,	INC,	INX,	INY,	JMP,
    JSR,	LDA,	LDX,	LDY,	LSR,	NOP,	ORA,	PHA,	PHP,	PLA,	PLP,	ROL,	ROR,	RTI,
    RTS,	SBC,	SEC,	SED,	SEI,	STA,	STX,	STY,	TAX,	TAY,	TSX,	TXA,	TXS,	TYA,
}

#[derive(Hash, Eq, PartialEq, Debug)]
struct Instruction {
    address_mode: Mode,
    operation: Operation,
    cycles: u8,
}

struct Registers {
    a_reg: u8,
    y_reg: u8,
    x_reg: u8,
    stack_pointer: u8,
    program_counter:u16,
    cpu_flags:u8, // carry 0, zero 1, irq 2 decimal 3, break 4, unused 5, overflow 6, negative 7

}
struct Emulator {
    registers: Registers,
    memory:[u8;65536],
    fetched_data:u8,
    address_absolute:u16,
    address_relative:u16,
    opcode:u8,
    cycles:u8,
    current_mode:Mode,
}

impl Emulator {
    fn new() -> Self {
        let reg = Registers {
            a_reg: 0,
            y_reg: 0,
            x_reg:0,
            stack_pointer: 0,
            program_counter:0,
            cpu_flags:0,
        };

        let mem:[u8;65536] = [0;65536];

        return Emulator {
            registers:reg,
            memory:mem,
            current_mode:Null,
            fetched_data:0,
            address_absolute:0,
            address_relative:0,
            opcode:0,
            cycles:0,
        };
    }
    fn load_rom(&mut self, rom_path:&str){
        // Load ROM Into Memory.
        let rom_bytes = fs::read(rom_path.to_string()).unwrap();
        // TODO READ 16 BYTE HEADER HERE ETC.
        // Load ROM INTO 0x8000 CATRIDGE WRAM
        for i in 0..rom_bytes.len() {
            self.memory[0x8000 + i] = rom_bytes[i];
            // stop at 32kb
            // stop if reaching end of PRG ROM SECTION
            if i + 0x8000 == 0xFFFA {
                break;
            }
            if i == 32768 {
                break;
            }
        }
        self.registers.program_counter = 0x8000;
    }
    fn read_address(&mut self,address:usize) -> u16 {
        // lo
        // hi
        // result = (hi << 8) | lo;
        let idx = address as usize;
        let address_high = self.memory[idx ];
        let address_low = self.memory[idx + 1];
        self.registers.program_counter += 1;
        let addr = ((address_high as u16) << 8) | address_low as u16;
        return addr;
    }

    fn read_byte(&mut self, address:usize) -> u8 {
        return self.memory[address];
    }

    fn write_byte(&mut self, address:usize,value:u8) -> bool {
        self.memory[address] = value;
        return true;
    }

    fn nmi(&mut self){
        self.write_byte(0x0100 + self.registers.stack_pointer as usize,( (self.registers.program_counter >> 8) & 0x00FF) as u8);
        self.registers.stack_pointer -= 1;
        self.write_byte(0x0100 + self.registers.stack_pointer as usize,(self.registers.program_counter & 0x00FF) as u8 );
        self.registers.stack_pointer -= 1;
        self.registers.cpu_flags = set_bit(self.registers.cpu_flags,4);
        self.registers.cpu_flags = set_bit(self.registers.cpu_flags,5);
        self.registers.cpu_flags = set_bit(self.registers.cpu_flags,2);
        self.write_byte(0x0100 + self.registers.stack_pointer as usize,self.registers.cpu_flags);
        self.registers.stack_pointer -= 1;
        self.address_absolute = 0xFFFA;
        let lo:u16 = self.read_byte((self.address_absolute + 0) as usize) as u16;
        let hi:u16 = self.read_byte((self.address_absolute + 1) as usize) as u16;
        self.registers.program_counter = (hi << 8) | lo;
        self.cycles = 8;
    }

    fn irq(&mut self){
        if get_flag(self.registers.cpu_flags,2) == 0 {
            self.write_byte(0x0100 + self.registers.stack_pointer as usize,( (self.registers.program_counter >> 8) & 0x00FF) as u8);
            self.registers.stack_pointer -= 1;
            self.write_byte(0x0100 + self.registers.stack_pointer as usize,(self.registers.program_counter & 0x00FF) as u8 );
            self.registers.stack_pointer -= 1;
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,4);
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,5);
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,2);
            self.write_byte(0x0100 + self.registers.stack_pointer as usize,self.registers.cpu_flags);
            self.registers.stack_pointer -= 1;
            self.address_absolute = 0xFFFE;
            let lo:u16 = self.read_byte((self.address_absolute + 0) as usize) as u16;
            let hi:u16 = self.read_byte((self.address_absolute + 1) as usize) as u16;
            self.registers.program_counter = (hi << 8) | lo;
            self.cycles = 7;
        }
    }

    fn reset(&mut self){
        self.registers.a_reg = 0;
        self.registers.x_reg = 0;
        self.registers.y_reg = 0;
        self.registers.stack_pointer = 0xFD;
        self.registers.cpu_flags = 0x00;
        self.address_absolute = 0xFFFC;
        let lo:u16 = self.read_byte((self.address_absolute + 0) as usize) as u16;
        let hi:u16 = self.read_byte((self.address_absolute + 1) as usize) as u16;
        self.registers.program_counter = (hi << 8) | lo;
        self.address_relative = 0x0000;
        self.address_absolute = 0x0000;
        self.fetched_data = 0x00;
        self.cycles = 8;
    }

    fn start(&mut self){
        self.registers.program_counter = 0x8000 + 0x10;
        loop {
            if self.memory[self.registers.program_counter as usize] == 0x00 {

                println!("Zero encountered Exit!");
                break;
            }
            self.clock();
        }
    }

    fn print_state(&self) {
        println!("----- Dump -------");
        println!("PC 0x{:X}",self.registers.program_counter);
        println!("SP 0x{:X}",self.registers.stack_pointer as u16 + 0x0100);
        println!("A {:X}",self.registers.a_reg);
        println!("X {:X}",self.registers.x_reg);
        println!("Y {:X}",self.registers.y_reg);
        println!("flags: {:#010b}", self.registers.cpu_flags);
        println!("Relative Address: {:X}",self.address_relative);
        println!("Absolute Address: {:X}",self.address_absolute);
        println!("Current Opcode: {:X}",self.opcode);
        println!("--- System Memory Dump --- ");
        print!("[ ");
        let ram = &self.memory[0x8000..0x8100];
        for (i, byte) in ram.iter().enumerate() {
            print!("{:X},",byte);
            if i % 16 == 0 && i != 0 {
                println!();
            }
        }
        print!(" ]");
        println!();
        println!("--- Stack Dump-- ");
        print!("[ ");
        let stack = &self.memory[0x8100..0x8200];
        for (i, byte) in stack.iter().enumerate() {
            print!("{:X},",byte);
            if i % 16 == 0 && i != 0 {
                println!();
            }
        }
        print!(" ]");
        println!();
        println!("--- 0x0 ... 0xFF -- ");
        print!("[ ");
        let zeros = &self.memory[0x0..0xFF];
        for (i, byte) in zeros.iter().enumerate() {
            print!("{:X},",byte);
            if i % 16 == 0 && i != 0 {
                println!();
            }
        }
        print!(" ]");
        println!()
    }
    fn clock(&mut self){
        if self.cycles == 0 {
            let pc = self.registers.program_counter;
            self.opcode = self.memory[pc as usize];
            self.print_state();
            self.execute_instruction();
        }
        self.cycles -= 1;
    }
    fn fetch(&mut self) -> u8 {
        match self.current_mode {
            Implied => {
                return self.read_byte(self.address_absolute as usize);
            }
            Immediate => {
                return self.read_byte(self.address_absolute as usize);
            }
            _ => {
                unreachable!("Unknown Addressing State");
            }
        }
    }
    /*
    ADDRESSING MODES PUT VALUE INTO FETCHED AND INCREMENT THE PROGRAM COUNTER
    */
    fn implied_mode(&mut self) -> u8{
        self.fetched_data = self.registers.a_reg;
        return 0;
    }
    fn accumulator_mode(&mut self) -> u8{
        self.fetched_data = 0;
        return 0;
    }
    fn immediate_mode(&mut self) -> u8 {
        println!("immediate");
        // Increment Program Counter So We Can read
        self.registers.program_counter += 1;
        // set target absolute address to program counter;
        self.address_absolute = self.registers.program_counter;
        return 0;
    }

    fn indirect_mode(&mut self) -> u8 {
        // Increment Program Counter
        self.registers.program_counter += 1;
        let low = self.read_byte(self.registers.program_counter as usize) as u16;
        self.registers.program_counter += 1;
        let high = self.read_byte(self.registers.program_counter as usize) as u16;
        // set absolute address
        let ptr = (high << 8) | low;
        // Emulating that processor bug
        if low == 0x00FF {
            let read1:u16 = self.read_byte((ptr & 0xFF00) as usize) as u16;
            let read2:u16 = self.read_byte((ptr + 0) as usize) as u16;
            self.address_absolute = (read1 << 8 ) | read2;
        } else {
            let read1:u16 = self.read_byte((ptr + 1) as usize) as u16;
            let read2:u16 = self.read_byte((ptr + 0) as usize) as u16;
            self.address_absolute = (read1 << 8 ) | read2;
        }
        return 0;
    }

    fn indirect_mode_page_zero_x(&mut self) -> u8 {
        // Increment Program Counter
        self.registers.program_counter += 1;
        let low = self.read_byte(self.registers.program_counter as usize) as u16;
        self.registers.program_counter += 1;
        let high = self.read_byte(self.registers.program_counter as usize) as u16;
        // set absolute address
        let ptr = (high << 8) | low;
        let lo:u16 = (self.read_byte((ptr + self.registers.x_reg as u16) as usize) & 0x00FF) as u16;
        let hi:u16 = (self.read_byte((ptr + (self.registers.x_reg + 1) as u16) as usize) & 0x00FF) as u16;
        self.address_absolute = (hi << 8) | lo;
        return 0;
    }

    fn indirect_mode_page_zero_y(&mut self) -> u8 {
        // Increment Program Counter
        self.registers.program_counter += 1;
        let low = self.read_byte(self.registers.program_counter as usize) as u16;
        self.registers.program_counter += 1;
        let high = self.read_byte(self.registers.program_counter as usize) as u16;
        // set absolute address
        let ptr = (high << 8) | low;
        let lo = self.read_byte((ptr & 0x00FF) as usize) as u16;
        let hi = self.read_byte(((ptr+1) & 0x00FF) as usize) as u16;
        self.address_absolute = (hi << 8 )| lo;
        if (self.address_absolute & 0xFF00) != (high << 8){
            return 1;
        }
        return 0;
    }

    fn absolute_mode(&mut self) -> u8 {
        // Increment Program Counter
        self.registers.program_counter += 1;
        let low = self.read_byte(self.registers.program_counter as usize) as u16;
        self.registers.program_counter += 1;
        let high = self.read_byte(self.registers.program_counter as usize) as u16;
        // set absolute address
        self.address_absolute = (high << 8) | low;
        return 0;
    }

    fn absolute_mode_x(&mut self) -> u8 {
        // Increment Program Counter
        self.registers.program_counter += 1;
        let low = self.read_byte(self.registers.program_counter as usize) as u16;
        self.registers.program_counter += 1;
        let high = self.read_byte(self.registers.program_counter as usize) as u16;
        // set absolute address
        self.address_absolute = (high << 8) | low;
        self.address_absolute += self.registers.x_reg as u16;
        // Check if we moved to another page if we did return 1 and add to clock cycles.
        if (self.address_absolute & 0xFF00) != (high << 8){
            return 1;
        }
        return 0;
    }

    fn absolute_mode_y(&mut self) -> u8 {
        // Increment Program Counter
        self.registers.program_counter += 1;
        let low = self.read_byte(self.registers.program_counter as usize) as u16;
        self.registers.program_counter += 1;
        let high = self.read_byte(self.registers.program_counter as usize) as u16;
        // set absolute address
        self.address_absolute = (high << 8) | low;
        self.address_absolute += self.registers.y_reg as u16;
        // Check if we moved to another page if we did return 1 and add to clock cycles.
        if (self.address_absolute & 0xFF00) != (high << 8){
            return 1;
        }
        return 0;
    }

    fn zero_page_mode(&mut self) -> u8 {
        //0xFF55 ff is page 55 is offset.
        // Increment pc so we can read the next byte
        self.registers.program_counter += 1;
        let val = self.read_byte(self.registers.program_counter as usize);
        // set absolute address
        self.address_absolute = (val & 0x00FF) as u16;
        return 0;
    }

    fn zero_page_x_mode(&mut self) -> u8 {
        //0xFF55 ff is page 55 is offset.
        // Increment pc so we can read the next byte
        self.registers.program_counter += 1;
        let val = self.read_byte(self.registers.program_counter as usize) + self.registers.x_reg;
        // set absolute address
        self.address_absolute = (val & 0x00FF) as u16;
        return 0;
    }

    fn zero_page_y_mode(&mut self) -> u8 {
        //0xFF55 ff is page 55 is offset.
        // Increment pc so we can read the next byte
        self.registers.program_counter += 1;
        let val = self.read_byte(self.registers.program_counter as usize) + self.registers.y_reg;
        // set absolute address
        self.address_absolute = (val & 0x00FF) as u16;
        return 0;
    }

    fn relative_mode(&mut self) -> u8 {
        // Increment Program Counter
        self.registers.program_counter += 1;
        let low = self.read_byte(self.registers.program_counter as usize) as u16;
        self.registers.program_counter += 1;
        let high = self.read_byte(self.registers.program_counter as usize) as u16;
        // set relative address
        self.address_relative = (high << 8) | low;
        if self.address_relative & 0x80 != 0 {
            self.address_relative |= 0xFF00;
        }
        return 0;
    }

    /*
        ACTUAL OPERATIONS
    */

    fn sei(&mut self) -> u8 {
        self.registers.cpu_flags = set_bit(self.registers.cpu_flags,2);
        return 0;
    }

    fn rti(&mut self) -> u8 {
        // Wrap Stack Pointer Around I Guess Thats What emulators seem to do also
        //self.registers.stack_pointer += 1;
        let wrap_sp = Wrapping(self.registers.stack_pointer as u16);
        let wrap_inc = Wrapping(0x1 as u16);
        let wrap_sp = wrap_sp.add(wrap_inc);
        self.registers.stack_pointer = wrap_sp.0 as u8;
        // Increment the stack pointer even if it wraps
        let wrap_offset = Wrapping(0x0100);
        let wrap_result = wrap_sp.add(wrap_offset);
        self.registers.cpu_flags = self.read_byte(wrap_result.0 as usize);
        // unset flags
        self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,4);
        self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,5);
        self.registers.stack_pointer += 1;
        self.registers.program_counter = self.read_byte(0x0100 + self.registers.stack_pointer as usize) as u16;
        self.registers.stack_pointer += 1;
        self.registers.program_counter |= (self.read_byte(0x0100 + self.registers.stack_pointer as usize) as u16) << 8;
        println!("{:X}",self.registers.program_counter);
        self.print_state();
        return 0;
    }

    /// Set Bits In Flags
    fn clc(&mut self){
        self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,0); // clear carry bit zero
    }

    fn cld(&mut self){
        self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,3); // decimal bit zero
    }

    fn sta(&mut self) -> u8 {
        self.write_byte(self.address_absolute as usize,self.registers.a_reg);
        return 0;
    }

    fn inx(&mut self) -> u8 {
        // we need to wrap here
        let wrap_x = Wrapping(self.registers.x_reg as u16);
        let wrap_inc = Wrapping(0x1 as u16);
        let wrap_x = wrap_x.add(wrap_inc);
        self.registers.x_reg = wrap_x.0 as u8;
        //self.registers.x_reg += 1;
        if self.registers.x_reg == 0 {
            println!("Setting ZERO FLAG");
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,1)
        }
        // negative flag check 7th bit
        if self.registers.x_reg & (1 << 7) != 0 {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,7)
        }
        return 0;
    }

    fn dex(&mut self) -> u8 {
        if self.registers.x_reg == 1 {
            let breaks = 0;
        }
        // we need to wrap here
        let wrap_x = Wrapping(self.registers.x_reg as u16);
        let wrap_inc = Wrapping(0x1 as u16);
        let wrap_x = wrap_x.sub(wrap_inc);
        self.registers.x_reg = wrap_x.0 as u8;
        //self.registers.x_reg -= 1;
        if self.registers.x_reg == 0 {
            println!("Setting ZERO FLAG");
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,1)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,1)
        }
        // negative flag check 7th bit
        if self.registers.x_reg & (1 << 7) != 0 {
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,7)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,7)
        }
        return 0;
    }

    fn lda(&mut self) -> u8{
        let result = self.fetch();
        self.handle_flags(result as usize);
        self.registers.a_reg = result;
        // check if page boundary crossed if so add a cycle
        if (self.address_absolute & 0xFF00) != (self.registers.program_counter & 0xFF00){
            self.cycles += 1;
        }
        // effects zero and neg bits
        // zero bit 1
        if result  == 0 {
            println!("Setting ZERO FLAG");
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,1)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,1)
        }
        // negative flag check 7th bit
        if result & (1 << 7) != 0 {
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,7)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,7)
        }
        return 0;
    }

    fn ldx(&mut self) -> u8{
        let result = self.fetch();
        self.handle_flags(result as usize);
        self.registers.x_reg = result;
        // check if page boundary crossed if so add a cycle
        if (self.address_absolute & 0xFF00) != (self.registers.program_counter & 0xFF00){
            self.cycles += 1;
        }
        // effects zero and neg bits
        // zero bit 1
        if result == 0 {
            println!("Setting ZERO FLAG");
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,1)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,1)
        }
        // negative flag check 7th bit
        if result & (1 << 7) != 0 {
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,7)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,7)
        }
        return 0;
    }
    fn txs(&mut self) -> u8 {
        self.registers.stack_pointer = self.registers.x_reg;
        // effects zero and neg bits
        // zero bit 1
        // zero bit 1
        if self.registers.stack_pointer == 0 {
            println!("Setting ZERO FLAG");
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,1)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,1)
        }
        // negative flag check 7th bit
        if self.registers.stack_pointer & (1 << 7) != 0 {
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,7)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,7)
        }
        return 0;
    }
    // push stack
    // pop stack 0x0100 is start of stack from page zero
    fn pha(&mut self) -> u8 {
        self.write_byte(0x0100 + self.registers.stack_pointer as usize,self.registers.a_reg);
        self.registers.stack_pointer -= 1;
        return 0;
    }
    // pop stack 0x0100 is start of stack from page zero
    fn pla(&mut self) -> u8 {
        self.registers.stack_pointer += 1;
        self.registers.a_reg = self.read_byte(0x0100 + self.registers.stack_pointer as usize);
        self.handle_flags(self.registers.a_reg as usize);
        return 0;
    }


    fn subc(&mut self) -> u8 {
        // Just Do The Sub with carry here
        let fetched = (self.fetch() as u16) ^ 0x00FF;
        // actual ADD here
        let tmp:u16 = self.registers.a_reg as u16 + fetched + get_flag(self.registers.cpu_flags,0) as u16;
        // Handle flags and overflow below.
        self.handle_flags(tmp as usize);
        // Handle overflow flags
        if (self.registers.a_reg as u16 ^ fetched) & (self.registers.a_reg as u16 ^ tmp) & 0x0080 == 1 {
            set_bit(self.registers.cpu_flags,6);
        } else {
            unset_bit(self.registers.cpu_flags,6);
        }
        self.registers.a_reg = (tmp & 0x00FF) as u8;
        return 1;
    }
    fn adc(&mut self) -> u8 {
        // Just Do The Add With Carry Here:w:
        let fetched = self.fetch() as u16;
        // actual ADD here
        let tmp:u16 = self.registers.a_reg as u16 + fetched + get_flag(self.registers.cpu_flags,0) as u16;
        // Handle flags and overflow below.
        self.handle_flags(tmp as usize);
        // Handle overflow flags
        if (self.registers.a_reg as u16 ^ fetched) & (self.registers.a_reg as u16 ^ tmp) as u16 & 0x0080 == 1 {
            set_bit(self.registers.cpu_flags,6);
        } else {
            unset_bit(self.registers.cpu_flags,6);
        }
        self.registers.a_reg = (tmp & 0x00FF) as u8;
        return 1;
    }

    fn bcs(&mut self) -> u8 {
        // check if carry bit is set
        // if carry is set we branch
        if get_flag(self.registers.cpu_flags,0) == 1 {
            self.cycles += 1;
            self.address_absolute = self.registers.program_counter + self.address_relative;
            if (self.address_absolute & 0xFF00) != (self.registers.program_counter & 0xFF00){
                self.cycles += 1;
            }
            self.registers.program_counter = self.address_absolute;
        }
        return 0;
    }

    fn bne(&mut self) -> u8 {
        // check if zero bit is set
        // IF ZERO NOT SET WE BRANCH
        if get_flag(self.registers.cpu_flags,1) == 0 {
            self.cycles += 1;
            let wrap_rel = Wrapping(self.address_relative);
            let wrap_pc = Wrapping(self.registers.program_counter);
            let wrap_result = wrap_pc.add(wrap_rel);
            self.address_absolute = wrap_result.0;
            if (self.address_absolute & 0xFF00) != (self.registers.program_counter & 0xFF00){
                self.cycles += 1;
            }
            self.registers.program_counter = self.address_absolute;
        }
        return 0;
    }

    // AND instruction
    fn and(&mut self) -> u8 {
        let result = self.registers.a_reg & self.fetch();
        self.registers.a_reg = result;
        self.handle_flags(result as usize);
        return 1;
    }

    fn execute_instruction(&mut self) {
        match INSTRUCTION_TABLE.get(&self.opcode) {
            Some(instruction) => {
                // Fetch Data Based On Addressing Mode
                match instruction.address_mode {
                    Implied => {
                        println!("implied");
                        self.cycles += instruction.cycles;
                        self.implied_mode();
                        self.current_mode = Implied;
                    }
                    Immediate => {
                        println!("immediate");
                        self.cycles += instruction.cycles;
                        self.immediate_mode();
                        self.current_mode = Immediate;
                    }
                    ZeroPage => {
                        println!("zero page");
                        self.cycles += instruction.cycles;
                        self.cycles += self.zero_page_mode();
                        self.current_mode = ZeroPage;
                    }
                    ZeroPageX => {
                        println!("zero page x");
                        self.cycles += instruction.cycles;
                        self.cycles += self.zero_page_x_mode();
                        self.current_mode = ZeroPageX;
                    }
                    ZeroPageY => {
                        println!("zero page y");
                        self.cycles += instruction.cycles;
                        self.cycles += self.zero_page_y_mode();
                        self.current_mode = ZeroPageY;
                    }
                    Absolute => {
                        println!("absolute");
                        self.cycles += instruction.cycles;
                        self.cycles += self.absolute_mode();
                        self.current_mode = Absolute;
                    }
                    AbsoluteX => {
                        println!("absolute x");
                        self.cycles += instruction.cycles;
                        self.cycles += self.absolute_mode_x();
                        self.current_mode = AbsoluteX;
                    }
                    AbsoluteY  => {
                        println!("absolute xy");
                        self.cycles += instruction.cycles;
                        self.cycles += self.absolute_mode_y();
                        self.current_mode = AbsoluteY;
                    }
                    IndirectX => {
                        println!("indirect x");
                        self.cycles += instruction.cycles;
                        self.cycles += self.indirect_mode_page_zero_x();
                        self.current_mode = IndirectX;
                    }
                    IndirectY => {
                        println!("indirect y");
                        self.cycles += instruction.cycles;
                        self.cycles += self.indirect_mode_page_zero_y();
                        self.current_mode = IndirectY;

                    }
                    Relative => {
                        println!("relative");
                        self.cycles += instruction.cycles;
                        self.cycles += self.relative_mode();
                        self.current_mode = Relative;
                    }
                    _ => {
                        unreachable!("Addressing Mode Not In Instruction Table")
                    }
                }
                // Match On Opcode
                // we have to borrow here?
                match instruction.operation {
                    RTI => {
                        println!("RTI");
                        self.cycles += self.rti();
                    }
                    AND => {
                        println!("AND!");
                        self.cycles += self.and();
                    }
                    BRK => {
                        println!("BRK!");
                    }
                    SEI => {
                        println!("SEI");
                        self.sei();
                    }
                    CLD => {
                        println!("CLD");
                        self.cld();
                    }
                    LDX => {
                        self.ldx();
                        println!("LDX");
                        self.cycles += self.ldx();
                    }
                    TXS => {
                        println!("TXS");
                        self.cycles += self.txs();
                    }
                    LDA => {
                        println!("LDA");
                        self.cycles += self.lda();
                    }
                    STA => {
                        println!("STA");
                        self.cycles += self.sta();
                    }
                    DEX => {
                        println!("DEX");
                        self.cycles += self.dex();
                    }
                    INX => {
                        println!("INX");
                        self.cycles += self.inx();
                    }
                    BNE => {
                        println!("BNE");
                        self.cycles += self.bne();
                        return;

                    }
                    _ => {
                        unreachable!("Operation Not In Instruction Table");
                    }
                }
            }
            _ => {
                unreachable!("Opcode Not In Instruction Table!");
            }
        }
        self.registers.program_counter += 1;
    }

    fn handle_flags(&mut self,result:usize) {
        // carry flag check zero bit
        if result > 255 {
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,0)
        } else {
            self.registers.cpu_flags =  unset_bit(self.registers.cpu_flags,0)
        }
        // zero bit 1
        if result == 0 {
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,1)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,1)
        }
        // negative flag check 7th bit
        if result & (1 << 7) != 0 {
            self.registers.cpu_flags = set_bit(self.registers.cpu_flags,7)
        } else {
            self.registers.cpu_flags = unset_bit(self.registers.cpu_flags,7)
        }
    }
}



fn main() {
    // TODO parse 16 Byte NES HEADER IN LOAD ROm
    let mut emulator = Emulator::new();
    emulator.load_rom("C:\\Users\\lator\\Desktop\\CC65\\main.nes");
    emulator.start();
    // http://www.6502.org/tutorials/6502opcodes.html#STA
    //http://www.emulator101.com/6502-addressing-modes.html
    //https://github.com/Klaus2m5/6502_65C02_functional_tests
    // https://www.pagetable.com/c64ref/6502/?tab=2#
}


/*match self.opcode {
      // ADC instruction
      0x069 | 0x65 | 0x75 | 0x6D | 0x7D | 0x79 | 0x61 | 0x71 => {
          println!("ADD With Carry!");
          self.adc(opcode);
      }
      // AND
      0x29 | 0x25 | 0x35 | 0x2D | 0x3D | 0x39 | 0x21 | 0x31 => {
          println!("AND!");
      }
      // ASL (Arithimetic shift left)
      0x0A | 0x06 | 0x16 | 0x0E | 0x1E => {
          println!("Arithmetic Shift Left");
      }
      // BIT
      0x24 | 0x2C => {
          println!("TEST BIT");
      }
      // BRANCH INSTRUCTIONS
      0x10 | 0x30 | 0x50 | 0x70 | 0x90 | 0xB0 | 0xD0 | 0xF0 => {
          self.registers.program_counter += 1;
          println!("BRANCH");
      }
      // BRK
      0x00 => {
          println!("BRK");
      }
      // CMP
      0xC9 | 0xC5 | 0xD5 | 0xCD | 0xDD | 0xD9 | 0xC1 | 0xD1 => {
          println!("Compare Accumulator!");
      }
      // CPX
      0xE0 | 0xE4 | 0xEC => {
          println!("Compare X Register");
      }
      // CPY
      0xC0 | 0xC4 | 0xCC => {
          println!("Compare Y Register");
      }
      // DEC
      0xC6 | 0xD6 | 0xCE | 0xDE => {
          println!("Decrement!");
      }
      // EOR
      0x49 | 0x45 | 0x55 | 0x4D | 0x5D | 0x59 | 0x41 | 0x51 => {
          println!("Exclusive OR");
      }
      // FLAG INSTRUCTIONS
      0x18 | 0x38 | 0x58 | 0x78 | 0xB8 | 0xD8 | 0xF8 => {
          println!("Flag instructions");
      }
      // INC MEM
      0xE6 | 0xF6 | 0xEE | 0xFE => {
          println!("INC MEM");
      }
      // JMP
      0x4C | 0x6C => {
          println!("JMP");
      }
      // JSR
      0x20 => {
          println!("JSR");
      }
      // LDA
      0xA9 | 0xA5 | 0xB5 | 0xAD | 0xBD | 0xB9 | 0xA1 | 0xB1 => {
          self.registers.program_counter += 1;
          println!("Load Accumulator");
      }
      // LDX
      0xA2 | 0xA6 | 0xB6 | 0xAE | 0xBE => {
          // Just for now
          self.registers.program_counter += 1;
          println!("Load X Register");
      }
      // LDY
      0xA0 | 0xA4 | 0xB4 | 0xAC | 0xBC => {
          println!("Load Y Register")
      }
      // LSR
      0x4A | 0x46 | 0x56 | 0x4E | 0x5E => {
          println!("Load shift right");
      }
      // NOP
      0xEA => {
          println!("NOP");
      }
      // ORA
      0x09 | 0x05 | 0x15 | 0x0D | 0x1D | 0x19 | 0x01 | 0x11 => {
          println!("bitwise or");
      }
      // Register Instructions
      0xAA | 0x8A | 0xCA | 0xE8 | 0xA8 | 0x98 | 0x88 | 0xC8 => {
          println!("register instruction");
      }
      // ROL
      0x2A | 0x26 | 0x36 | 0x2E | 0x3E => {
          println!("rotate left");
      }
      // ROR
      0x6A | 0x66 | 0x76 | 0x6E | 0x7E => {
          println!("rotate right");
      }
      // RTI
      0x40 => {
          println!("return from interrupt");
      }
      // RTS
      0x60 => {
          println!("return from subroutine");
      }
      // SBC
      0xE9 | 0xE5 | 0xF5 | 0xED | 0xFD| 0xF9 | 0xE1 | 0xF1 => {
          println!("Subtract with carry")
      }
      // STA
      0x85 | 0x95 | 0x8D | 0x9D | 0x99 | 0x81 | 0x91 => {
          self.registers.program_counter += 1;
          println!("Store accumulator");
      }
      // Stack instructions
      0x9A | 0xBA | 0x48 | 0x68 | 0x08 | 0x28 => {
          println!("stack instruction");
      }
      // STX
      0x86 | 0x96 | 0x8E => {
          println!("Store X register");
      }
      // STY
      0x84 | 0x94 | 0x8C => {
          println!("Store Y register");
      }
      // Unknown Opcode?
      _ => unreachable!("Unknown Opcode!")
  }*/