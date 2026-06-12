use std::ffi::c_void;

use libc::pid_t;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_foundation::ns_string;

type AXUIElementRef = *mut c_void;
type AXValueRef = *mut c_void;
type CFTypeRef = *const c_void;
type AXError = i32;

const K_AX_SUCCESS: AXError = 0;
const K_AX_VALUE_CGPOINT: u32 = 1;
const K_AX_VALUE_CGSIZE: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
struct RawPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawSize {
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TilePosition {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    LeftThird,
    CenterThird,
    RightThird,
    Maximize,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateApplication(pid: pid_t) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attr: *const c_void,
        out: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attr: *const c_void,
        value: CFTypeRef,
    ) -> AXError;
    fn AXValueCreate(ty: u32, value: *const c_void) -> AXValueRef;
    fn AXValueGetValue(v: AXValueRef, ty: u32, out: *mut c_void) -> bool;
}

#[allow(clashing_extern_declarations)]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn rect_for(pos: &TilePosition, vf: Rect) -> Rect {
    let hw = vf.w / 2.0;
    let hh = vf.h / 2.0;
    let tw = vf.w / 3.0;
    match pos {
        TilePosition::LeftHalf => Rect {
            x: vf.x,
            y: vf.y,
            w: hw,
            h: vf.h,
        },
        TilePosition::RightHalf => Rect {
            x: vf.x + hw,
            y: vf.y,
            w: hw,
            h: vf.h,
        },
        // In Cocoa coords +y is up, so "top" half lives at higher y
        TilePosition::TopHalf => Rect {
            x: vf.x,
            y: vf.y + hh,
            w: vf.w,
            h: hh,
        },
        TilePosition::BottomHalf => Rect {
            x: vf.x,
            y: vf.y,
            w: vf.w,
            h: hh,
        },
        TilePosition::TopLeft => Rect {
            x: vf.x,
            y: vf.y + hh,
            w: hw,
            h: hh,
        },
        TilePosition::TopRight => Rect {
            x: vf.x + hw,
            y: vf.y + hh,
            w: hw,
            h: hh,
        },
        TilePosition::BottomLeft => Rect {
            x: vf.x,
            y: vf.y,
            w: hw,
            h: hh,
        },
        TilePosition::BottomRight => Rect {
            x: vf.x + hw,
            y: vf.y,
            w: hw,
            h: hh,
        },
        TilePosition::LeftThird => Rect {
            x: vf.x,
            y: vf.y,
            w: tw,
            h: vf.h,
        },
        TilePosition::CenterThird => Rect {
            x: vf.x + tw,
            y: vf.y,
            w: tw,
            h: vf.h,
        },
        TilePosition::RightThird => Rect {
            x: vf.x + tw * 2.0,
            y: vf.y,
            w: tw,
            h: vf.h,
        },
        TilePosition::Maximize => vf,
    }
}

