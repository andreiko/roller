use avr_device::atmega328p::Peripherals;
use crate::display::segment::POINT;

/// Maps segments of a standard 7-segment display to channel bits of the I/O port "D" to which
/// the corresponding LED's are connected on the board.
pub mod segment {
    pub const A: u8 = 1 << 0;
    pub const B: u8 = 1 << 1;
    pub const C: u8 = 1 << 2;
    pub const D: u8 = 1 << 3;
    pub const E: u8 = 1 << 4;
    pub const F: u8 = 1 << 5;
    pub const G: u8 = 1 << 6;
    pub const POINT: u8 = 1 << 7;
}

/// Defines visual representations of all digits and some letters using available display segments.
pub mod symbol {
    use crate::display::segment::*;

    pub const ZERO: u8 = A | B | C | D | E | F;
    pub const ONE: u8 = B | C;
    pub const TWO: u8 = A | B | D | E | G;
    pub const THREE: u8 = A | B | C | D | G;
    pub const FOUR: u8 = B | C | F | G;
    pub const FIVE: u8 = A | C | D | F | G;
    pub const SIX: u8 = A | C | D | E | F | G;
    pub const SEVEN: u8 = A | B | C;
    pub const EIGHT: u8 = A | B | C | D | E | F | G;
    pub const NINE: u8 = A | B | C | D | F | G;
    pub const ALPHA: u8 = A | B | C | E | F | G;
    pub const DELTA: u8 = B | C | D | E | G;
    pub const ECHO: u8 = A | D | E | F | G;

    /// Defines an array where visual representations of digits 0-9 are stored under the corresponding indices.
    pub const MAP: [u8; 10] = [ZERO, ONE, TWO, THREE, FOUR, FIVE, SIX, SEVEN, EIGHT, NINE];
}

/// Maps 7-segment displays to channel bits of the I/O port "B" to which they're connected on the board.
pub mod position {
    pub const D1: u8 = 1 << 0;
    pub const D2: u8 = 1 << 1;
    pub const D3: u8 = 1 << 2;
    pub const D4: u8 = 1 << 6;

    /// Defines an array where each display's channel bit can be looked up by its index.
    pub const MAP: [u8; 4] = [D1, D2, D3, D4];
    /// All the channel bits added together.
    pub const MASK_ALL: u8 = D1 | D2 | D3 | D4;
}

/// Type alias for an array representing a display buffer. Each element corresponds to the
/// symbol displayed by a 7-segment display, left to right.
pub type Buffer = [u8; 4];

#[inline(always)]
/// Returns a new empty buffer.
const fn empty_buffer() -> Buffer {
    [0; 4]
}

/// Implements a multi-digit display based on the LED matrix principle where each 7-segment display
/// is a row and each of its segment is a column.
pub struct Display {
    pub buffer: Buffer,
    next_index: usize,
}

impl Display {
    /// Returns a new instance of Display.
    pub const fn new() -> Self {
        Self {
            buffer: empty_buffer(),
            next_index: 0,
        }
    }

    /// Prepares I/O ports "B" and "D" for the operation of the display.
    pub fn initialize(&mut self) {
        unsafe {
            let p = Peripherals::steal();
            // Switch all channels of the I/O port "D" into the output mode.
            p.PORTD.ddrd.write(|w| w.bits(0xff));
            // Set outputs of all the I/O port "D" channels to LOW - no segment is active.
            p.PORTD.portd.write(|w| w.bits(0));

            // Switch channels of the I/O port "B" connected to the displays into the output mode.
            p.PORTB.ddrb.modify(|r, w| w.bits(r.bits() | position::MASK_ALL));
            // Set outputs of the I/O port "B" channels connected to the displays to LOW — no display is selected.
            p.PORTB.portb.modify(|r, w| w.bits(r.bits() & !position::MASK_ALL));
        }
    }

    /// Switch active display to the next one.
    ///
    /// This is intended to be called at regular intervals by the timer interrupt handler.
    pub fn refresh(&mut self) {
        for n in 1..=self.buffer.len() {
            if self.buffer[self.next_index] == 0 {
                self.next_index = (self.next_index + 1) % 4;
                if n == self.buffer.len() {
                    return;
                }
            }
        }

        unsafe {
            let p = Peripherals::steal();
            // Turn off all segments on the currently active display.
            p.PORTD.portd.write(|w| w.bits(0));
            // Unset all channel bits connected to the displays, set the bit of the display that must be activated next.
            p.PORTB.portb.modify(|r, w| w.bits((r.bits() & !position::MASK_ALL) | position::MAP[self.next_index]));
            // Copy the corresponding element from the diplay buffer into the I/O port "D".
            p.PORTD.portd.write(|w| w.bits(self.buffer[self.next_index]));
        };

        self.next_index = (self.next_index + 1) % 4;
    }

    /// Renders the provided unsigned 16-bit number aligned to the right with a dot at the end,
    /// writes the result to the display buffer.
    pub fn set_number(&mut self, n: u16) {
        let mut tmp = [0u8; 4];
        let size = encode_u16_into(&mut tmp, n);
        let shift = 4 - size;

        for i in 0..shift {
            self.buffer[i] = 0;
        }

        for i in 0..size {
            self.buffer[i + shift] = tmp[i];
        }

        self.buffer[self.buffer.len() - 1] |= POINT;
    }

    /// Makes the display show the specified symbol in the specified position regardless of
    /// the current internal state.
    pub fn force_output(&mut self, symbol: u8, position: u8) {
        unsafe {
            let p = Peripherals::steal();
            p.PORTB.portb.modify(|r, w| w.bits((r.bits() & !position::MASK_ALL) | position));
            p.PORTD.portd.write(|w| w.bits(symbol));
        };
    }
}

/// Renders an unsigned 16-bit number into the provided buffer. Returns the number of digits rendered.
pub fn encode_u16_into(buf: &mut [u8], mut n: u16) -> usize {
    let mut size = 0;
    let mut divisor = 1000;
    while divisor > 0 && size < buf.len() {
        let d = n / divisor;
        if size > 0 || d > 0 {
            buf[size] = symbol::MAP[d as usize];
            size += 1;
        }
        n = n % divisor;
        divisor /= 10;
    }

    if size == 0 {
        buf[0] = symbol::ZERO;
        size = 1;
    }

    for i in size..buf.len() {
        buf[i] = 0;
    }

    size
}

/// Renders an unsigned 8-bit number into the provided buffer. Returns the number of digits rendered.
pub fn encode_u8_into(buf: &mut [u8], mut n: u8) -> usize {
    let mut size = 0;
    let mut divisor = 100;
    while divisor > 0 && size < buf.len() {
        let d = n / divisor;
        if size > 0 || d > 0 {
            buf[size] = symbol::MAP[d as usize];
            size += 1;
        }
        n = n % divisor;
        divisor /= 10;
    }

    if size == 0 {
        buf[0] = symbol::ZERO;
        size = 1;
    }

    for i in size..buf.len() {
        buf[i] = 0;
    }

    size
}

/// Re-initializes display from scratch and makes all displays show the specified symbol.
pub fn fail_with_symbol(s: u8) {
    unsafe {
        let p = Peripherals::steal();
        p.PORTD.ddrd.write(|w| w.bits(0xff));
        p.PORTD.portd.write(|w| w.bits(s));
        p.PORTB.ddrb.modify(|r, w| w.bits(r.bits() | position::MASK_ALL));
        p.PORTB.portb.write(|w| w.bits(position::MASK_ALL));
    }
}
