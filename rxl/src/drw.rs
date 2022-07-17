use std::{slice};

use x11::{xlib::{XSetForeground, XDrawRectangle, Display, Window, Drawable, GC, Cursor, XFillRectangle}, xft::{XftFont, FcPattern, XftColor}};

#[repr(C)]
struct Cur {
	cursor: Cursor,
}

#[repr(C)]
struct Fnt {
	dpy: *mut Display,
	h: u32,
	xfont: *mut XftFont,
    pattern: *mut FcPattern,
    next: *mut Fnt,
}

type Clr = XftColor;
#[repr(C)]
pub struct Drw  {
	w: u32, h: u32,
	dpy: *mut Display,
	screen: i32,
	root: Window,
	drawable: Drawable,
	gc: GC,
	scheme: *mut Clr,
	fonts: *mut Fnt,
}

const COL_FG: usize = 0;
const COL_BG: usize = 1;
const COL_BORDER: usize = 2;

impl Drw {
    fn rect(&self, x: i32, y: i32, w: u32, h: u32, filled: bool, invert: bool) {
        unsafe {
            let colscheme = slice::from_raw_parts(self.scheme, 3);
            XSetForeground(self.dpy, self.gc, if invert {colscheme[COL_BG].pixel} else {colscheme[COL_FG].pixel});
            if filled {
                XFillRectangle(self.dpy, self.drawable, self.gc, x, y, w, h);
            }
            else {
                XDrawRectangle(self.dpy, self.drawable, self.gc, x, y, w - 1, h - 1);
            }
        }
    }
}

#[no_mangle]
unsafe extern "C" fn drw_rect(drw: *mut Drw, x: i32, y: i32, w: u32, h: u32, filled: i32, invert: i32)
{
    println!("draw rect");
	if drw == std::ptr::null_mut() || (*drw).scheme == std::ptr::null_mut() {
		return;
    }
    let drw = &mut *drw;
    drw.rect(x, y, w, h, filled != 0, invert != 0);
    println!("finish rect");
}