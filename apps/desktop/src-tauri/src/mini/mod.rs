//! The Mini Window: a second, frameless, always-on-top window summoned by a global
//! shortcut over whatever application the reader is in. The shortcut copies the
//! selection there (putting the clipboard back afterwards), opens the Mini Window beside
//! the pointer and looks the selection up; Esc closes it and gives focus back. A second
//! shortcut brings the main window forward, and clipboard watching (off by default)
//! looks up whatever the reader copies.

pub mod placement;
pub mod selection;
pub mod settings;

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod unix;
#[cfg(target_os = "windows")]
mod windows;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde::Serialize;
use tauri::{App, AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::window::MAIN;
use placement::Area;
use settings::{MiniSettings, Role, SettingsView};

/// The Mini Window's label in `tauri.conf.json` (declared with `"create": false`; built here).
pub const MINI: &str = "mini";

/// Sent to the Mini Window with a [`Summons`] each time it is summoned.
pub const SUMMONED_EVENT: &str = "mini-summoned";
/// Sent to the main window with a headword when the reader opens it there.
pub const OPEN_EVENT: &str = "open-in-main";

/// What the Mini Window is to look up. `serial` tells summonses apart, so the same text
/// summoned twice is looked up twice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summons {
    pub text: String,
    pub serial: u64,
}

/// The settings as the interface shows them, and which shortcuts could not be
/// registered (another application has them).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MiniSettingsView {
    #[serde(flatten)]
    pub settings: SettingsView,
    pub unavailable: Vec<&'static str>,
    /// Why the settings file could not be read, if it could not.
    pub problem: Option<String>,
}

/// The Mini Window's state, managed by the app.
pub struct Mini {
    settings: Mutex<MiniSettings>,
    problem: Mutex<Option<String>>,
    /// Shortcuts the system refused (another application registered them first).
    unavailable: Mutex<Vec<Role>>,
    last: Mutex<Option<Summons>>,
    serial: AtomicU64,
    /// Where the reader was when the Mini Window was summoned.
    focus: Mutex<Option<Focus>>,
    /// A capture is under way: its copy and restore are not the reader's copies.
    capturing: AtomicBool,
    /// The clipboard's change count after the last capture put it back.
    restored: AtomicU64,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Mini {
    pub fn load(data_dir: &Path) -> Mini {
        let (settings, problem) = MiniSettings::load(&data_dir.join(settings::FILE_NAME));
        if let Some(problem) = &problem {
            eprintln!("OpenDict: {problem}");
        }
        Mini {
            settings: Mutex::new(settings),
            problem: Mutex::new(problem),
            unavailable: Mutex::new(Vec::new()),
            last: Mutex::new(None),
            serial: AtomicU64::new(0),
            focus: Mutex::new(None),
            capturing: AtomicBool::new(false),
            restored: AtomicU64::new(0),
        }
    }

    pub fn view(&self) -> MiniSettingsView {
        MiniSettingsView {
            settings: lock(&self.settings).view(),
            unavailable: lock(&self.unavailable)
                .iter()
                .map(|role| role_name(*role))
                .collect(),
            problem: lock(&self.problem).clone(),
        }
    }

    /// The latest summons, for a Mini Window that loads after it was sent.
    pub fn last(&self) -> Option<Summons> {
        lock(&self.last).clone()
    }
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::LookUpSelection => "lookUpSelection",
        Role::ShowMainWindow => "showMainWindow",
    }
}

/// The global-shortcut plugin, wired to the Mini Window's two shortcuts.
pub fn shortcut_plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let Some(mini) = app.try_state::<Mini>() else {
                return;
            };
            let settings = lock(&mini.settings);
            if settings.shortcut(Role::LookUpSelection) == Some(*shortcut) {
                drop(settings);
                let app = app.clone();
                // Copying waits for the application in front; the event loop must not.
                std::thread::spawn(move || summon_selection(&app));
            } else if settings.shortcut(Role::ShowMainWindow) == Some(*shortcut) {
                drop(settings);
                show_main(app);
            }
        })
        .build()
}

/// Builds the (hidden) Mini Window from its configuration, registers the shortcuts and
/// starts clipboard watching.
pub fn set_up(app: &App, data_dir: &Path) -> tauri::Result<()> {
    app.manage(Mini::load(data_dir));
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MINI)
        .expect("tauri.conf.json declares the Mini Window")
        .clone();
    let window = WebviewWindowBuilder::from_config(app.handle(), &config)?.build()?;
    // Closing (Alt+F4, say) puts it away like Esc; it is summoned again, not rebuilt.
    let handle = app.handle().clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            close(&handle);
        }
    });
    let handle = app.handle().clone();
    for role in [Role::LookUpSelection, Role::ShowMainWindow] {
        register(&handle, role, None);
    }
    watch_clipboard(handle);
    Ok(())
}

