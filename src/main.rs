//! yggtopo — `lstopo` + `htop`, for a yggterm fleet.
//!
//! The machines, the containers they host, and what is actually burning them
//! right now, as ONE view. A libyggterm document-surface app: no UI code, no
//! web engine, no canvas — it declares a widget schema and yggterm paints it as
//! shell DOM, which is what keeps it screenshot-faithful and drivable by the
//! host's own automation.
//!
//! Outside yggterm it is a plain CLI that prints the same reading, because an
//! app that can only exist inside a GUI cannot be checked without one.
//!
//! ⭐ THE SECOND HALF OF THE APP IS AN OFF SWITCH. The fleet's booter is a
//! watchdog that kicks stalled sessions; it could always be stood down by
//! someone with a shell on the right machine who knew the verb, which is not an
//! off switch but a rumour of one. The Booter tab shows who is armed, when they
//! are due, and turns it off — which is the whole reason this app was built now
//! rather than later.

mod booter;
mod fleet;
mod manifest;
mod osc;
mod probe;
mod schema;
mod server;

use anyhow::Result;
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// The declare cadence. yggterm expires a contribution after ~15s of silence,
/// so a killed app never leaves an overlay behind; ~4s is the contract's rate.
const HEARTBEAT: Duration = Duration::from_secs(4);

#[derive(Parser)]
#[command(
    name = "yggtopo",
    version,
    about = "lstopo + htop for a yggterm fleet (libyggterm document-surface app)"
)]
struct Args {
    /// Which tab to open on.
    #[arg(long, value_parser = ["topology", "booter"], default_value = "topology")]
    tab: String,
    /// Print one reading and exit, even inside yggterm.
    #[arg(long)]
    once: bool,
    /// With --once, print the raw readings instead of the tree.
    #[arg(long)]
    json: bool,
    /// Run the host probe alone and print its JSON. `--probe <ssh-alias>` runs
    /// it there. The self-check for "is this machine readable at all".
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    probe: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    manifest::write_best_effort();

    if let Some(host) = args.probe {
        let host = host.trim();
        return server::probe_once((!host.is_empty()).then_some(host));
    }

    // ⛔ THE ENV VAR IS THE ONLY HONEST TEST FOR "AM I INSIDE YGGTERM".
    //    The daemon exports it into every PTY it owns, local or over ssh.
    //    Guessing from a TTY check or a parent-process name would be right on
    //    this machine and wrong on the next one.
    //
    // ⭐ AND THERE ARE TWO SPELLINGS, WHICH THE SIBLING BUILD APP KNEW AND THIS
    //    ONE DID NOT. A user who types `ssh <host>` by hand gets a stripped
    //    environment — but stock OpenSSH forwards `LC_*`, so an app on the far
    //    side of a MANUAL hop can still tell it is inside a surface. Checking
    //    only the direct export is a real, reported bug: the pilot editor
    //    answered "not inside yggterm" after exactly that hop.
    let session = ["YGGTERM_SESSION_ID", "LC_YGGTERM_SESSION_ID"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    if args.once || session.is_empty() {
        if session.is_empty() && !args.once {
            eprintln!(
                "yggtopo: not running inside yggterm ($YGGTERM_SESSION_ID unset) — \
                 printing one reading instead of opening a surface."
            );
        }
        return server::print_once(args.json);
    }

    let control = server::spawn()?;
    {
        let mut pane = control.state.lock().unwrap();
        pane.view.tab = args.tab.clone();
    }

    // ⛔ CLOSE ON THE WAY OUT, OR THE OVERLAY OUTLIVES THE APP by the length of
    //    the expiry. The heartbeat covers a SIGKILL; this covers the ordinary
    //    case, which is the one the user actually sees.
    let running = Arc::new(AtomicBool::new(true));
    {
        let running = Arc::clone(&running);
        let session = session.clone();
        ctrlc::set_handler(move || {
            osc::emit_close(&session);
            running.store(false, Ordering::SeqCst);
        })?;
    }

    let mut last_stamp = u64::MAX;
    while running.load(Ordering::SeqCst) {
        let stamp = control.state.lock().unwrap().stamp;
        // The declare is idempotent and is the liveness signal, so it goes out
        // on every beat whether or not the content moved; the stamp is what
        // tells the GUI to refetch.
        osc::emit_declare(&session, &control.url, &stamp.to_string());
        last_stamp = stamp;
        std::thread::sleep(HEARTBEAT);
    }
    let _ = last_stamp;
    osc::emit_close(&session);
    Ok(())
}
