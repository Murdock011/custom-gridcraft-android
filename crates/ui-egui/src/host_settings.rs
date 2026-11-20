//! The settings a host without its own settings code keeps between runs (the Android app):
//! `ui.json` ([`UiState`]) and `prefs.json` (the engine's preferences), in a folder the host
//! names. The desktop app has its own copy of this in `apps/gridcraft/src/main.rs`. Reading never
//! fails: a missing or unreadable file keeps the defaults.

use std::path::Path;

use crate::{SheetApp, UiState};

/// Reads the saved settings from `dir` into `app`. Without a `ui.json` the defaults stay: the
/// Light theme and the phone's Tiles layout.
pub fn load(app: &mut SheetApp, dir: &Path) {
    if let Ok(b) = std::fs::read(dir.join("ui.json"))
        && let Ok(ui) = serde_json::from_slice::<UiState>(&b)
    {
        let saved = serde_json::from_slice(&b).ok().and_then(|v| crate::i18n::saved_language(&v));
        let language = saved.unwrap_or(app.ui.language);
        app.ui = ui.at_startup();
        app.ui.language = language;
    }
    if let Ok(b) = std::fs::read(dir.join("prefs.json"))
        && let Ok(p) = serde_json::from_slice(&b)
    {
        app.session.prefs = p;
    }
}

/// The settings as bytes, to tell whether they changed since they were saved.
pub fn snapshot(app: &SheetApp) -> Vec<u8> {
    serde_json::to_vec(&(&app.ui, &app.session.prefs)).unwrap_or_default()
}

/// Writes the settings into `dir` (made when missing), each file through a temporary file and a
/// rename, so an interrupted write leaves the previous file. A failed write only loses the change.
pub fn save(app: &SheetApp, dir: &Path) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    for (name, bytes) in [("ui.json", serde_json::to_vec_pretty(&app.ui)), ("prefs.json", serde_json::to_vec_pretty(&app.session.prefs))] {
        let Ok(bytes) = bytes else { continue };
        let tmp = dir.join(format!(".{name}.tmp"));
        if std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, dir.join(name))).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> SheetApp {
        SheetApp::new(gridcraft_engine::Session::new(), crate::Services::default())
    }

    /// The theme and the phone layout survive a save and a load; a missing folder keeps defaults.
    #[test]
    fn settings_round_trip() {
        let dir = std::env::temp_dir().join(format!("gridcraft-host-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut a = app();
        let before = a.ui.dark;
        load(&mut a, &dir);
        assert_eq!(a.ui.dark, before, "no files: the defaults stay");
        a.ui.dark = !before;
        a.ui.phone_list_style = !a.ui.phone_list_style;
        let list = a.ui.phone_list_style;
        save(&a, &dir);
        let mut b = app();
        load(&mut b, &dir);
        assert_eq!(b.ui.dark, !before, "the theme is kept");
        assert_eq!(b.ui.phone_list_style, list, "the phone layout is kept");
        assert_eq!(snapshot(&a), snapshot(&b));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
