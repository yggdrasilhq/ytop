//! ytop — `htop` + fleet agent rows + booter, for a yggterm fleet.

mod booter;
mod fleet;
mod manifest;
mod osc;
mod probe;
mod rows;
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
    name = "ytop",
    version,
    about = "htop + fleet rows + booter for yggterm (libyggterm document-surface app)"
)]
struct Args {
    /// Which tab to open on.
    #[arg(long, value_parser = ["rows", "topology", "booter"], default_value = "rows")]
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

    let session = ["YGGTERM_SESSION_ID", "LC_YGGTERM_SESSION_ID"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    if args.once || session.is_empty() {
        if session.is_empty() && !args.once {
            eprintln!(
                "ytop: not running inside yggterm ($YGGTERM_SESSION_ID unset) — \
                 printing one reading instead of opening a surface."
            );
        }
        return server::print_once(&args.tab, args.json);
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
