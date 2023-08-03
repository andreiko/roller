#![no_main]
#![no_std]
#![feature(abi_avr_interrupt)]

mod system;
mod utils;
mod scales;
mod animation;
mod display;
mod random;

use core::num::Wrapping;
use avr_device::atmega328p::{Peripherals, tc0, adc};
use avr_device::interrupt;

use crate::display::Display;
use crate::scales::{Zone, QUANTITY, QUALITY};
use crate::utils::Agg;
use crate::animation::{Spinner, BlinkingDot};

/// How many of the latest measurements are stored.
const AGG_SIZE: usize = 16;

#[cfg(feature = "debug_spi")]
use crate::utils::Ring;

/// Global device state.
static mut DEVICE: Device = Device::new();

#[interrupt(atmega328p)]
/// This is called by the hardware timer with an approximate f=200Hz
unsafe fn TIMER0_COMPA() {
    DEVICE.timer_interrupt();
}

#[interrupt(atmega328p)]
/// This is called by the hardware Analog-to-Digital Converter when a conversion result is ready.
unsafe fn ADC() {
    DEVICE.adc_interrupt();
}

#[cfg(feature = "debug_spi")]
#[interrupt(atmega328p)]
unsafe fn SPI_STC() {
    DEVICE.debug_stc();
}

/// Defines things that we measure with the ADC.
enum Measurement {
    PotQuantity,
    PotQuality,
    AccX,
    AccY,
    AccZ,
}

/// Defines specific device states.
enum State {
    Displaying { disturbed_ticks: u8, idle_ticks: u16 },
    Rolling { params: random::Params, quantity: u8, results: Agg<u8, 20>, balanced_ticks: u8, animation: Spinner },
    Sleeping { disturbed_ticks: u8, animation: BlinkingDot },
}

/// Defines general device state and behavior.
struct Device {
    display: Display,
    state: State,

    /// Currently active settings. Uninitialized for the first few moments after the startup.
    quantity: Option<&'static Zone>,
    quality: Option<&'static Zone>,

    /// What's currently being measured by the ADC.
    adc_measuring: Option<Measurement>,

    /// Aggregations of recentl measurement results.
    acc_l1: AccLevel,
    acc_l2: AccLevel,
    pot_quantity: Agg<u16, AGG_SIZE>,
    pot_quality: Agg<u16, AGG_SIZE>,

    /// Bits that constantly get updated by the accelerometer measurement results.
    entropy: Wrapping<u16>,

    #[cfg(feature = "debug_spi")]
    debug_buf: Ring,
    #[cfg(feature = "debug_spi")]
    debug_sending: bool,
}

/// Container for accelerator measurement aggregations.
struct AccLevel {
    x: Agg<u16, AGG_SIZE>,
    y: Agg<u16, AGG_SIZE>,
    z: Agg<u16, AGG_SIZE>,
}

impl AccLevel {
    /// Returns a new instance of AccLevel.
    const fn new() -> Self {
        Self {
            x: Agg::new(),
            y: Agg::new(),
            z: Agg::new(),
        }
    }
}

impl Device {
    const NORMAL_FREQUENCY: u8 = 200;
    const SLEEPING_FREQUENCY: u8 = 50;

    // minimal force amplitude to be considered a disturbance, measured in ADC steps (1/256g)
    const MIN_FORCE_AMPLITUDE: u16 = 40; // ~0.156g

    const TICKS_TO_DISTURB: u8 = (Device::NORMAL_FREQUENCY as f64 * 0.35) as u8;
    const TICKS_TO_BALANCE: u8 = (Device::NORMAL_FREQUENCY as f64 * 0.6) as u8;
    const TICKS_TO_SLEEP: u16 = Device::NORMAL_FREQUENCY as u16 * 30;
    const TICKS_TO_WAKE: u8 = (Device::SLEEPING_FREQUENCY as f64 * 0.4) as u8;