/// Registers the shortcut for `role` in place of `previous`; records it as unavailable
/// if the system refuses it.
fn register<R: Runtime>(app: &AppHandle<R>, role: Role, previous: Option<Shortcut>) {
    let mini = app.state::<Mini>();
    let shortcuts = app.global_shortcut();
    if let Some(previous) = previous
        && shortcuts.is_registered(previous)
    {
        let _ = shortcuts.unregister(previous);
    }
    let mut unavailable = lock(&mini.unavailable);
    unavailable.retain(|r| *r != role);
    let Some(shortcut) = lock(&mini.settings).shortcut(role) else {
        return;
    };
    if let Err(error) = shortcuts.register(shortcut) {
        eprintln!(
            "OpenDict: the shortcut {} is not available ({error})",
            settings::accelerator(shortcut)
        );
        unavailable.push(role);
    }
}

/// Reads the settings again from the data directory, now at `data_dir` (after it moved, or
/// the reader switched to other data), and registers its shortcuts in place of the old.
pub fn reload<R: Runtime>(app: &AppHandle<R>, data_dir: &Path) {
    let mini = app.state::<Mini>();
    let (settings, problem) = MiniSettings::load(&data_dir.join(settings::FILE_NAME));
    let previous = {
        let mut current = lock(&mini.settings);
        let previous =
            [Role::LookUpSelection, Role::ShowMainWindow].map(|role| current.shortcut(role));
        *current = settings;
        previous
    };
    *lock(&mini.problem) = problem;
    for (role, previous) in [Role::LookUpSelection, Role::ShowMainWindow]
        .into_iter()
        .zip(previous)
    {
        register(app, role, previous);
    }
}

/// Changes a shortcut (`None` turns it off) and registers it in place of the old one.
pub fn set_shortcut<R: Runtime>(
    app: &AppHandle<R>,
    role: Role,
    accelerator: Option<&str>,
) -> Result<MiniSettingsView, String> {
    let mini = app.state::<Mini>();
    let previous = lock(&mini.settings).shortcut(role);
    lock(&mini.settings).set_shortcut(role, accelerator)?;
    *lock(&mini.problem) = None;
    register(app, role, previous);
    Ok(mini.view())
}

pub fn set_watch_clipboard<R: Runtime>(
    app: &AppHandle<R>,
    on: bool,
) -> Result<MiniSettingsView, String> {
    let mini = app.state::<Mini>();
    lock(&mini.settings).set_watch_clipboard(on)?;
    *lock(&mini.problem) = None;
    Ok(mini.view())
}

/// The shortcut was pressed: copy the selection in the application in front and look it
/// up. With nothing selected the Mini Window opens empty, ready to type in.
fn summon_selection<R: Runtime>(app: &AppHandle<R>) {
    let mini = app.state::<Mini>();
    if mini.capturing.swap(true, Ordering::SeqCst) {
        return; // the reader pressed it twice; the first capture answers
    }
    let focus = Focus::now();
    let text = selected_text();
    mini.restored.store(clipboard_count(), Ordering::SeqCst);
    mini.capturing.store(false, Ordering::SeqCst);
    summon(app, text.unwrap_or_default(), focus);
}

/// Opens the Mini Window beside the pointer and looks `text` up there.
fn summon<R: Runtime>(app: &AppHandle<R>, text: String, focus: Option<Focus>) {
    let Some(window) = app.get_webview_window(MINI) else {
        return;
    };
    let mini = app.state::<Mini>();
    // Summoned again from the Mini Window itself, the reader still came from where they were.
    if !window.is_focused().unwrap_or(false) {
        *lock(&mini.focus) = focus;
    }
    if let Some(position) = position_near_pointer(app, &window) {
        let _ = window.set_position(position);
    }
    let summons = Summons {
        text,
        serial: mini.serial.fetch_add(1, Ordering::SeqCst) + 1,
    };
    *lock(&mini.last) = Some(summons.clone());
    let _ = app.emit_to(MINI, SUMMONED_EVENT, summons);
    let _ = window.show();
    let _ = window.set_focus();
}

