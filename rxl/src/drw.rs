use std::{slice};

use x11::{xlib::{XSetForeground, XDrawRectangle, Display, Window, Drawable, GC, Cursor, XFillRectangle, XCreateFontCursor, XFreeCursor}, xft::{XftFont, FcPattern, XftColor, XftTextExtentsUtf8}, xrender::XGlyphInfo};

#[repr(C)]
pub struct Cur {
	cursor: Cursor,
}

#[repr(C)]
pub struct Fnt {
	dpy: *mut Display,
	h: u32,
	xfont: *mut XftFont,
    pattern: *mut FcPattern,
    next: *mut Fnt,
}

impl Fnt {
    fn get_extents(&self, text: &str) -> Option<(u32, u32)> {
        let mut ext = XGlyphInfo{
            width: 0, height: 0, x: 0, y: 0, xOff: 0, yOff: 0,
        };

        if text.len() == 0 {
            return None
        }
    
        unsafe {
            XftTextExtentsUtf8(
            self.dpy,
            self.xfont,
            text.as_ptr(),
            text.len() as i32,
            &mut ext as *mut XGlyphInfo);
        }
        Some((ext.xOff as u32, self.h as u32))
    }
}

#[no_mangle]
pub extern "C" fn drw_font_getexts(font: *mut Fnt, text: *const i8, len: u32, w: *mut u32, h: *mut u32)
{
    let fnt = unsafe { & *font };
    let text = unsafe {
        let text = std::slice::from_raw_parts(text, len as usize);
        match std::str::from_utf8(std::mem::transmute(text)) {
            Ok(text) => text,
            _ => return
        }
    };
    if let Some((ww, hh)) = fnt.get_extents(text) {
        unsafe {
            if w != std::ptr::null_mut() {
                *w = ww;
            }
            if h != std::ptr::null_mut() {
                *h = hh;
            }
        }
    }
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
	if drw == std::ptr::null_mut() || (*drw).scheme == std::ptr::null_mut() {
		return;
    }
    let drw = &mut *drw;
    drw.rect(x, y, w, h, filled != 0, invert != 0);
}

#[no_mangle]
pub extern "C" fn drw_cur_create(drw: *mut Drw, shape: i32) -> *mut Cur {
	if drw == std::ptr::null_mut() {
        return std::ptr::null_mut()
    }
    let mut cur: Box<Cur> = Box::new(Cur{cursor: Default::default()});
    cur.cursor = unsafe { XCreateFontCursor((*drw).dpy, shape as u32) };
    Box::into_raw(cur)
}

#[no_mangle]
pub extern "C" fn drw_cur_free(drw: *mut Drw, cursor: *mut Cur)
{
	if cursor == std::ptr::null_mut() {
		return
    }

	unsafe {
        XFreeCursor((*drw).dpy, (*cursor).cursor);
        Box::from_raw(cursor);
    }
}
