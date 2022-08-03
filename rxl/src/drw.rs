use std::{slice};

use x11::{
	xlib::{XSetForeground, XDrawRectangle, Display, Window, Drawable, GC, Cursor, XFillRectangle, XCreateFontCursor, XFreeCursor, XDefaultVisual, XDefaultColormap},
	xft::{XftFont, FcPattern, XftColor, XftTextExtentsUtf8, XftDraw, XftDrawCreate, XftCharExists, XftDrawDestroy, XftFontMatch, XftDrawStringUtf8, FcResult},
	xrender::XGlyphInfo
};
use fontconfig_sys::{FcBool, FcChar32, FcChar8, FcCharSet, FcCharSetAddChar, FcCharSetCreate, FcCharSetDestroy, FcConfigSubstitute, FcDefaultSubstitute, FcMatchPattern, FcNameParse, FcPatternAddBool, FcPatternAddCharSet, FcPatternDestroy, FcPatternDuplicate, FcPatternGetBool, FcResultMatch, FcTypeBool};

use fontconfig_sys::constants::{FC_CHARSET, FC_COLOR, FC_SCALABLE};
use x11::xft::{XftColorAllocName, XftFontClose, XftFontOpenName, XftFontOpenPattern};
use x11::xlib::{CapButt, False, JoinMiter, LineSolid, XCopyArea, XCreateGC, XCreatePixmap, XDefaultDepth, XFreeGC, XFreePixmap, XSetLineAttributes, XSync};
use x11::xrender::XRenderColor;

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
unsafe extern "C" fn drw_fontset_getwidth(drw: *mut Drw, text: *const i8) -> u32 {
	if drw == std::ptr::null_mut() || (*drw).fonts == std::ptr::null_mut() || text == std::ptr::null() {
		return 0;
    }
	return drw_text(drw, 0, 0, 0, 0, 0, text, 0) as u32;
}

const UTF_INVALID: i32 = 0xFFFD;
const UTF_SIZ: usize   = 4;

const utfbyte: [u8; UTF_SIZ + 1] = [0x80,  0, 0xC0, 0xE0, 0xF0];
const utfmask: [u8; UTF_SIZ + 1] = [0xC0, 0x80, 0xE0, 0xF0, 0xF8];
const utfmin: [i32; UTF_SIZ + 1] = [   0,    0,  0x80,  0x800,  0x10000];
const utfmax: [i32; UTF_SIZ + 1] = [0x10FFFF, 0x7F, 0x7FF, 0xFFFF, 0x10FFFF];

unsafe fn utf8decodebyte(c: i8, i: *mut usize) -> i32 {
	(*i) = 0;
	while *i < (UTF_SIZ + 1) {
		if (c as u8 & utfmask[*i]) == utfbyte[*i] {
			return (c as u8 & !utfmask[*i]) as i32;
		}
		(*i) += 1;
	}
	return 0;
}

fn between<T: PartialOrd>(X: T, A: T, B: T) -> bool {
	(A) <= (X) && (X) <= (B)
}

unsafe fn utf8validate(u: *mut i32, mut i: usize) -> usize {
	if !between(*u, utfmin[i], utfmax[i]) || between(*u, 0xD800, 0xDFFF) {
		*u = UTF_INVALID;
	}
	i = 1;
	while *u > utfmax[i] {
		i += 1;
	}
	return i;
}

unsafe fn utf8decode(c: *const i8, u: *mut i32, clen: usize) -> usize {
	let mut i: usize = 0;
	let mut j: usize = 0;
	let mut len: usize = 0;
	let mut typ: usize = 0;
	let mut udecoded: i32 = 0;

	*u = UTF_INVALID;
	if clen == 0 {
		return 0;
	}
	udecoded = utf8decodebyte(*c.offset(0), &mut len);
	if !between(len, 1, UTF_SIZ) {
		return 1;
	}
	i = 1;
	j = 1;
	while i < clen && j < len {
		udecoded = (udecoded << 6) | utf8decodebyte(*c.offset(i as isize), &mut typ);
		if typ != 0 {
			return j;
		}
		i += 1;
		j += 1;
	}
	if j < len {
		return 0;
	}
	*u = udecoded;
	utf8validate(u, len);
	return len;
}