    /// Returns a new instance of Device.
    pub const fn new() -> Self {
        Device {
            display: Display::new(),
            adc_measuring: None,

            entropy: Wrapping(0),
            state: State::Displaying { disturbed_ticks: 0, idle_ticks: 0 },

            pot_quantity: Agg::new(),
            pot_quality: Agg::new(),
            acc_l1: AccLevel::new(),
            acc_l2: AccLevel::new(),

            quantity: None,
            quality: None,

            #[cfg(feature = "debug_spi")]
            debug_buf: Ring::new(),
            #[cfg(feature = "debug_spi")]
            debug_sending: false,
        }
    }

    /// Performs initial hardware initialization.
    pub fn initialize(&mut self) {
        let p = unsafe {
            Peripherals::steal()
        };
        // enable the default "idle" sleeping mode
        p.CPU.smcr.write(|w| w.se().variant(true));

        self.display.initialize();

        Self::timer_init();
        Self::adc_init();

        #[cfg(feature = "debug_spi")]
        Self::debug_init();

        unsafe { interrupt::enable() };
    }

    /// Uses the latest averaged measurements of potentiometer channels to detect if the settings
    /// have been changed. If either of the settings has changed, displays the new settings.
    pub fn test_pots(&mut self) {
        let mut render = false;

        if let Some(new) = Self::test_pot(&self.pot_quantity, self.quantity, &QUANTITY[..]) {
            render = true;
            self.quantity = Some(new);
        }

        if let Some(new) = Self::test_pot(&self.pot_quality, self.quality, &QUALITY[..]) {
            render = true;
            self.quality = Some(new);
        }

        if !render {
            return;
        }

        match (self.quantity, self.quality) {
            (Some(quantity), Some(quality)) => {
                self.enter_displaying();
                self.render_settings(quantity.value, quality.value);
            }
            _ => {}
        }
    }

    /// Determine whether the new position reported by the pot should lead to a change of the current setting.
    fn test_pot(pos: &Agg<u16, AGG_SIZE>, current: Option<&'static Zone>, scale: &'static [Zone]) -> Option<&'static Zone> {
        let avg = if let Some(avg) = pos.avg_full::<u16>() {
            avg
        } else {
            return None;
        };

