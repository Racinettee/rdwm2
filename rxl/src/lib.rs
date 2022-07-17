use libc::{waitpid, WNOHANG};
use x11::xlib::{Display, XErrorEvent, BadWindow, BadMatch, BadDrawable, BadAccess, XDefaultRootWindow, SubstructureRedirectMask, XSync};

mod xconst;
mod drw;

pub use drw::*;

static mut XERRORXLIB: Option<unsafe extern "C" fn(*mut Display, *mut XErrorEvent) -> i32> = None;
#[no_mangle]
pub extern "C" fn check_other_wm(display: *mut Display) -> i32 {
    use x11::xlib::{
        XSetErrorHandler, XSelectInput
    };
    // Get the original error handling function
    unsafe {
        XERRORXLIB = XSetErrorHandler(Some(xerrorstart));
        // this causes an error if another window manager is running
        XSelectInput(display, XDefaultRootWindow(display), SubstructureRedirectMask);
        XSync(display, 0);
        XSetErrorHandler(Some(xerror));
        XSync(display, 0);
    }

    0
}
#[no_mangle]
pub unsafe extern "C" fn xerror(display: *mut Display, ee: *mut XErrorEvent) -> i32 {
    #[allow(non_upper_case_globals)]
    match ((*ee).request_code, (*ee).error_code) {
        (xconst::X_SETINPUTFOCUS,   BadMatch) |
        (xconst::X_POLYTEXT8,       BadDrawable) |
        (xconst::X_POLYFILLRECTANGLE, BadDrawable) |
        (xconst::X_POLYSEGMENT,     BadDrawable) |
        (xconst::X_CONFIGUREWINDOW, BadMatch) |
        (xconst::X_GRABBUTTON,      BadAccess) |
        (xconst::X_GRABKEY,         BadAccess) |
        (xconst::X_COPYAREA,        BadDrawable) |
        (_, BadWindow) => return 0,
        (_, _) => ()
    }
    eprintln!("rdwm: fatal error: request code={}, error code={}", (*ee).request_code, (*ee).error_code);
    return XERRORXLIB.unwrap()(display, ee); /* may call exit */
}
pub extern "C" fn xerrorstart(_display: *mut Display, _ee: *mut XErrorEvent) -> i32 {
    eprintln!("another wm is already running");
    std::process::exit(-1);
}
#[no_mangle]
extern "C" fn sigchld(_: i32) {
    let mut nilstatus: libc::c_int = 0;
    while 0 < unsafe { waitpid(-1, &mut nilstatus as *mut libc::c_int, WNOHANG) } {}
}