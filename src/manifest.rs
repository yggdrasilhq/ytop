//! ytop's LAUNCHER MANIFEST — how the yggterm menus learn ytop exists.
//!
//! Written to `~/.yggterm/apps/ytop.json` on the app's OWN host on every
//! run, which repairs the binary path after an upgrade. The host's daemon scans
//! the directory and deletes manifests whose binary is gone — that is the whole
//! uninstall story. An app declares itself with a FILE, not by linking the
//! platform.
//! Formerly `yggtopo.json` — that file is removed on upgrade.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn manifest_value(binary: &Path) -> Value {
    json!({
        "name": "ytop",
        "label": "Ytop",
        "icon": "📊\u{fe0e}",
        "binary": binary.to_string_lossy(),
        // Exactly one verb is a row: the document surface the user opens and
        // returns to. The optional Dash shortcut remains launcher-only.
        //
        // yggterm's ROW context menu spawns a session and puts a row in the
        // sidebar for it. `New Ytop` is that foreground session. Dash is a
        // launch mode for the same app, not an extra row-context verb.
        //
        // `row_spawn` only controls the row context menu. The titlebar `+` and
        // start page still offer every verb.
        //
        // The flag defaults to true, so this is an opt-out and older yggterm
        // builds that do not know the field simply ignore it.
        "verbs": [
            { "id": "new", "label": "New Ytop", "args": [], "row_spawn": true },
            { "id": "dash", "label": "Dash notebooks", "args": ["--mode", "dash"], "row_spawn": false },
        ],
    })
}

fn write_to(apps_dir: &Path, binary: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(apps_dir)?;
    // Remove stale yggtopo manifest (one-release compat).
    let _ = std::fs::remove_file(apps_dir.join("yggtopo.json"));
    let path = apps_dir.join("ytop.json");
    std::fs::write(&path, serde_json::to_string_pretty(&manifest_value(binary))?)?;
    Ok(path)
}

/// Best-effort on every run; a failure must never stop the app.
pub fn write_best_effort() {
    let Some(home) = dirs::home_dir() else { return };
    let Ok(binary) = std::env::current_exe() else { return };
    let _ = write_to(&home.join(".yggterm").join("apps"), &binary);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_names_match_the_file_stem_and_the_binary_is_absolute() {
        let value = manifest_value(Path::new("/usr/local/bin/ytop"));
        assert_eq!(value["name"], "ytop");
        assert!(value["binary"].as_str().unwrap().starts_with('/'));
        assert_eq!(value["verbs"].as_array().unwrap().len(), 2);
    }

    /// The context menu's one launch affordance is deliberately a real row:
    /// Ytop is a foreground document-surface app and the row is how the user
    /// returns to it. The Dash shortcut remains launcher-only.
    #[test]
    fn only_new_ytop_asks_to_become_a_sidebar_row() {
        let value = manifest_value(Path::new("/usr/local/bin/ytop"));
        let verbs = value["verbs"].as_array().unwrap();
        assert_eq!(verbs[0]["label"], "New Ytop");
        assert_eq!(verbs[0]["row_spawn"], serde_json::json!(true));
        assert!(verbs[1..]
            .iter()
            .all(|verb| verb["row_spawn"] == serde_json::json!(false)));
    }
}
