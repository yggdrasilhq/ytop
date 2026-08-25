//! The libyggterm OSC 7717 channel — ytop dash's side of the surface contract.
//!
//! ytop dash is a DOCUMENT-SURFACE app: it declares one viewport pane and one
//! rail pane, and yggterm renders their SCHEMA as ordinary shell DOM. No web
//! engine, no child webview — which is what keeps the pane screenshot-faithful
//! and reachable by the host's own automation, and is the reason the tier
//! exists at all.
//!
//! ⛔ THE SCHEMA DOES NOT RIDE THE OSC. The declare carries only a loopback
//! control URL and the list of panes offered; the GUI then GETs the schema it
//! wants. A fleet view is large and refreshes often — putting it on the PTY
//! byte stream would push every frame of it through the terminal.

use base64::Engine as _;
use serde_json::json;
use std::io::Write as _;

fn emit(verb: &str, action: &str, payload: &str) {
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    let mut stdout = std::io::stdout().lock();
    let _ = write!(stdout, "\u{1b}]7717;{verb};{action};{encoded}\u{7}");
    let _ = stdout.flush();
}

/// `sidebar ; declare` — idempotent, re-emitted on the heartbeat cadence as the
/// liveness signal. `document_version` moves when the reading does, and that is
/// what makes the GUI refetch.
///
/// ⛔ IT MUST NOT RE-RESOLVE THE CONTROL URL. The declare fires every few
/// seconds; resolving the endpoint inside it would spawn one forwarding tunnel
/// per heartbeat, which is a leak that looks like a working app for the first
/// few minutes.
pub fn emit_declare(session: &str, control: &str, document_version: &str) {
    crate::trace::event(
        "osc",
        "heartbeat",
        json!({
            "session": session,
            "document_version": document_version
        }),
    );
    let payload = json!({
        "session": session,
        "control": control,
        "app_name": "Ytop Dash",
        "document_version": document_version,
        "panes": [
            {
                // The fleet itself, in the main viewport.
                // U+FE0E keeps the glyph in text presentation so it sits with
                // yggterm's monochrome chrome instead of arriving as colour
                // emoji at a different size.
                "id": "topo",
                "icon": "🌳\u{fe0e}",
                "title": "Ytop Dash (fleet topology and live processes)",
                "placement": "viewport",
            },
            {
                // The same two tabs in the rail, for keeping an eye on the
                // booter while working in the terminal.
                "id": "rail",
                "icon": "🌳\u{fe0e}",
                "title": "Ytop Dash (fleet at a glance)",
                "placement": "rail",
            },
        ],
    });
    emit("sidebar", "declare", &payload.to_string());
}

pub fn emit_close(session: &str) {
    emit("sidebar", "close", &json!({ "session": session }).to_string());
}
