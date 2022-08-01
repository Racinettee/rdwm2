use std::{slice};

use x11::{
	xlib::{XSetForeground, XDrawRectangle, Display, Window, Drawable, GC, Cursor, XFillRectangle, XCreateFontCursor, XFreeCursor, XDefaultVisual, XDefaultColormap},
	xft::{XftFont, FcPattern, XftColor, XftTextExtentsUtf8, XftDraw, XftDrawCreate, XftCharExists, XftDrawDestroy, XftFontMatch, XftDrawStringUtf8, FcResult},
	xrender::XGlyphInfo
};
use fontconfig_sys::{FcBool, FcChar8, FcCharSetAddChar, FcCharSetCreate, FcCharSetDestroy, FcConfigSubstitute, FcDefaultSubstitute, FcMatchPattern, FcNameParse, FcPatternAddBool, FcPatternAddCharSet, FcPatternDestroy, FcPatternDuplicate, FcPatternGetBool, FcResultMatch, FcTypeBool};

use fontconfig_sys::constants::{FC_CHARSET, FC_COLOR, FC_SCALABLE};
use x11::xft::{XftFontClose, XftFontOpenName, XftFontOpenPattern};
use x11::xlib::{CapButt, JoinMiter, LineSolid, XCreateGC, XCreatePixmap, XDefaultDepth, XFreeGC, XFreePixmap, XSetLineAttributes};

const ColFg: i32 = 0;
const ColBg: i32 = 1;
const ColBorder: i32 = 2;

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

#[no_mangle]
unsafe extern "C" fn __drw_fontset_getwidth(drw: *mut Drw, text: *const i8) -> u32 {
	if drw == std::ptr::null_mut() || (*drw).fonts == std::ptr::null_mut() || text == std::ptr::null() {
		return 0;
    }
	return _drw_text(drw, 0, 0, 0, 0, 0, text, 0) as u32;
}

