//! ytop — `htop` + fleet agent rows + booter + ZFS/LXC topology, for a yggterm fleet.

mod booter;
mod complaints;
mod fleet;
mod legendary;
mod manifest;
mod notebook;
mod osc;
mod probe;
mod rate;
mod rows;
mod schema;
mod server;
mod sysinternals;
mod timeline;

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
    about = "Modern infrastructure + fleet agent cockpit for yggterm (libyggterm document surface)"
)]
struct Args {
    /// Operational mode: "top" (machines, ZFS, LXC) or "dash" (agent fleet & jankbox)
    #[arg(long, value_parser = ["top", "dash"], default_value = "top")]
    mode: String,
    /// Dash subtab: "rows" (fleet table), "jankbox" (ytrace complaints +
    /// leaked/twin processes), or "supervision" (arming and quota holds).
    #[arg(long, value_parser = ["rows", "jankbox", "supervision"], default_value = "rows")]
    tab: String,
    /// Print one reading and exit, even inside yggterm.
    #[arg(long)]
    once: bool,
    /// With --once, print the raw JSON reading.
    #[arg(long)]
    json: bool,
    /// Run the host probe alone and print its JSON. `--probe <ssh-alias>` runs it there.
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    probe: Option<String>,
    /// Print a notebook without a GUI. `--notebook` alone lists the shelf;
    /// `--notebook <id>` lists its pages; add `--page <n>` for one page.
    ///
    /// ⭐ THE NOTEBOOKS ARE READINGS, SO THEY ARE CHECKABLE LIKE ONE. A page
    /// that can only be seen inside a running window cannot be verified without
    /// interrupting whoever is using that window — and the live blocks are the
    /// half most worth checking, because they are the half that can go wrong
    /// quietly.
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    notebook: Option<String>,
    /// Which page of `--notebook`, 1-based.
    #[arg(long)]
    page: Option<usize>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    manifest::write_best_effort();

    if let Some(id) = args.notebook {
        return server::print_notebook(id.trim(), args.page);
    }

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
        return server::print_once(&args.mode, &args.tab, args.json);
    }

    let control = server::spawn()?;
    {
        let mut pane = control.state.lock().unwrap();
        pane.view.mode = args.mode.clone();
    }

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
        osc::emit_declare(&session, &control.url, &stamp.to_string());
        last_stamp = stamp;
        std::thread::sleep(HEARTBEAT);
    }
    let _ = last_stamp;
    osc::emit_close(&session);
    Ok(())
}
