use crate::display::segment::*;
use crate::display::position;
use crate::display::{Buffer, Display};

/// Implements the rolling animation: single segment spinning around 4 digit displays.
pub struct Spinner {
    next_frame: usize,
    ticks_left: u8,
}

impl Spinner {
    const EXPECTED_FREQUENCY_HZ: u8 = 200;
    const TICKS_PER_FRAME: u8 = Self::EXPECTED_FREQUENCY_HZ / 25;
    const FRAMES: [Buffer; 12] = [
        [A, 0, 0, 0],
        [0, A, 0, 0],
        [0, 0, A, 0],
        [0, 0, 0, A],
        [0, 0, 0, B],
        [0, 0, 0, C],
        [0, 0, 0, D],
        [0, 0, D, 0],
        [0, D, 0, 0],
        [D, 0, 0, 0],
        [E, 0, 0, 0],
        [F, 0, 0, 0],
    ];

    /// Returns a new instance of Spinner.
    pub fn new() -> Self {
        Self {
            next_frame: 0,
            ticks_left: 0,
        }
    }

    /// Updates the animation's internal state and maybe updates the provided writable display buffer.
    ///
    /// This is intended to be called at EXPECTED_FREQUENCY_HZ by the timer interrupt handler.
    pub fn advance(&mut self, buffer: &mut Buffer) {
        if self.ticks_left > 0 {
            self.ticks_left -= 1;
            return;
        }

        for i in 0..buffer.len() {
            buffer[i] = Self::FRAMES[self.next_frame][i];
        }

        self.next_frame = (self.next_frame + 1) % Self::FRAMES.len();
        self.ticks_left = Self::TICKS_PER_FRAME - 1;
    }
}

/// Implements the sleeping animation: single rightmost dot appears for a moment every few seconds
pub struct BlinkingDot {
    dot_visible: bool,
    ticks_left: u16,
}

impl BlinkingDot {
    const EXPECTED_FREQUENCY_HZ: u8 = 50;
    const TICKS_VISIBLE: u16 = (Self::EXPECTED_FREQUENCY_HZ / 2) as u16;
    const TICKS_HIDDEN: u16 = Self::EXPECTED_FREQUENCY_HZ as u16 * 10;

    /// Returns a new instance of BlinkingDot
    pub fn new() -> Self {
        Self {
            dot_visible: false,
            ticks_left: Self::TICKS_HIDDEN - 1,
        }
    }

    /// Updates the animation's internal state and maybe updates the provided display.
    ///
    /// This is intended to be called at EXPECTED_FREQUENCY_HZ by the timer interrupt handler.
    pub fn advance(&mut self, display: &mut Display) {
        if self.ticks_left > 0 {
            self.ticks_left -= 1;
            return;
        }

        if self.dot_visible {
            self.dot_visible = false;
            self.ticks_left = Self::TICKS_HIDDEN - 1;
            display.force_output(0, 0);
        } else {
            self.dot_visible = true;
            self.ticks_left = Self::TICKS_VISIBLE - 1;
            display.force_output(POINT, position::D4);
        }
    }
}
