//! Where the reader's selection is on screen, on Windows: the bounds UI Automation gives
//! for the selected text of the element with the keyboard focus, or failing that the
//! caret the application shows. Both in physical pixels, as the Mini Window is placed.

use ::windows::Win32::Foundation::{HWND, POINT};
use ::windows::Win32::Graphics::Gdi::ClientToScreen;
use ::windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
    SAFEARRAY,
};
use ::windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetLBound, SafeArrayGetUBound,
    SafeArrayUnaccessData,
};
use ::windows::Win32::System::Variant::{VARIANT, VT_I4};
use ::windows::Win32::UI::Accessibility::{
    AccessibleObjectFromWindow, CUIAutomation8, IAccessible, IUIAutomation, IUIAutomation2,
    IUIAutomationTextPattern, UIA_TextPatternId,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CHILDID_SELF, GUITHREADINFO, GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId,
    OBJID_CARET,
};
use ::windows::core::Interface;

use super::placement::Area;

/// How long UI Automation may wait on the application in front, in milliseconds. One that
/// does not answer by then is placed by the pointer instead.
const PATIENCE_MS: u32 = 300;

/// The bounds of the selection in the application in front, or of its caret; `None`
/// when it tells neither.
pub fn selection_bounds() -> Option<Area> {
    selected_text_bounds().or_else(caret_bounds)
}

/// The union of the rectangles UI Automation gives for the selected text.
fn selected_text_bounds() -> Option<Area> {
    // SAFETY: COM is set up for this thread and torn down again only if this call set it
    // up; every interface is released when dropped, and the arrays are destroyed below.
    unsafe {
        let set_up = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let bounds = (|| -> ::windows::core::Result<Option<Area>> {
            let automation: IUIAutomation =
                CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)?;
            if let Ok(patient) = automation.cast::<IUIAutomation2>() {
                patient.SetConnectionTimeout(PATIENCE_MS)?;
                patient.SetTransactionTimeout(PATIENCE_MS)?;
            }
            let element = automation.GetFocusedElement()?;
            let text: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId)?;
            let ranges = text.GetSelection()?;
            let mut rectangles = Vec::new();
            for i in 0..ranges.Length()? {
                let array = ranges.GetElement(i)?.GetBoundingRectangles()?;
                let values = doubles(array);
                let _ = SafeArrayDestroy(array);
                rectangles.extend(
                    values?
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|&[x, y, w, h]| (x, y, w, h)),
                );
            }
            Ok(union(&rectangles))
        })();
        if set_up {
            CoUninitialize();
        }
        bounds.ok().flatten()
    }
}

/// The numbers in a one-dimensional SAFEARRAY of doubles.
///
/// # Safety
/// `array` is null or a valid one-dimensional SAFEARRAY of `VT_R8`.
unsafe fn doubles(array: *mut SAFEARRAY) -> ::windows::core::Result<Vec<f64>> {
    if array.is_null() {
        return Ok(Vec::new());
    }
    // SAFETY: as the caller promises; the data is let go before returning.
    unsafe {
        let low = SafeArrayGetLBound(array, 1)?;
        let high = SafeArrayGetUBound(array, 1)?;
        let len = usize::try_from(high - low + 1).unwrap_or(0);
        let mut data = std::ptr::null_mut();
        SafeArrayAccessData(array, &mut data)?;
        let values = std::slice::from_raw_parts(data as *const f64, len).to_vec();
        let _ = SafeArrayUnaccessData(array);
        Ok(values)
    }
}

/// The smallest rectangle around `(left, top, width, height)` rectangles with an area.
fn union(rectangles: &[(f64, f64, f64, f64)]) -> Option<Area> {
    let mut sides: Option<(f64, f64, f64, f64)> = None;
    for &(left, top, width, height) in rectangles {
        if !(width > 0.0 && height > 0.0 && left.is_finite() && top.is_finite()) {
            continue;
        }
        let (right, bottom) = (left + width, top + height);
        sides = Some(match sides {
            None => (left, top, right, bottom),
            Some((l, t, r, b)) => (l.min(left), t.min(top), r.max(right), b.max(bottom)),
        });
    }
    let (left, top, right, bottom) = sides?;
    Some(Area {
        x: left.round() as i32,
        y: top.round() as i32,
        width: (right - left).round() as u32,
        height: (bottom - top).round() as u32,
    })
}

/// The caret of the application in front: the system caret where it shows one, else the
/// caret it describes to accessibility tools (browsers and Electron apps draw their own
/// caret, and describe it so).
fn caret_bounds() -> Option<Area> {
    // SAFETY: plain Win32 calls on a zeroed, sized structure.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return None;
        }
        let thread = GetWindowThreadProcessId(window, None);
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        GetGUIThreadInfo(thread, &mut info).ok()?;
        let caret: HWND = info.hwndCaret;
        let rect = info.rcCaret;
        if caret.is_invalid() || rect.bottom <= rect.top {
            let focus = if info.hwndFocus.is_invalid() {
                window
            } else {
                info.hwndFocus
            };
            return described_caret(focus);
        }
        let mut corner = POINT {
            x: rect.left,
            y: rect.top,
        };
        if !ClientToScreen(caret, &mut corner).as_bool() {
            return None;
        }
        Some(Area {
            x: corner.x,
            y: corner.y,
            width: (rect.right - rect.left).max(1) as u32,
            height: (rect.bottom - rect.top) as u32,
        })
    }
}

/// The caret the window describes to accessibility tools (`OBJID_CARET`).
fn described_caret(window: HWND) -> Option<Area> {
    // SAFETY: COM is set up for this thread for the call; the object is released when
    // dropped; the VARIANT holds a plain integer.
    unsafe {
        let set_up = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let found = (|| -> ::windows::core::Result<Option<Area>> {
            let mut raw = std::ptr::null_mut();
            AccessibleObjectFromWindow(window, OBJID_CARET.0 as u32, &IAccessible::IID, &mut raw)?;
            if raw.is_null() {
                return Ok(None);
            }
            let caret = IAccessible::from_raw(raw);
            let mut child = VARIANT::default();
            (*child.Anonymous.Anonymous).vt = VT_I4;
            (*child.Anonymous.Anonymous).Anonymous.lVal = CHILDID_SELF as i32;
            let (mut x, mut y, mut width, mut height) = (0, 0, 0, 0);
            caret.accLocation(&mut x, &mut y, &mut width, &mut height, &child)?;
            Ok((height > 0).then_some(Area {
                x,
                y,
                width: width.max(1) as u32,
                height: height as u32,
            }))
        })();
        if set_up {
            CoUninitialize();
        }
        found.ok().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bounds_of_a_selection_over_two_lines_take_in_both() {
        assert_eq!(
            union(&[(100.0, 200.0, 300.0, 20.0), (40.0, 220.0, 80.0, 20.0)]),
            Some(Area {
                x: 40,
                y: 200,
                width: 360,
                height: 40
            })
        );
    }

    #[test]
    fn empty_rectangles_say_nothing() {
        assert_eq!(union(&[]), None);
        assert_eq!(union(&[(10.0, 10.0, 0.0, 0.0)]), None);
    }
}
