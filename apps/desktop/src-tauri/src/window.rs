//! The main window's native chrome: the platform's own title bar controls, and the
//! platform's window material (Mica on Windows 11, vibrancy on macOS) behind the
//! headword column where the system offers one.

use serde::Serialize;
use tauri::{App, WebviewWindowBuilder};

/// The window's label in `tauri.conf.json` (declared with `"create": false`; built here).
pub const MAIN: &str = "main";

/// The platform the app runs on, as far as the window's chrome cares.
// Each build constructs only its own platform; the others exist for the tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows { build: u32 },
    MacOs,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        return Platform::Windows {
            build: windows_version::OsVersion::current().build,
        };
        #[cfg(target_os = "macos")]
        return Platform::MacOs;
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        return Platform::Linux;
    }

    fn name(self) -> &'static str {
        match self {
            Platform::Windows { .. } => "windows",
            Platform::MacOs => "macos",
            Platform::Linux => "linux",
        }
    }
}

/// A system material shown through the window where the interface leaves it clear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Material {
    /// Windows 11's Mica (it tints with the window's light or dark theme).
    Mica,
    /// macOS's sidebar vibrancy.
    Vibrancy,
}

impl Material {
    fn name(self) -> &'static str {
        match self {
            Material::Mica => "mica",
            Material::Vibrancy => "vibrancy",
        }
    }
}

/// First Windows 11 build; Mica does not exist before it.
const WINDOWS_11: u32 = 22000;

/// The material the platform offers, if any.
pub fn material_for(platform: Platform) -> Option<Material> {
    match platform {
        Platform::Windows { build } if build >= WINDOWS_11 => Some(Material::Mica),
        Platform::Windows { .. } | Platform::Linux => None,
        Platform::MacOs => Some(Material::Vibrancy),
    }
}

/// What the interface is told about the window, so it can leave room for the title bar
/// controls and let the material show through.
#[derive(Debug, Clone, Serialize)]
pub struct ChromeView {
    platform: &'static str,
    /// The material that was actually applied; `None` if there is none or it failed.
    material: Option<&'static str>,
}

impl ChromeView {
    pub fn new(platform: Platform, material: Option<Material>) -> Self {
        Self {
            platform: platform.name(),
            material: material.map(Material::name),
        }
    }
}

/// Builds the main window from its configuration with the platform's native chrome:
/// the system's own title bar controls everywhere (on macOS they sit in the cover band,
/// which is the title bar there), and the system material where there is one. The
/// window is only made transparent when a material will fill it, so where there is none
/// (Windows 10, Linux) nothing can show through.
pub fn build_main(app: &App) -> tauri::Result<ChromeView> {
    let platform = Platform::current();
    let wanted = material_for(platform);
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN)
        .expect("tauri.conf.json declares the main window")
        .clone();
    let builder = WebviewWindowBuilder::from_config(app.handle(), &config)?;
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        // Centred in the 48px cover band.
        .traffic_light_position(tauri::LogicalPosition::new(16.0, 18.0));
    let window = builder.transparent(wanted.is_some()).build()?;
    let applied = wanted.filter(|material| apply(&window, *material));
    Ok(ChromeView::new(platform, applied))
}

/// Applies `material` to the window; false if the system refused it.
fn apply(window: &tauri::WebviewWindow, material: Material) -> bool {
    let result = match material {
        Material::Mica => window_vibrancy::apply_mica(window, None),
        Material::Vibrancy => {
            #[cfg(target_os = "macos")]
            let result = window_vibrancy::apply_vibrancy(
                window,
                window_vibrancy::NSVisualEffectMaterial::Sidebar,
                None,
                None,
            );
            #[cfg(not(target_os = "macos"))]
            let result = Err(window_vibrancy::Error::UnsupportedPlatform(
                "vibrancy is macOS only",
            ));
            result
        }
    };
    match result {
        Ok(()) => true,
        Err(error) => {
            eprintln!("OpenDict: no window material ({error})");
            false
        }
    }
}

/// What [`build_main`] found: the platform, and the material if it took.
#[tauri::command]
pub fn window_chrome(chrome: tauri::State<'_, ChromeView>) -> ChromeView {
    chrome.inner().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_11_gets_mica() {
        assert_eq!(
            material_for(Platform::Windows { build: 22000 }),
            Some(Material::Mica)
        );
        assert_eq!(
            material_for(Platform::Windows { build: 26200 }),
            Some(Material::Mica)
        );
    }

    #[test]
    fn windows_10_has_no_material() {
        assert_eq!(material_for(Platform::Windows { build: 19045 }), None);
    }

    #[test]
    fn macos_gets_vibrancy_and_linux_nothing() {
        assert_eq!(material_for(Platform::MacOs), Some(Material::Vibrancy));
        assert_eq!(material_for(Platform::Linux), None);
    }

    #[test]
    fn the_interface_is_told_the_platform_and_the_material_that_took() {
        let chrome = ChromeView::new(Platform::Windows { build: 26200 }, Some(Material::Mica));
        assert_eq!(
            serde_json::to_value(chrome).unwrap(),
            serde_json::json!({ "platform": "windows", "material": "mica" })
        );
        let chrome = ChromeView::new(Platform::MacOs, None);
        assert_eq!(
            serde_json::to_value(chrome).unwrap(),
            serde_json::json!({ "platform": "macos", "material": null })
        );
    }
}
