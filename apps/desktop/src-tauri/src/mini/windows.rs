//! Selection capture and focus on Windows: the whole clipboard saved and put back
//! format by format, a simulated Ctrl+C, and the window the reader was in.

use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{GlobalFree, HANDLE, HWND};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData,
    GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows_sys::Win32::System::Ole::{
    CF_BITMAP, CF_DSPBITMAP, CF_DSPENHMETAFILE, CF_DSPMETAFILEPICT, CF_ENHMETAFILE, CF_GDIOBJFIRST,
    CF_GDIOBJLAST, CF_METAFILEPICT, CF_OWNERDISPLAY, CF_PALETTE, CF_PRIVATEFIRST, CF_PRIVATELAST,
    CF_UNICODETEXT,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsWindow, SetForegroundWindow,
};

use super::selection::{Clipboard, Keyboard};

/// The Windows clipboard.
pub struct SystemClipboard;

/// Every format on the clipboard that is plain memory, with its bytes. Formats held as
/// GDI objects (bitmaps, metafiles, palettes) cannot be copied this way; Windows offers
/// the same pictures as plain-memory DIBs, which are kept.
pub struct SavedClipboard(Vec<(u32, Vec<u8>)>);

/// Holds the clipboard open; closes it when dropped.
struct Open;

impl Open {
    /// Opens the clipboard, retrying a moment while another application holds it.
    fn clipboard() -> Result<Open, String> {
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            // SAFETY: no owner window; closed again by `Drop`.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return Ok(Open);
            }
            if Instant::now() > deadline {
                return Err("another application is holding the clipboard".to_owned());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: opened by `Open::clipboard`.
        unsafe { CloseClipboard() };
    }
}

/// Whether the format is plain memory (an HGLOBAL), which can be copied as bytes.
fn is_memory(format: u32) -> bool {
    let handles = [
        CF_BITMAP,
        CF_METAFILEPICT,
        CF_PALETTE,
        CF_ENHMETAFILE,
        CF_OWNERDISPLAY,
        CF_DSPBITMAP,
        CF_DSPMETAFILEPICT,
        CF_DSPENHMETAFILE,
    ];
    let in_range = |first: u16, last: u16| (u32::from(first)..=u32::from(last)).contains(&format);
    !handles.iter().any(|&f| u32::from(f) == format)
        && !in_range(CF_PRIVATEFIRST, CF_PRIVATELAST)
        && !in_range(CF_GDIOBJFIRST, CF_GDIOBJLAST)
}

/// The bytes of a plain-memory clipboard handle.
///
/// # Safety
/// The clipboard must be open and `handle` must come from `GetClipboardData` for a
/// plain-memory format.
unsafe fn bytes_of(handle: HANDLE) -> Option<Vec<u8>> {
    // SAFETY: per the contract, `handle` is an HGLOBAL owned by the clipboard.
    unsafe {
        let size = GlobalSize(handle);
        let data = GlobalLock(handle);
        if data.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(data.cast::<u8>(), size).to_vec();
        GlobalUnlock(handle);
        Some(bytes)
    }
}

/// Puts `bytes` on the open clipboard as `format`.
///
/// # Safety
/// The clipboard must be open and emptied by this process.
unsafe fn put(format: u32, bytes: &[u8]) -> bool {
    // SAFETY: a fresh moveable block of the right size; ownership passes to the clipboard
    // when SetClipboardData succeeds, and is freed here when it does not.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1));
        if memory.is_null() {
            return false;
        }
        let data = GlobalLock(memory);
        if data.is_null() {
            GlobalFree(memory);
            return false;
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.cast::<u8>(), bytes.len());
        GlobalUnlock(memory);
        if SetClipboardData(format, memory).is_null() {
            GlobalFree(memory);
            return false;
        }
        true
    }
}

fn registered(name: &str) -> u32 {
    let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
    // SAFETY: a NUL-terminated UTF-16 string.
    unsafe { RegisterClipboardFormatW(wide.as_ptr()) }
}

impl Clipboard for SystemClipboard {
    type Saved = SavedClipboard;

    fn change_count(&mut self) -> u64 {
        // SAFETY: no arguments.
        u64::from(unsafe { GetClipboardSequenceNumber() })
    }

    fn save(&mut self) -> Result<SavedClipboard, String> {
        let _open = Open::clipboard()?;
        let mut saved = Vec::new();
        let mut format = 0;
        loop {
            // SAFETY: the clipboard is open.
            format = unsafe { EnumClipboardFormats(format) };
            if format == 0 {
                break;
            }
            if !is_memory(format) {
                continue;
            }
            // SAFETY: the clipboard is open and the format is plain memory.
            let handle = unsafe { GetClipboardData(format) };
            if handle.is_null() {
                continue;
            }
            // SAFETY: as above.
            if let Some(bytes) = unsafe { bytes_of(handle) } {
                saved.push((format, bytes));
            }
        }
        Ok(SavedClipboard(saved))
    }

    fn text(&mut self) -> Option<String> {
        let _open = Open::clipboard().ok()?;
        // SAFETY: the clipboard is open; CF_UNICODETEXT is plain memory.
        let bytes = unsafe { bytes_of(GetClipboardData(u32::from(CF_UNICODETEXT)))? };
        let (pairs, _) = bytes.as_chunks::<2>();
        let units: Vec<u16> = pairs
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .take_while(|&unit| unit != 0)
            .collect();
        Some(String::from_utf16_lossy(&units))
    }