/// Where the Mini Window goes for the pointer where it is now, on the screen it is on.
fn position_near_pointer<R: Runtime>(
    app: &AppHandle<R>,
    window: &tauri::WebviewWindow<R>,
) -> Option<PhysicalPosition<i32>> {
    let pointer = app.cursor_position().ok()?;
    let monitor = app.monitor_from_point(pointer.x, pointer.y).ok()??;
    let area = monitor.work_area();
    let size = window.outer_size().ok()?;
    // The window takes the scale of the screen it moves to.
    let ratio = monitor.scale_factor() / window.scale_factor().unwrap_or(1.0);
    let size = (
        (f64::from(size.width) * ratio).round() as u32,
        (f64::from(size.height) * ratio).round() as u32,
    );
    let (x, y) = placement::place(
        (pointer.x.round() as i32, pointer.y.round() as i32),
        size,
        Area {
            x: area.position.x,
            y: area.position.y,
            width: area.size.width,
            height: area.size.height,
        },
        monitor.scale_factor(),
    );
    Some(PhysicalPosition::new(x, y))
}

/// Esc: the Mini Window goes and the reader is back where they were.
pub fn close<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(MINI) {
        let _ = window.hide();
    }
    let focus = lock(&app.state::<Mini>().focus).take();
    match focus {
        Some(focus) => focus.give_back(app),
        None => {
            if let Some(main) = app.get_webview_window(MAIN)
                && main.is_visible().unwrap_or(false)
            {
                let _ = main.set_focus();
            }
        }
    }
}

/// "Open in main window": the Mini Window goes, the main window comes forward and looks
/// `headword` up.
pub fn open_in_main<R: Runtime>(app: &AppHandle<R>, headword: &str) {
    if let Some(window) = app.get_webview_window(MINI) {
        let _ = window.hide();
    }
    lock(&app.state::<Mini>().focus).take();
    show_main(app);
    let _ = app.emit_to(MAIN, OPEN_EVENT, headword);
}

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
}

/// How often the clipboard is checked while clipboard watching is on.
const WATCH_INTERVAL: Duration = Duration::from_millis(400);

/// Clipboard watching: while the reader has it on, whatever they copy (as text) is
/// looked up in the Mini Window. The copies and restores of the shortcut's own captures
/// do not count.
fn watch_clipboard<R: Runtime>(app: AppHandle<R>) {
    std::thread::spawn(move || {
        // The change count when last checked; none while watching is off, so turning it
        // on does not look up what was copied before.
        let mut seen = None;
        loop {
            std::thread::sleep(WATCH_INTERVAL);
            let mini = app.state::<Mini>();
            if !lock(&mini.settings).watch_clipboard() {
                seen = None;
                continue;
            }
            if mini.capturing.load(Ordering::SeqCst) {
                continue;
            }
            let now = clipboard_count();
            if seen.replace(now).is_none_or(|before| before == now) {
                continue;
            }
            if now == mini.restored.load(Ordering::SeqCst) {
                continue;
            }
            if let Some(text) = clipboard_text().as_deref().and_then(selection::query_from) {
                summon(&app, text, Focus::now());
            }
        }
    });
}

/// Where the reader was, to give focus back to.
#[derive(Debug, Clone, Copy)]
struct Focus {
    #[cfg(target_os = "windows")]
    window: windows::Focus,
}

impl Focus {
    #[cfg(target_os = "windows")]
    fn now() -> Option<Focus> {
        windows::focus().map(|window| Focus { window })
    }

    /// On macOS and Linux, the reader was in some application; which one is the
    /// system's to remember.
    #[cfg(not(target_os = "windows"))]
    fn now() -> Option<Focus> {
        Some(Focus {})
    }

    #[cfg(target_os = "windows")]
    fn give_back<R: Runtime>(self, _app: &AppHandle<R>) {
        windows::give_back(self.window);
    }

    /// macOS gives focus back to the application the reader was in when OpenDict hides,
    /// unless the main window is open: then it is the one that comes forward.
    #[cfg(target_os = "macos")]
    fn give_back<R: Runtime>(self, app: &AppHandle<R>) {
        let main_open = app
            .get_webview_window(MAIN)
            .is_some_and(|main| main.is_visible().unwrap_or(false));
        if !main_open {
            let _ = app.hide();
        }
    }

    /// The window manager gives focus to the window under the Mini Window.
    #[cfg(target_os = "linux")]
    fn give_back<R: Runtime>(self, _app: &AppHandle<R>) {}
}

#[cfg(target_os = "windows")]
fn selected_text() -> Option<String> {
    selection::capture(
        &mut windows::SystemClipboard,
        &mut windows::SystemKeyboard,
        &mut std::thread::sleep,
    )
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn selected_text() -> Option<String> {
    unix::selected_text()
}

#[cfg(target_os = "windows")]
fn clipboard_count() -> u64 {
    use selection::Clipboard;
    windows::SystemClipboard.change_count()
}

#[cfg(target_os = "windows")]
fn clipboard_text() -> Option<String> {
    use selection::Clipboard;
    windows::SystemClipboard.text()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn clipboard_count() -> u64 {
    unix::clipboard_count()
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn clipboard_text() -> Option<String> {
    unix::clipboard_text()
}
