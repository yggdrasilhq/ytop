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
        // ⛔ `row_spawn: false` on every verb, and it is not an oversight.
        //
        // yggterm's ROW context menu spawns a session and puts a row in the
        // sidebar for it. ytop is a terminal-invoked TUI: none of these verbs is
        // a session anyone wants a persistent sidebar row for, and offering them
        // there gave the user rows for things that were never sessions. The app
        // is the only party that can know this — yggterm would have to hardcode
        // another app's name to guess it — so the manifest says so.
        //
        // ⚠ It removes them from the ROW menu ONLY. The titlebar `+` and the
        // start page still offer all three, because opening a dashboard from
        // there is exactly what those surfaces are for.
        //
        // The flag defaults to true, so this is an opt-out and older yggterm
        // builds that do not know the field simply ignore it.
        "verbs": [
            { "id": "open", "label": "Fleet topology", "args": [], "row_spawn": false },
            { "id": "booter", "label": "Fleet booter", "args": ["--tab", "booter"], "row_spawn": false },
            { "id": "dash", "label": "Dash notebooks", "args": ["--tab", "dash"], "row_spawn": false },
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
        assert_eq!(value["verbs"].as_array().unwrap().len(), 3);
    }

    /// ⛔ NONE of ytop's verbs is a session row. A row menu that offered them
    /// gave the user sidebar rows for a TUI that was never a session, which is
    /// what the flag exists to stop — and only this manifest can say so.
    #[test]
    fn no_verb_asks_to_become_a_sidebar_row() {
        let value = manifest_value(Path::new("/usr/local/bin/ytop"));
        for verb in value["verbs"].as_array().unwrap() {
            assert_eq!(
                verb["row_spawn"],
                serde_json::json!(false),
                "{} would spawn a row",
                verb["id"],
            );
        }
    }
}
