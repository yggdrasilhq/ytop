//! yggtopo's LAUNCHER MANIFEST — how the yggterm menus learn yggtopo exists.
//!
//! Written to `~/.yggterm/apps/yggtopo.json` on the app's OWN host on every
//! run, which repairs the binary path after an upgrade. The host's daemon scans
//! the directory and deletes manifests whose binary is gone — that is the whole
//! uninstall story. An app declares itself with a FILE, not by linking the
//! platform.

use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn manifest_value(binary: &Path) -> Value {
    json!({
        "name": "yggtopo",
        "label": "Yggtopo",
        "icon": "🌳\u{fe0e}",
        "binary": binary.to_string_lossy(),
        "verbs": [
            { "id": "open", "label": "Fleet topology", "args": [] },
            { "id": "booter", "label": "Fleet booter", "args": ["--tab", "booter"] },
        ],
    })
}

fn write_to(apps_dir: &Path, binary: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(apps_dir)?;
    let path = apps_dir.join("yggtopo.json");
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
        let value = manifest_value(Path::new("/usr/local/bin/yggtopo"));
        assert_eq!(value["name"], "yggtopo");
        assert!(value["binary"].as_str().unwrap().starts_with('/'));
        assert_eq!(value["verbs"].as_array().unwrap().len(), 2);
    }
}
