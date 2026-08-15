//! The booter plane — who is armed, when they are due, and the off switch.
//!
//! ⛔ THIS FILE RE-IMPLEMENTS NOTHING. The booter's state lives in files under
//! `~/.yggterm/relay/`, and reading them directly would be easy and wrong: it
//! would mean a second copy of the rules that interpret them — when a deferral
//! has expired, when a disarm has lapsed, what "due" means. Two answers to
//! "is the booter on right now" is the whole defect this pane exists to fix.
//!
//! So every read is `ygg-booter.py … --json` and every write is one of its
//! verbs. yggtopo renders and drives; the booter decides.

use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

/// Where the booter script is, and how we know.
///
/// ⭐ THE RUNNING WATCHER NAMES ITS OWN SOURCE. A watcher process was started
/// from a particular copy of the script, and that copy is the one whose state
/// directory it is writing — so when one is running, its own command line is a
/// better answer than any convention. The convention is the fallback for the
/// case that matters most: nothing is running, which is exactly when someone
/// wants to arm it.
pub fn script_path() -> (PathBuf, &'static str) {
    if let Ok(explicit) = std::env::var("YGGTOPO_BOOTER") {
        if !explicit.trim().is_empty() {
            return (PathBuf::from(explicit), "YGGTOPO_BOOTER");
        }
    }
    if let Some(from_pid) = from_running_watcher() {
        return (from_pid, "the running watcher's own command line");
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    (
        home.join("gh/yggterm/.agents/skills/yggterm-agent-fleet/ygg-booter.py"),
        "the conventional checkout path",
    )
}

fn from_running_watcher() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let pid = std::fs::read_to_string(home.join(".yggterm/relay/booter.pid"))
        .ok()?
        .trim()
        .to_string();
    let cmdline = std::fs::read_to_string(format!("/proc/{pid}/cmdline")).ok()?;
    let arg = cmdline
        .split('\0')
        .find(|a| a.ends_with("ygg-booter.py"))?
        .to_string();
    // ⛔ IDENTIFY, NEVER COUNT. A pid file whose pid has been reused by an
    //    unrelated process would otherwise hand us a path from someone else's
    //    command line — the booter's own code carries this same warning about
    //    its own pid file, for the same reason.
    if !cmdline.contains("ygg-booter") {
        return None;
    }
    Some(PathBuf::from(arg))
}

fn run(host: Option<&str>, args: &[&str], timeout: Duration) -> Value {
    let (script, _how) = script_path();
    let script = script.to_string_lossy().to_string();
    let output = match host {
        None => Command::new("python3")
            .arg(&script)
            .args(args)
            .output(),
        Some(h) => {
            // The remote copy lives at the same place in that host's home.
            // ⚠ `~` is expanded by the REMOTE shell, so the path is sent with
            //    the tilde intact rather than this machine's home baked in.
            let remote = script.replace(
                &dirs::home_dir().unwrap_or_default().to_string_lossy().to_string(),
                "~",
            );
            Command::new("ssh")
                .arg("-o")
                .arg(format!("ConnectTimeout={}", timeout.as_secs().max(1)))
                .arg("-o")
                .arg("BatchMode=yes")
                .arg(h)
                .arg(format!("python3 {remote} {}", args.join(" ")))
                .output()
        }
    };
    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            // ⛔ THE OBJECT SPANS MANY LINES, SO "THE LAST LINE THAT LOOKS LIKE
            //    JSON" IS A NESTED FRAGMENT. That heuristic works for a probe
            //    printing one compact line and fails silently here, reporting
            //    "EOF while parsing an object" about output that was perfectly
            //    well formed — a parse error blamed on the far side when the
            //    fault was entirely in how it was cut up.
            //
            //    The discriminator is INDENTATION: the verb narrates itself in
            //    timestamped log lines, and the reply is pretty-printed, so the
            //    document's own opening brace is the first one at column zero
            //    and every nested brace is indented under it.
            let start = text
                .lines()
                .position(|l| l.starts_with('{'))
                .map(|i| text.lines().take(i).map(|l| l.len() + 1).sum::<usize>());
            match start.map(|i| text[i.min(text.len())..].to_string()) {
                Some(document) => serde_json::from_str::<Value>(&document).unwrap_or_else(|e| {
                    json!({"ok": false, "error": format!("unreadable booter output: {e}")})
                }),
                None => json!({
                    "ok": out.status.success(),
                    // ⛔ SAY WHAT IT SAID. A verb that printed a refusal — a
                    //    monitor declining to be unsubscribed, say — is not a
                    //    failure to report as "no output"; the refusal IS the
                    //    answer, and hiding it makes the button look broken.
                    "message": if text.trim().is_empty() { stderr.trim().to_string() }
                               else { text.trim().to_string() },
                }),
            }
        }
        Err(e) => json!({"ok": false, "error": e.to_string()}),
    }
}

/// One host's booter state, as the booter itself reports it.
///
/// ⭐ `--due` IS THE WHOLE REASON THIS TAB IS WORTH OPENING. Without it the
/// pane can say who is armed but not who is about to be kicked, which is the
/// question a human actually arrives with. It costs a classification per
/// subscriber, which is why the booter keeps it opt-in and why this plane is
/// sampled on its own slower cadence rather than with the topology.
#[allow(dead_code)]
pub fn state(host: Option<&str>, timeout: Duration) -> Value {
    let mut value = run(host, &["list", "--json", "--due"], timeout);
    if value.get("host").is_none() {
        value["host"] = json!(host.unwrap_or(crate::fleet::LOCAL));
    }
    value
}

#[allow(dead_code)]
pub fn disarm(host: Option<&str>, hours: Option<f64>, note: &str, timeout: Duration) -> Value {
    let hours_text;
    let mut args: Vec<&str> = vec!["disarm"];
    match hours {
        None => args.push("--forever"),
        Some(h) => {
            hours_text = format!("{h}");
            args.push("--hours");
            args.push(&hours_text);
        }
    }
    // Quoted so a multi-word reason survives the remote shell.
    let quoted = format!("'{}'", note.replace('\'', ""));
    args.push("--note");
    args.push(&quoted);
    run(host, &args, timeout)
}

#[allow(dead_code)]
pub fn arm(host: Option<&str>, timeout: Duration) -> Value {
    run(host, &["arm"], timeout)
}

#[allow(dead_code)]
pub fn defer(host: Option<&str>, uuid: &str, secs: u32, timeout: Duration) -> Value {
    let secs = secs.to_string();
    run(
        host,
        &["defer", "--row", uuid, "--secs", &secs, "--note", "'deferred from yggtopo'"],
        timeout,
    )
}

#[allow(dead_code)]
pub fn unsubscribe(host: Option<&str>, uuid: &str, timeout: Duration) -> Value {
    run(host, &["unsubscribe", "--row", uuid], timeout)
}

pub fn set_rate_limit_hold(host: Option<&str>, duration: &str, reason: &str) -> Value {
    let quoted_reason = format!("'{}'", reason.replace('\'', ""));
    run(host, &["hold", "--until", duration, "--reason", &quoted_reason], Duration::from_secs(5))
}

pub fn release_rate_limit_hold(host: Option<&str>) -> Value {
    run(host, &["hold", "--release"], Duration::from_secs(5))
}