        if let Some(s) = current {
            return scales::detect_zone_change(avg, 10, s, scale);
        }
        return Some(scales::detect_zone(avg, scale));
    }

    /// Render the currently active "quantity" and "quality" settings and write to the display.
    fn render_settings(&mut self, quantity: u8, quality: u8) {
        let mut quantity_buf = [0u8; 2];
        let quantity_len = display::encode_u8_into(&mut quantity_buf, quantity);

        let mut quality_buf = [0u8; 2];
        let quality_len = display::encode_u8_into(&mut quality_buf, quality);

        self.display.buffer = match (quantity_len, quality_len) {
            (1, 1) => [
                0,
                quantity_buf[0],
                display::symbol::DELTA,
                quality_buf[0],
            ],
            (2, 1) => [
                quantity_buf[0],
                quantity_buf[1],
                display::symbol::DELTA,
                quality_buf[0],
            ],
            (1, 2) => [
                quantity_buf[0],
                display::symbol::DELTA,
                quality_buf[0],
                quality_buf[1],
            ],
            (2, 2) => [
                quantity_buf[0],
                quantity_buf[1] | display::segment::POINT,
                quality_buf[0],
                quality_buf[1],
            ],
            _ => panic!()
        };
    }

    /// Uses the latest aggregated measurements of the accelerometer axes to trigger transitions
    /// between the "Rolling" and "Displaying" states.
    pub fn test_acceleration(&mut self) {
        let amps = (self.acc_l2.x.amplitude_full(), self.acc_l2.y.amplitude_full(), self.acc_l2.z.amplitude_full());
        let (ax, ay, az) = if let (Some(ax), Some(ay), Some(az)) = amps {
            (ax, ay, az)
        } else {
            return;
        };

        match &mut self.state {
            State::Displaying { disturbed_ticks, idle_ticks } => {
                if Self::acc_has_been_balanced(ax, ay, az) {
                    // the signal amplitudes of all axes have been low, reset the disturbance counter
                    *disturbed_ticks = 0;
                    *idle_ticks += 1;
                    if *idle_ticks > Self::TICKS_TO_SLEEP {
                        self.enter_sleeping();
                    }
                    return;
                }

                *disturbed_ticks += 1;
                *idle_ticks = 0;
                if *disturbed_ticks > Self::TICKS_TO_DISTURB {
                    // the signal amplitude of at least one axis has been high for long enough time
                    // to trigger a transition to the "Rolling" state.
                    // Abort if settings haven't been initialized yet.
                    if let (Some(quantity), Some(quality)) = (self.quantity, self.quality) {
                        self.enter_rolling(quantity.value, quality.value);
                    }
                }
            }
            State::Rolling { balanced_ticks, quantity, results, .. } => {
                if Self::acc_has_been_disturbed(ax, ay, az) {
                    // the signal amplitude of at least one axis has been high recently, reset the balance counter.
                    *balanced_ticks = 0;
                    return;
                }

                *balanced_ticks += 1;
                if *balanced_ticks >= Self::TICKS_TO_BALANCE {
                    // the signal amplitudes of all axes have been low for long enough time to exit
                    // the "Rolling" state and display the result. If the result is not ready, try this
                    // again on the next timer tick.
                    if let Some(sum) = results.sum_of_first::<u16>(*quantity as usize) {
                        self.display.set_number(sum);
                        self.enter_displaying();
                    }
                }
            }
            State::Sleeping { disturbed_ticks, .. } => {
                if Self::acc_has_been_balanced(ax, ay, az) {
                    // the signal amplitudes of all axes have been low, reset the disturbance counter
                    *disturbed_ticks = 0;
                    return;
                }

                *disturbed_ticks += 1;
                if *disturbed_ticks > Self::TICKS_TO_WAKE {
                    // the signal amplitude of at least one axis has been high for long enough time
                    // to trigger a transition to the "Rolling" state.
                    // Abort if settings haven't been initialized yet.
                    if let (Some(quantity), Some(quality)) = (self.quantity, self.quality) {
                        self.enter_rolling(quantity.value, quality.value);
                    }
                }
            }
        };
    }

    /// shortcut for checking for sufficient disturbance on any axis
    fn acc_has_been_disturbed(ax: u16, ay: u16, az: u16) -> bool {
        ax >= Self::MIN_FORCE_AMPLITUDE || ay >= Self::MIN_FORCE_AMPLITUDE || az >= Self::MIN_FORCE_AMPLITUDE
    }

    /// shortcut for checking for balance on all axis
    fn acc_has_been_balanced(ax: u16, ay: u16, az: u16) -> bool {
        ax < Self::MIN_FORCE_AMPLITUDE && ay < Self::MIN_FORCE_AMPLITUDE && az < Self::MIN_FORCE_AMPLITUDE
    }

    /// Transitions the device into the "Rolling" state and prepares parameters for the random
    /// result generation from the current settings.
    fn enter_rolling(&mut self, quantity: u8, quality: u8) {
        if matches!(self.state, State::Sleeping { .. }) {
            Self::timer_set_normal();
        }
        self.state = State::Rolling {
            quantity,
            params: random::params_for(quality),
            results: Agg::new(),
            balanced_ticks: 0,
            animation: Spinner::new(),
        };
    }

    /// Transitions the device into the "Displaying" state.
    fn enter_displaying(&mut self) {
        if matches!(self.state, State::Sleeping { .. }) {
            Self::timer_set_normal();
        }
        self.state = State::Displaying { disturbed_ticks: 0, idle_ticks: 0 };
    }

    /// Transitions the device into the "Sleeping" state.
    fn enter_sleeping(&mut self) {
        Self::timer_set_sleeping();
        self.state = State::Sleeping { disturbed_ticks: 0, animation: BlinkingDot::new() };
        // turn the display off immediately
        self.display.force_output(0, 0);
    }

    // Sets timer to normal frequency (200Hz)
    fn timer_set_normal() {
        let p = unsafe { Peripherals::steal() };
        // sets prescaler to /1024 for timer0.
        p.TC0.tccr0b.write(|w| w.cs0().variant(tc0::tccr0b::CS0_A::PRESCALE_1024));
        // sets timer0's Output Compare Register "A" to 38 ((8,000,000/1024)/(38+1)) = 200.3205)
        p.TC0.ocr0a.write(|w| w.bits(38));
    }

    // Sets timer to the reduced sleeping frequency (50Hz)
    fn timer_set_sleeping() {
        let p = unsafe { Peripherals::steal() };
        p.TC0.tccr0b.write(|w| w.cs0().variant(tc0::tccr0b::CS0_A::PRESCALE_1024));
        // sets timer0's Output Compare Register "A" to 155 ((8,000,000/1024)/(155+1)) = 50.0801)
        p.TC0.ocr0a.write(|w| w.bits(155));
    }

    /// Initializes the hardware timer to call the interrupt handler at approximately f=200Hz
    ///
    /// Assumes the MCU frequency to be 8MHz.
    fn timer_init() {
        let p = unsafe { Peripherals::steal() };

        // enables "Clear Timer on Compare" mode for timer0.
        p.TC0.tccr0a.write(|w| w.wgm0().variant(tc0::tccr0a::WGM0_A::CTC));
        Self::timer_set_normal();
        // enables Output Compare Match "A" Interrupt for timer0.
        p.TC0.timsk0.write(|w| w.ocie0a().bit(true));

        // TODO: calculate the best prescaler and OCR values for the desired freqnency with a macro
    }

    /// Interrupt handler for the timer.
    pub fn timer_interrupt(&mut self) {
        match &mut self.state {
            State::Rolling { animation: spinner, results, params, .. } => {
                // advance the spinning animation.
                spinner.advance(&mut self.display.buffer);

                // generate the a new random die throw and add to the results on success.
                if let Some(rnd) = random::generate(&params, self.entropy.0 as u8) {
                    results.put(rnd + 1);
                }
            }
            State::Sleeping { animation, .. } => {
                animation.advance(&mut self.display);
            }
            _ => {}
        }

        if !matches!(self.state, State::Sleeping{ .. } ) {
            self.display.refresh();
        }

        self.adc_start(Measurement::PotQuantity);
    }

    /// Initialize ADC.
    fn adc_init() {
        let p = unsafe { Peripherals::steal() };
        // clear the ADC power reduction bit of the power reduction register.
        p.CPU.prr.modify(|_, w| w
            .pradc().variant(false)
        );
        p.ADC.adcsra.write(|w| w
            // set the ADC prescaler to /128 (puts the ADC clock into the required 50kHz-100kHz range).
            .adps().variant(adc::adcsra::ADPS_A::PRESCALER_128)
            // enable the ADC interrupt.
            .adie().variant(true)
        );
    }

    /// Interrupt handler for the ADC.
    pub fn adc_interrupt(&mut self) {
        let p = unsafe { Peripherals::steal() };
        self.adc_measuring.take().map(|m| self.adc_ready(m, p.ADC.adc.read().bits()));
    }

    /// Starts the specified measurement on the ADC.
    ///
    /// Assumes the MCU frequency to be 1MHz.
    fn adc_start(&mut self, m: Measurement) {
        if self.adc_measuring.is_some() {
            // Currently, the ~5ms interval between timer ticks leaves enough time for 5 ADC measurements
            // and their interpretation. If the code changes and we start seeing panics here,
            // we'll know that something needs to be optimized.
            panic!();
        }

        // maps measurements to the ADC channels connected to the corresponding devices on the board.
        let chan = match m {
            Measurement::AccX => adc::admux::MUX_A::ADC0,
            Measurement::AccY => adc::admux::MUX_A::ADC1,
            Measurement::AccZ => adc::admux::MUX_A::ADC2,
            Measurement::PotQuantity => adc::admux::MUX_A::ADC3,
            Measurement::PotQuality => adc::admux::MUX_A::ADC4,
        };

        self.adc_measuring = Some(m);

        let p = unsafe { Peripherals::steal() };
        p.ADC.admux.write(|w| w
            // specify the source channel for the ADC.
            .mux().variant(chan)
            // specify that the AVCC pin of the MCU must be used as a reference.
            .refs().variant(adc::admux::REFS_A::AVCC)
        );
        p.ADC.adcsra.modify(|_, w| w
            // enable the ADC.
            .aden().variant(true)
            // start an ADC conversion.
            .adsc().variant(true)
        );
    }

    /// Handles a completed measurement result from the ADC.
    fn adc_ready(&mut self, m: Measurement, result: u16) {
        // if this is an accelerometer measurement, add it to the entropy.
        if matches!(m, Measurement::AccX | Measurement::AccY | Measurement::AccZ) {
            #[cfg(feature = "debug_spi")]
            self.debug_acc_measurement(&m, result);

            self.entropy += Wrapping(result);
        }

        match m {
            Measurement::PotQuantity => {
                self.pot_quantity.put(result);
                self.adc_start(Measurement::PotQuality);
            }
            Measurement::PotQuality => {
                self.pot_quality.put(result);
                self.adc_start(Measurement::AccX);
            }
            Measurement::AccX => {
                self.acc_l1.x.put(result);
                self.acc_l1.x.avg_full::<u16>().take().into_iter().for_each(|x| self.acc_l2.x.put(x));

                self.adc_start(Measurement::AccY);
            }
            Measurement::AccY => {
                self.acc_l1.y.put(result);
                self.acc_l1.y.avg_full::<u16>().take().into_iter().for_each(|y| self.acc_l2.y.put(y));

                self.adc_start(Measurement::AccZ);
            }
            Measurement::AccZ => {
                self.acc_l1.z.put(result);
                self.acc_l1.z.avg_full::<u16>().take().into_iter().for_each(|z| self.acc_l2.z.put(z));

                let p = unsafe { Peripherals::steal() };
                // disable the ADC
                p.ADC.adcsra.modify(|_, w| w.aden().variant(false));

                self.test_pots();
                self.test_acceleration();
            }
        }
    }

    #[cfg(feature = "debug_spi")]
    fn debug_init() {
        let p = unsafe { Peripherals::steal() };
        p.CPU.prr.modify(|_, w| w
            .prspi().variant(false)
        );
        p.PORTB.ddrb.modify(|_, w| w
            .pb3().variant(true)
            .pb5().variant(true)
        );
        p.SPI.spcr.write(|w| w
            .spie().variant(true)
            .spe().variant(true)
            .mstr().variant(true)
        )
    }

    #[cfg(feature = "debug_spi")]
    fn debug_stc(&mut self) {
        if let Some(next_data) = self.debug_buf.read() {
            let p = unsafe { Peripherals::steal() };
            p.SPI.spdr.write(|w| w.bits(next_data));
        } else {
            self.debug_sending = false;
        }
    }

    #[cfg(feature = "debug_spi")]
    fn debug_acc_measurement(&mut self, m: &Measurement, value: u16) {
        if matches!(m, Measurement::AccX) {
            self.debug_push_u16(u16::MAX);
        }
        self.debug_push_u16(value);
    }

    #[cfg(feature = "debug_spi")]
    fn debug_push_u8(&mut self, data: u8) {
        if !self.debug_sending {
            self.debug_sending = true;
            let p = unsafe { Peripherals::steal() };
            p.SPI.spdr.write(|w| w.bits(data));
            return;
        }

        self.debug_buf.write(data);
    }

    #[cfg(feature = "debug_spi")]
    fn debug_push_u16(&mut self, data: u16) {
        // msb
        self.debug_push_u8((data >> 8) as u8);
        // lsb
        self.debug_push_u8((data & (u8::MAX) as u16) as u8);
    }
}

#[no_mangle]
/// Entry point. This initializes the hardware and enters an infinite loop.
/// All the behaviour is defined in the interrupt handlers.
pub unsafe extern fn main() {
    DEVICE.initialize();
    loop {
        avr_device::asm::sleep();
    }
}