#[no_mangle]
unsafe extern "C" fn _drw_text(drw: *mut Drw, mut x: i32, y: i32, mut w: u32, h: u32, lpad: u32, text: *const i8, invert: i32) -> i32 {
	let mut buf = [0; 1024];
    let render = x != 0 || y != 0 || w != 0 || h != 0;
    let mut d: *mut XftDraw = std::ptr::null_mut();
    let mut utf8codepoint = 0;
    let mut charexists = 0;
	let mut ty;
	let mut ew = 0u32;
	//Fnt *usedfont, *curfont, *nextfont;
	//size_t i, len;
	//const char *utf8str;
	//FcPattern *match;
	//XftResult result;

	if drw == std::ptr::null_mut() || (render && (*drw).scheme == std::ptr::null_mut()) ||
        text == std::ptr::null() || (*drw).fonts == std::ptr::null_mut() {
        return 0;
    }

	if !render {
		w = !w;
	} else {
        let colscheme = if invert != 0 { COL_FG } else { COL_BG };
		XSetForeground((*drw).dpy, (*drw).gc, (*(*drw).scheme.offset(colscheme.try_into().unwrap())).pixel);
		XFillRectangle((*drw).dpy, (*drw).drawable, (*drw).gc, x, y, w, h);
		d = XftDrawCreate((*drw).dpy, (*drw).drawable,
		                  XDefaultVisual((*drw).dpy, (*drw).screen),
		                  XDefaultColormap((*drw).dpy, (*drw).screen));
		x += lpad as i32;
		w -= lpad;
	}
    let mut usedfont = (*drw).fonts;
	let mut curfont = std::ptr::null_mut();
	let utf8str = text;
	let utf8str = slice::from_raw_parts(utf8str, 256);
	let utf8str = std::str::from_utf8_unchecked(std::mem::transmute(utf8str));
	loop {

		let mut nextfont = std::ptr::null_mut();
		for ch in utf8str.chars() {
			utf8codepoint = ch as u32;
			curfont = (*drw).fonts;
			while curfont != std::ptr::null_mut() {
				charexists = (charexists != 0 || XftCharExists((*drw).dpy, (*curfont).xfont, utf8codepoint) != 0) as i32;
				// If the character exists in one of the fonts we're using we reak out of this loop
				if charexists != 0 {
					if curfont != usedfont {
						nextfont = curfont;
					}
					break;
				}
				curfont = (*curfont).next;
			}

			if charexists == 0 || nextfont != std::ptr::null_mut() {
				break;
			} else {
				charexists = 0;
			}
		}

		if utf8str.len() != 0 {
			drw_font_getexts(usedfont, utf8str.as_ptr() as *const i8, utf8str.len() as u32, &mut ew as *mut u32, std::ptr::null_mut());
			/* shorten text if necessary */
			let mut len = utf8str.len().min(buf.len());
			while len != 0 && ew > w {
				drw_font_getexts(usedfont, utf8str.as_ptr() as *const i8, len as u32, &mut ew, std::ptr::null_mut());
				len -= 1;
			}
			if len != 0 {
				for (dst, src) in buf.iter_mut().zip(utf8str.as_bytes()) { *dst = *src; }
				buf[len] = '\0' as u8;
				if len < utf8str.len() {
					buf[len-3..len].fill('.' as u8);
				}
				if render {
					ty = y + (h as i32 - (*usedfont).h as i32) / 2 + (*(*usedfont).xfont).ascent;
					XftDrawStringUtf8(d, (*drw).scheme.offset(if invert != 0 {ColBg}else{ColFg} as isize),
									  (*usedfont).xfont, x, ty, buf.as_ptr(), len as i32);
				}
				x += ew as i32;
				w -= ew;
			}
		}

		if *utf8str.as_ptr().offset((utf8str.len()-1) as isize) == 0 {
			break;
		} else if nextfont != std::ptr::null_mut() {
			charexists = 0;
			usedfont = nextfont;
		} else {
			/* Regardless of whether or not a fallback font is found, the
			 * character must be drawn. */
			charexists = 1;

			let fccharset = FcCharSetCreate();
			FcCharSetAddChar(fccharset, utf8codepoint);

			if (*(*drw).fonts).pattern == std::ptr::null_mut() {
				/* Refer to the comment in xfont_create for more information. */
				panic!("the first font in the cache must be loaded from a font string.");
			}

			let fcpattern = FcPatternDuplicate((*(*drw).fonts).pattern as *const fontconfig_sys::FcPattern);
			FcPatternAddCharSet(fcpattern, FC_CHARSET.as_ptr(), fccharset);
			FcPatternAddBool(fcpattern, FC_SCALABLE.as_ptr(), 1);
			FcPatternAddBool(fcpattern, FC_COLOR.as_ptr(), 0);

			FcConfigSubstitute(std::ptr::null_mut(), fcpattern, FcMatchPattern);
			FcDefaultSubstitute(fcpattern);
			let result: *mut FcResult = std::ptr::null_mut();
			let mtch = XftFontMatch((*drw).dpy, (*drw).screen, fcpattern as *const FcPattern, result);

			FcCharSetDestroy(fccharset);
			FcPatternDestroy(fcpattern);

			if !mtch.is_null() {
				usedfont = xfont_create(drw, std::ptr::null_mut(), mtch);
				if !usedfont.is_null() && XftCharExists((*drw).dpy, (*usedfont).xfont, utf8codepoint) != 0 {
					curfont = (*drw).fonts;
					while !(*curfont).next.is_null() { curfont = (*curfont).next; }
					(*curfont).next = usedfont;
				} else {
					xfont_free(usedfont);
					usedfont = (*drw).fonts;
				}
			}
		}
	}
	if !d.is_null() {
		XftDrawDestroy(d);
	}
	return x + if render { w as i32 } else { 0 };
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
unsafe fn drw_create(dpy: *mut Display, screen: i32, root: Window, w: u32, h: u32) -> *mut Drw {
	let mut drw = Box::new(Drw{
		dpy,
		screen,
		root,
		w, h,
		drawable: XCreatePixmap(dpy, root, w, h, XDefaultDepth(dpy, screen) as u32),
		gc: XCreateGC(dpy, root, 0, std::ptr::null_mut()),
		fonts: std::ptr::null_mut(),
		scheme: std::ptr::null_mut(),
	});
	XSetLineAttributes(dpy, drw.gc, 1, LineSolid, CapButt, JoinMiter);

	Box::into_raw(drw)
}

#[no_mangle]
unsafe fn drw_free(drw: *mut Drw) {
	let drw = Box::from_raw(drw);
	XFreePixmap(drw.dpy, drw.drawable);
	XFreeGC(drw.dpy, drw.gc);
	drw_fontset_free(drw.fonts);
}

#[no_mangle]
unsafe fn drw_resize(drw: *mut Drw, w: u32, h: u32) {
	if drw.is_null() {
		return;
	}
	(*drw).w = w;
	(*drw).h = h;
	if !(*drw).drawable != 0 {
		XFreePixmap((*drw).dpy, (*drw).drawable);
	}
	(*drw).drawable = XCreatePixmap((*drw).dpy, (*drw).root, w, h, XDefaultDepth((*drw).dpy, (*drw).screen) as u32);
}

#[no_mangle]
unsafe fn drw_rect(drw: *mut Drw, x: i32, y: i32, w: u32, h: u32, filled: i32, invert: i32)
{
	if drw == std::ptr::null_mut() || (*drw).scheme == std::ptr::null_mut() {
		return;
    }
    let drw = &mut *drw;
    drw.rect(x, y, w, h, filled != 0, invert != 0);
}

#[no_mangle]
pub fn drw_cur_create(drw: *mut Drw, shape: i32) -> *mut Cur {
	if drw == std::ptr::null_mut() {
        return std::ptr::null_mut()
    }
    let mut cur: Box<Cur> = Box::new(Cur{cursor: Default::default()});
    cur.cursor = unsafe { XCreateFontCursor((*drw).dpy, shape as u32) };
    Box::into_raw(cur)
}

#[no_mangle]
pub fn drw_cur_free(drw: *mut Drw, cursor: *mut Cur)
{
	if cursor == std::ptr::null_mut() {
		return
    }

	unsafe {
        XFreeCursor((*drw).dpy, (*cursor).cursor);
        Box::from_raw(cursor);
    }
}
#[no_mangle]
unsafe extern "C" fn xfont_create(drw: *mut Drw, fontname: *const i8, fontpattern: *mut FcPattern) -> *mut Fnt
{
	let mut xfont = std::ptr::null_mut();
	let mut pattern = std::ptr::null_mut();
	if !fontname.is_null() {
	/* Using the pattern found at font->xfont->pattern does not yield the
	 * same substitution results as using the pattern returned by
	 * FcNameParse; using the latter results in the desired fallback
	 * behaviour whereas the former just results in missing-character
	 * rectangles being drawn, at least with some fonts. */
		xfont = XftFontOpenName((*drw).dpy, (*drw).screen, fontname);
		if xfont.is_null() {
			eprintln!("error, cannot load font from name: {:?}", fontname);
			return std::ptr::null_mut();
		}
		pattern = FcNameParse(fontname as *const FcChar8);
		if pattern.is_null() {
			eprintln!("error, cannot parse font name to pattern: {:?}", fontname);
			XftFontClose((*drw).dpy, xfont);
			return std::ptr::null_mut();
		}
	} else if !fontpattern.is_null() {
		xfont = XftFontOpenPattern((*drw).dpy, fontpattern);
		if xfont.is_null() {
			eprintln!("error, cannot load font from pattern.");
			return std::ptr::null_mut();
		}
	} else {
		panic!("no font specified.");
	}

	/* Do not allow using color fonts. This is a workaround for a BadLength
	 * error from Xft with color glyphs. Modelled on the Xterm workaround. See
	 * https://bugzilla.redhat.com/show_bug.cgi?id=1498269
	 * https://lists.suckless.org/dev/1701/30932.html
	 * https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=916349
	 * and lots more all over the internet.
	 */
	let mut iscol: FcBool = 0;
	if FcPatternGetBool((*xfont).pattern as *mut fontconfig_sys::FcPattern, FC_COLOR.as_ptr(), 0, &mut iscol) == FcResultMatch && iscol != 0 {
		XftFontClose((*drw).dpy, xfont);
		return std::ptr::null_mut();
	}

	let mut font = Box::new(Fnt{
		xfont,
		pattern: pattern as *mut FcPattern,
		h: ((*xfont).ascent + (*xfont).descent) as u32,
		dpy: (*drw).dpy,
		next: std::ptr::null_mut()
	});

	return Box::into_raw(font);
}
#[no_mangle]
unsafe extern "C" fn xfont_free(font: *mut Fnt) {
	if font.is_null() {
		return;
	}
	let font = Box::from_raw(font);
	if font.pattern.is_null() {
		FcPatternDestroy(font.pattern as *mut fontconfig_sys::FcPattern);
	}
	XftFontClose(font.dpy, font.xfont);
}

#[no_mangle]
unsafe extern "C" fn drw_fontset_create(drw: *mut Drw, font: *mut *const i8, fontcount: usize) -> *mut Fnt {
	if drw.is_null() || font.is_null() {
		return std::ptr::null_mut();
	}
	let font = slice::from_raw_parts(font, fontcount);
	let mut ret = std::ptr::null_mut();

	for fnt in font.iter().rev() {
		let cur = xfont_create(drw, *fnt, std::ptr::null_mut());
		if !cur.is_null() {
			(*cur).next = ret;
			ret = cur;
		}
	}
	(*drw).fonts = ret;
	ret
}

unsafe fn drw_fontset_free(font: *mut Fnt) {
	if !font.is_null() {
		drw_fontset_free((*font).next);
		xfont_free(font);
	}
}