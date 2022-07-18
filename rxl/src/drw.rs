use std::{slice};

use x11::{xlib::{XSetForeground, XDrawRectangle, Display, Window, Drawable, GC, Cursor, XFillRectangle, XCreateFontCursor, XFreeCursor, XDefaultVisual, XDefaultColormap}, xft::{XftFont, FcPattern, XftColor, XftTextExtentsUtf8, XftDraw, XftDrawCreate}, xrender::XGlyphInfo};

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

#[no_mangle]
unsafe extern "C" fn drw_text(drw: *mut Drw, mut x: i32, y: i32, mut w: u32, h: u32, lpad: u32, text: *const i8, invert: i32) -> i32 {
	let mut buf: [i8; 1024];
    let render = x != 0 || y != 0 || w != 0 || h != 0;
    let mut d: *mut XftDraw = std::ptr::null_mut();
    let mut utf8codepoint = 0;
    let mut charexists = 0;
	//int ty;
	//unsigned int ew;
	//Fnt *usedfont, *curfont, *nextfont;
	//size_t i, len;
	//int utf8strlen, utf8charlen;
	//const char *utf8str;
	//FcCharSet *fccharset;
	//FcPattern *fcpattern;
	//FcPattern *match;
	//XftResult result;

	if drw == std::ptr::null_mut() ||
        (render && (*drw).scheme == std::ptr::null_mut()) ||
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
	loop {
		let mut utf8strlen = 0;
		let utf8str = text;
		let nextfont = std::ptr::null_mut();
        let utf8str = slice::from_raw_parts(utf8str, 256);
        let utf8str = std::str::from_utf8_unchecked(std::mem::transmute(utf8str));
		for ch in utf8str.chars() {
			for (curfont = drw->fonts; curfont; curfont = curfont->next) {
				charexists = charexists || XftCharExists(drw->dpy, curfont->xfont, utf8codepoint);
				if (charexists) {
					if (curfont == usedfont) {
						utf8strlen += utf8charlen;
						text += utf8charlen;
					} else {
						nextfont = curfont;
					}
					break;
				}
			}

			if (!charexists || nextfont)
				break;
			else
				charexists = 0;
		}

		if (utf8strlen) {
			drw_font_getexts(usedfont, utf8str, utf8strlen, &ew, NULL);
			/* shorten text if necessary */
			for (len = MIN(utf8strlen, sizeof(buf) - 1); len && ew > w; len--)
				drw_font_getexts(usedfont, utf8str, len, &ew, NULL);

			if (len) {
				memcpy(buf, utf8str, len);
				buf[len] = '\0';
				if (len < utf8strlen)
					for (i = len; i && i > len - 3; buf[--i] = '.')
						; /* NOP */

				if (render) {
					ty = y + (h - usedfont->h) / 2 + usedfont->xfont->ascent;
					XftDrawStringUtf8(d, &drw->scheme[invert ? ColBg : ColFg],
					                  usedfont->xfont, x, ty, (XftChar8 *)buf, len);
				}
				x += ew;
				w -= ew;
			}
		}

		if (!*text) {
			break;
		} else if (nextfont) {
			charexists = 0;
			usedfont = nextfont;
		} else {
			/* Regardless of whether or not a fallback font is found, the
			 * character must be drawn. */
			charexists = 1;

			fccharset = FcCharSetCreate();
			FcCharSetAddChar(fccharset, utf8codepoint);

			if (!drw->fonts->pattern) {
				/* Refer to the comment in xfont_create for more information. */
				die("the first font in the cache must be loaded from a font string.");
			}

			fcpattern = FcPatternDuplicate(drw->fonts->pattern);
			FcPatternAddCharSet(fcpattern, FC_CHARSET, fccharset);
			FcPatternAddBool(fcpattern, FC_SCALABLE, FcTrue);
			FcPatternAddBool(fcpattern, FC_COLOR, FcFalse);

			FcConfigSubstitute(NULL, fcpattern, FcMatchPattern);
			FcDefaultSubstitute(fcpattern);
			match = XftFontMatch(drw->dpy, drw->screen, fcpattern, &result);

			FcCharSetDestroy(fccharset);
			FcPatternDestroy(fcpattern);

			if (match) {
				usedfont = xfont_create(drw, NULL, match);
				if (usedfont && XftCharExists(drw->dpy, usedfont->xfont, utf8codepoint)) {
					for (curfont = drw->fonts; curfont->next; curfont = curfont->next)
						; /* NOP */
					curfont->next = usedfont;
				} else {
					xfont_free(usedfont);
					usedfont = drw->fonts;
				}
			}
		}
	}
	if (d)
		XftDrawDestroy(d);

	return x + (render ? w : 0);
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