/// Tile the focused window of `pid` to `pos`. Returns false on hard failure.
/// Must be called from the main thread.
pub fn tile_focused_window(pid: pid_t, pos: &TilePosition) -> bool {
    // kAXFocusedWindowAttribute etc. are #define CFSTR(...) macros, not linked symbols.
    // Cast &NSString to *const c_void — toll-free bridged with CFStringRef.
    let attr_focused_win = ns_string!("AXFocusedWindow") as *const _ as *const c_void;
    let attr_position = ns_string!("AXPosition") as *const _ as *const c_void;
    let attr_size = ns_string!("AXSize") as *const _ as *const c_void;

    unsafe {
        let app_elem = AXUIElementCreateApplication(pid);
        if app_elem.is_null() {
            return false;
        }

        // Get the focused window element
        let mut win_ref: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(app_elem, attr_focused_win, &mut win_ref);
        CFRelease(app_elem as CFTypeRef);
        if err != K_AX_SUCCESS || win_ref.is_null() {
            return false;
        }
        let win = win_ref as AXUIElementRef;

        // Read current window position (AX coords: top-left origin, y downward)
        let mut pos_ref: CFTypeRef = std::ptr::null();
        if AXUIElementCopyAttributeValue(win, attr_position, &mut pos_ref) != K_AX_SUCCESS
            || pos_ref.is_null()
        {
            CFRelease(win as CFTypeRef);
            return false;
        }
        let mut raw_pos = RawPoint { x: 0.0, y: 0.0 };
        AXValueGetValue(
            pos_ref as AXValueRef,
            K_AX_VALUE_CGPOINT,
            &mut raw_pos as *mut RawPoint as *mut c_void,
        );
        CFRelease(pos_ref);

        // Read current window size for center calculation
        let mut sz_ref: CFTypeRef = std::ptr::null();
        let mut raw_sz = RawSize {
            width: 0.0,
            height: 0.0,
        };
        if AXUIElementCopyAttributeValue(win, attr_size, &mut sz_ref) == K_AX_SUCCESS
            && !sz_ref.is_null()
        {
            AXValueGetValue(
                sz_ref as AXValueRef,
                K_AX_VALUE_CGSIZE,
                &mut raw_sz as *mut RawSize as *mut c_void,
            );
            CFRelease(sz_ref);
        }

        // Window center in AX coords
        let cx = raw_pos.x + raw_sz.width / 2.0;
        let cy_ax = raw_pos.y + raw_sz.height / 2.0;

        // Find the target screen via NSScreen (main thread required)
        let mtm = MainThreadMarker::new().expect("must be on main thread");
        let screens = NSScreen::screens(mtm);
        let count = screens.len();

        // Primary screen height for AX ↔ Cocoa coordinate flip
        // AX y = primary_h - (cocoa_y + h); Cocoa y = primary_h - ax_y - h
        // Safety: NSScreen array is not mutated during this function call
        let primary_h = if count > 0 {
            screens.objectAtIndex_unchecked(0).frame().size.height
        } else {
            768.0
        };

        // Convert AX window center to Cocoa coords (bottom-left origin, y upward)
        let cy_cocoa = primary_h - cy_ax;

        let mut target_vf = None;
        for i in 0..count {
            let s = screens.objectAtIndex_unchecked(i);
            let f = s.frame();
            if cx >= f.origin.x
                && cx < f.origin.x + f.size.width
                && cy_cocoa >= f.origin.y
                && cy_cocoa < f.origin.y + f.size.height
            {
                target_vf = Some(s.visibleFrame());
                break;
            }
        }
        // Fall back to primary screen
        if target_vf.is_none() && count > 0 {
            target_vf = Some(screens.objectAtIndex_unchecked(0).visibleFrame());
        }

        let vf_ns = match target_vf {
            Some(r) => r,
            None => {
                CFRelease(win as CFTypeRef);
                return false;
            }
        };

        let vf = Rect {
            x: vf_ns.origin.x,
            y: vf_ns.origin.y,
            w: vf_ns.size.width,
            h: vf_ns.size.height,
        };

        let target = rect_for(pos, vf);

        // Flip target Cocoa rect to AX coords
        let ax_y = primary_h - (target.y + target.h);
        let new_pos = RawPoint {
            x: target.x,
            y: ax_y,
        };
        let new_sz = RawSize {
            width: target.w,
            height: target.h,
        };

        let sz_val = AXValueCreate(
            K_AX_VALUE_CGSIZE,
            &new_sz as *const RawSize as *const c_void,
        );
        let pos_val = AXValueCreate(
            K_AX_VALUE_CGPOINT,
            &new_pos as *const RawPoint as *const c_void,
        );

        // Set size → position → size (double-set defeats per-app min-size clamping)
        AXUIElementSetAttributeValue(win, attr_size, sz_val as CFTypeRef);
        AXUIElementSetAttributeValue(win, attr_position, pos_val as CFTypeRef);
        AXUIElementSetAttributeValue(win, attr_size, sz_val as CFTypeRef);

        CFRelease(sz_val as CFTypeRef);
        CFRelease(pos_val as CFTypeRef);
        CFRelease(win as CFTypeRef);

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VF: Rect = Rect {
        x: 0.0,
        y: 23.0,
        w: 1920.0,
        h: 1057.0,
    };

    #[test]
    fn halves_cover_full_area() {
        let l = rect_for(&TilePosition::LeftHalf, VF);
        let r = rect_for(&TilePosition::RightHalf, VF);
        assert!((l.w + r.w - VF.w).abs() < 0.001);
        assert_eq!(l.x, VF.x);
        assert!((l.x + l.w - r.x).abs() < 0.001);

        let t = rect_for(&TilePosition::TopHalf, VF);
        let b = rect_for(&TilePosition::BottomHalf, VF);
        assert!((t.h + b.h - VF.h).abs() < 0.001);
        assert!((b.y + b.h - t.y).abs() < 0.001);
    }

    #[test]
    fn quarters_tile_without_overlap() {
        let tl = rect_for(&TilePosition::TopLeft, VF);
        let tr = rect_for(&TilePosition::TopRight, VF);
        let bl = rect_for(&TilePosition::BottomLeft, VF);
        let br = rect_for(&TilePosition::BottomRight, VF);
        assert!((tl.w + tr.w - VF.w).abs() < 0.001);
        assert!((tl.h + bl.h - VF.h).abs() < 0.001);
        assert!((tl.x + tl.w - tr.x).abs() < 0.001);
        assert!((bl.x + bl.w - br.x).abs() < 0.001);
        // top row sits above bottom row
        assert!((tl.y - (bl.y + bl.h)).abs() < 0.001);
    }

    #[test]
    fn thirds_split_width_into_3() {
        let l = rect_for(&TilePosition::LeftThird, VF);
        let c = rect_for(&TilePosition::CenterThird, VF);
        let r = rect_for(&TilePosition::RightThird, VF);
        assert!((l.w + c.w + r.w - VF.w).abs() < 0.001);
        assert!((l.x + l.w - c.x).abs() < 0.001);
        assert!((c.x + c.w - r.x).abs() < 0.001);
    }

    #[test]
    fn maximize_equals_visible_frame() {
        assert_eq!(rect_for(&TilePosition::Maximize, VF), VF);
    }
}
