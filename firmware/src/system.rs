use crate::display::fail_with_symbol;
use crate::display::symbol::{ALPHA, ECHO};

#[panic_handler]
#[no_mangle]
/// This function gets called by Rust when there's a panic. It means that the program
/// is in an unrecoverable state. Currently shows "EEEE" on the display.
unsafe extern "C" fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
    // TODO: display an actual error code instead of "EEEE" if available from PanicInfo.
    avr_device::interrupt::disable();
    fail_with_symbol(ECHO);
    loop {}
}

#[no_mangle]
/// This function gets called by Rust in a situation related to a panic, but not sure exactly when.
/// Currently shows "AAAA" on the display.
unsafe extern "C" fn abort() {
    avr_device::interrupt::disable();
    fail_with_symbol(ALPHA);
}