    fn restore(&mut self, saved: SavedClipboard) -> Result<(), String> {
        let _open = Open::clipboard()?;
        // SAFETY: the clipboard is open.
        if unsafe { EmptyClipboard() } == 0 {
            return Err("the clipboard could not be emptied".to_owned());
        }
        let mut failed = 0;
        for (format, bytes) in &saved.0 {
            // SAFETY: the clipboard is open and was emptied by this process.
            if !unsafe { put(*format, bytes) } {
                failed += 1;
            }
        }
        // What goes back is what the reader had already: it is not a new item for the
        // clipboard history, the cloud clipboard or clipboard managers.
        let zero = 0u32.to_le_bytes();
        for (name, value) in [
            ("CanIncludeInClipboardHistory", &zero[..]),
            ("CanUploadToCloudClipboard", &zero[..]),
            ("ExcludeClipboardContentFromMonitorProcessing", &zero[..]),
        ] {
            let format = registered(name);
            if format != 0 && !saved.0.iter().any(|(f, _)| *f == format) {
                // SAFETY: as above.
                unsafe { put(format, value) };
            }
        }
        if failed > 0 {
            return Err(format!("{failed} clipboard formats could not be put back"));
        }
        Ok(())
    }
}

/// The keyboard of the window in front, through `SendInput`.
pub struct SystemKeyboard;

/// An unassigned virtual key. Tapped while Alt or the Windows key is down, it keeps their
/// release from opening the menu bar or the Start menu (the shortcut's own key never
/// reached the application in front).
const VK_MASK: VIRTUAL_KEY = 0xE8;
const MODIFIERS: [VIRTUAL_KEY; 5] = [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN];
/// How long the reader gets to let go of the shortcut's keys before the copy is sent.
const RELEASE_WAIT: Duration = Duration::from_millis(800);

fn is_down(key: VIRTUAL_KEY) -> bool {
    // SAFETY: no pointers.
    unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
}

fn key(key: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(inputs: &[INPUT]) -> Result<(), String> {
    // SAFETY: `inputs` is a valid slice of INPUT structures.
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err("the keys could not be sent (is another application blocking input?)".to_owned())
    }
}

impl Keyboard for SystemKeyboard {
    fn copy(&mut self) -> Result<(), String> {
        if [VK_MENU, VK_LWIN, VK_RWIN].into_iter().any(is_down) {
            send(&[key(VK_MASK, false), key(VK_MASK, true)])?;
        }
        // The shortcut's modifiers would turn Ctrl+C into another shortcut.
        let deadline = Instant::now() + RELEASE_WAIT;
        while MODIFIERS.into_iter().any(is_down) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let held: Vec<INPUT> = MODIFIERS
            .into_iter()
            .filter(|&m| is_down(m))
            .map(|m| key(m, true))
            .collect();
        if !held.is_empty() {
            send(&held)?;
        }
        send(&[
            key(VK_CONTROL, false),
            key(VK_C, false),
            key(VK_C, true),
            key(VK_CONTROL, true),
        ])
    }
}

/// The window the reader is in, to give focus back to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Focus(isize);

/// The window in front now.
pub fn focus() -> Option<Focus> {
    // SAFETY: no arguments.
    let window = unsafe { GetForegroundWindow() };
    (!window.is_null()).then_some(Focus(window as isize))
}

/// Brings the window back to the front, if it still exists. Works while one of this
/// process's windows is in front (the Mini Window, as the reader presses Esc).
pub fn give_back(focus: Focus) {
    let window = focus.0 as HWND;
    // SAFETY: IsWindow accepts any value; the window is only used if it exists.
    unsafe {
        if IsWindow(window) != 0 {
            SetForegroundWindow(window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Puts text and a private format of our own on the real clipboard.
    fn set(text: &str, private: Option<&[u8]>) {
        let _open = Open::clipboard().unwrap();
        unsafe { EmptyClipboard() };
        let wide: Vec<u8> = text
            .encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect();
        assert!(unsafe { put(u32::from(CF_UNICODETEXT), &wide) });
        if let Some(bytes) = private {
            assert!(unsafe { put(registered("OpenDict test format"), bytes) });
        }
    }

    fn private() -> Option<Vec<u8>> {
        let _open = Open::clipboard().unwrap();
        let handle = unsafe { GetClipboardData(registered("OpenDict test format")) };
        if handle.is_null() {
            None
        } else {
            unsafe { bytes_of(handle) }
        }
    }

    #[test]
    #[ignore = "uses the real clipboard (and puts the reader's back); run with --ignored on Windows"]
    fn the_whole_clipboard_is_put_back_format_by_format() {
        let mut clipboard = SystemClipboard;
        let reader = clipboard.save().unwrap();

        set("what the reader copied", Some(b"\x01\x02rich"));
        let before = clipboard.save().unwrap();
        let count = clipboard.change_count();
        set("the selection", None);
        assert!(
            clipboard.change_count() != count,
            "a copy changes the count"
        );
        assert_eq!(clipboard.text().as_deref(), Some("the selection"));
        assert_eq!(private(), None);

        clipboard.restore(before).unwrap();
        assert_eq!(clipboard.text().as_deref(), Some("what the reader copied"));
        assert_eq!(private().as_deref(), Some(&b"\x01\x02rich"[..]));

        clipboard.restore(reader).unwrap();
    }
}