#[no_mangle]
unsafe fn drw_text(drw: *mut Drw, mut x: i32, mut y: i32, mut w: u32, mut h: u32, lpad: u32, mut text: *const i8, invert: i32) -> i32 {
	let mut buf = [0i8; 1024];
	let mut ty = 0;
	let mut ew = 0u32;
	let mut d: *mut XftDraw = std::ptr::null_mut();
	let mut usedfont: *mut Fnt = std::ptr::null_mut();
	let mut curfont: *mut Fnt = std::ptr::null_mut();
	let mut nextfont: *mut Fnt = std::ptr::null_mut();
	let mut i = 0usize;
	let mut len = 0usize;
	let mut utf8strlen = 0;
	let mut utf8charlen = 0;
	let render = x != 0 || y != 0 || w != 0 || h != 0;
	let mut utf8codepoint = 0;
	let mut utf8str: *const i8 = std::ptr::null_mut();
	let mut fccharset = std::ptr::null_mut();
	let mut fcpattern = std::ptr::null_mut();
	let mut matc = std::ptr::null_mut();
	let mut result = FcResult::NoId;
	let mut charexists = false;

	if drw.is_null() || (render && (*drw).scheme.is_null()) || text.is_null() || (*drw).fonts.is_null() {
		return 0;
	}
	if !render {
		w = !w;
	} else {
		XSetForeground((*drw).dpy, (*drw).gc, (*(*drw).scheme.offset(if invert != 0 { ColFg } else { ColBg } as isize)).pixel);
		XFillRectangle((*drw).dpy, (*drw).drawable, (*drw).gc, x, y, w, h);
		d = XftDrawCreate((*drw).dpy, (*drw).drawable,
		XDefaultVisual((*drw).dpy, (*drw).screen),
		XDefaultColormap((*drw).dpy, (*drw).screen));
		x += lpad as i32;
		w -= lpad;
	}
	usedfont = (*drw).fonts;
	loop {
		utf8strlen = 0;
		utf8str = text;
		nextfont = std::ptr::null_mut();
		while (*text) != 0 {
			utf8charlen = utf8decode(text, &mut utf8codepoint, UTF_SIZ);
			curfont = (*drw).fonts;
			while !curfont.is_null() {
				charexists = charexists || XftCharExists((*drw).dpy, (*curfont).xfont, utf8codepoint as u32) != 0;
				if charexists {
					if curfont == usedfont {
						utf8strlen += utf8charlen;
						text = text.add(utf8charlen);
					} else {
						nextfont = curfont;
					}
					break;
				}
				curfont = (*curfont).next
			}

			if !charexists || !nextfont.is_null() {
				break;
			} else {
				charexists = false;
			}
		}

		if utf8strlen != 0 {
			drw_font_getexts(usedfont, utf8str, utf8strlen as u32, &mut ew, std::ptr::null_mut());
			/* shorten text if necessary */
			len = utf8strlen.min(buf.len() - 1);
			while len != 0 && ew > w {
				drw_font_getexts(usedfont, utf8str, len as u32, &mut ew, std::ptr::null_mut());
				len -= 1;
			}

			if len != 0 {
				for ind in 0..len { buf[ind] = *utf8str.offset(ind as isize); }
				buf[len] = '\0' as i8;
				if len < utf8strlen {
					i = len;
					while i != 0 && i > len - 3 {
						i -= 1;
						buf[i] = '.' as i8;
					}
				}
				if render {
					ty = y + (h - (*usedfont).h) as i32 / 2 + (*(*usedfont).xfont).ascent;
					XftDrawStringUtf8(d, (*drw).scheme.offset(if invert != 0 { ColBg } else { ColFg } as isize),
					  (*usedfont).xfont, x, ty, buf.as_ptr() as *const u8, len as i32);
				}
				x += ew as i32;
				w -= ew;
			}
		}
		if (*text) == 0 {
			break;
		} else if !nextfont.is_null() {
			charexists = false;
			usedfont = nextfont;
		} else {
			/* Regardless of whether or not a fallback font is found, the
			 * character must be drawn. */
			charexists = true;

			fccharset = FcCharSetCreate();
			FcCharSetAddChar(fccharset, utf8codepoint as FcChar32);

			if (*(*drw).fonts).pattern.is_null() {
				/* Refer to the comment in xfont_create for more information. */
				panic!("the first font in the cache must be loaded from a font string.");
			}

			fcpattern = FcPatternDuplicate((*(*drw).fonts).pattern as *const fontconfig_sys::FcPattern);
			FcPatternAddCharSet(fcpattern, FC_CHARSET.as_ptr(), fccharset);
			FcPatternAddBool(fcpattern, FC_SCALABLE.as_ptr(), 1);
			FcPatternAddBool(fcpattern, FC_COLOR.as_ptr(), 0);

			FcConfigSubstitute(std::ptr::null_mut(), fcpattern, FcMatchPattern);
			FcDefaultSubstitute(fcpattern);
			matc = XftFontMatch((*drw).dpy, (*drw).screen, fcpattern as *const FcPattern, &mut result);

			FcCharSetDestroy(fccharset);
			FcPatternDestroy(fcpattern);

			if !matc.is_null() {
				usedfont = xfont_create(drw, std::ptr::null_mut(), matc);
				if !usedfont.is_null() && XftCharExists((*drw).dpy, (*usedfont).xfont, utf8codepoint as u32) != 0 {
					curfont = (*drw).fonts;
					while !(*curfont).next.is_null() {
						curfont = (*curfont).next;
					}
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

	return x + (if render { w as i32 } else { 0 });
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
	fn create(dpy: *mut Display, screen: i32, root: Window, w: u32, h: u32) -> Box<Drw> {
		unsafe {
			let mut drw = Box::new(Drw {
				dpy,
				screen,
				root,
				w,
				h,
				drawable: XCreatePixmap(dpy, root, w, h, XDefaultDepth(dpy, screen) as u32),
				gc: XCreateGC(dpy, root, 0, std::ptr::null_mut()),
				fonts: std::ptr::null_mut(),
				scheme: std::ptr::null_mut(),
			});
			XSetLineAttributes(dpy, drw.gc, 1, LineSolid, CapButt, JoinMiter);
			drw
		}
	}
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

impl Drop for Drw {
	fn drop(&mut self) {
		unsafe {
			XFreePixmap(self.dpy, self.drawable);
			XFreeGC(self.dpy, self.gc);
			drw_fontset_free(self.fonts);
		}
	}
}

#[no_mangle]
fn drw_create(dpy: *mut Display, screen: i32, root: Window, w: u32, h: u32) -> *mut Drw {
	let drw = Drw::create(dpy, screen, root, w, h);
	Box::into_raw(drw)
}

#[no_mangle]
unsafe fn drw_free(drw: *mut Drw) {
	let drw = Box::from_raw(drw);
	drop(drw);
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
unsafe fn drw_rect(drw: *mut Drw, x: i32, y: i32, w: u32, h: u32, filled: i32, invert: i32) {
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
pub fn drw_cur_free(drw: *mut Drw, cursor: *mut Cur) {
	if cursor == std::ptr::null_mut() {
		return
    }
	unsafe {
        XFreeCursor((*drw).dpy, (*cursor).cursor);
        Box::from_raw(cursor);
    }
}

#[no_mangle]
unsafe fn xfont_create(drw: *mut Drw, fontname: *const i8, fontpattern: *mut FcPattern) -> *mut Fnt {
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
unsafe fn xfont_free(font: *mut Fnt) {
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
unsafe fn drw_fontset_create(drw: *mut Drw, font: *mut *const i8, fontcount: usize) -> *mut Fnt {
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

#[no_mangle]
fn drw_setscheme(drw: *mut Drw, scm: *mut Clr) {
	if !drw.is_null() {
		unsafe { (*drw).scheme = scm };
	}
}

#[no_mangle]
unsafe fn drw_map(drw: *mut Drw, win: Window, x: i32, y: i32, w: u32, h: u32)
{
	if drw.is_null() {
		return;
	}
	XCopyArea((*drw).dpy, (*drw).drawable, win, (*drw).gc, x, y, w, h, x, y);
	XSync((*drw).dpy, False);
}

unsafe fn drw_clr_create(drw: *mut Drw, dest: *mut Clr, clrname: *const i8) {
	if drw.is_null() || dest.is_null() || clrname.is_null() {
		return;
	}
	if XftColorAllocName((*drw).dpy, XDefaultVisual((*drw).dpy, (*drw).screen),
	XDefaultColormap((*drw).dpy, (*drw).screen),
	clrname, dest) == 0 {
		panic!("error, cannot allocate color '{:?}'", clrname);
	}
}

/* Wrapper to create color schemes. The caller has to call free(3) on the
 * returned color scheme when done using it. */
#[no_mangle]
unsafe fn drw_scm_create(drw: *mut Drw, clrnames: *mut *const i8, clrcount: usize) -> *mut Clr
{
	if drw.is_null() || clrnames.is_null() || clrcount < 2 {
		return std::ptr::null_mut();
	}
	/* need at least two colors for a scheme */
	let clrnames = slice::from_raw_parts(clrnames, clrcount);
	let mut ret: Vec<Clr> = Vec::new();
	ret.resize(clrcount, Clr{ pixel: 0, color: XRenderColor{ red: 0, green: 0, blue: 0, alpha: 0 } });

	for (i, clrname) in clrnames.iter().enumerate() {
		drw_clr_create(drw, &mut ret[i], *clrname);
	}
	let res_ptr = ret.as_mut_ptr();
	std::mem::forget(ret);
	res_ptr
}