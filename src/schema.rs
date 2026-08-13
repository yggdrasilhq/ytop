//! The widget schema — what yggterm paints.
//!
//! ⛔ EVERY WIDGET HERE ALREADY EXISTED. Nothing in this file is a new kind,
//! and that is the point: the app tier's claim is that a real desktop-class UI
//! can be composed from the vocabulary the host already has. An app that had to
//! invent a widget to be built would be evidence against the claim it was built
//! to demonstrate.
//!
//! The vocabulary used: `section` (with `card`), `label`, `search-box`,
//! `list-row` (with `status`, `actions`), `toggle`, `button`, `tabs`, and a
//! `footer` of labels. Contract: yggterm's `libyggterm-surfaces` skill.
//!
//! ⛔ AND NO CONTEXT MENU. `list-row` offers one; this app declines it, by
//! requirement. Every verb is a visible button on the row it acts on.
//!
//! ⭐ THE ONE WIDGET THIS APP WANTED AND DID NOT GET: a `meter` — a labelled
//! proportion bar, for memory and CPU. It is drawn as text here
//! ("24.1 / 62.7 GB · 38%") rather than invented, because the tier rules admit
//! a new declarative widget only when a SECOND app wants it. If the next app
//! wants a bar too, that is the evidence, and it is a host change — not a
//! canvas smuggled into this repo.

use crate::fleet::Machine;
use serde_json::{json, Value};

pub const TAB_TOPOLOGY: &str = "topology";
pub const TAB_BOOTER: &str = "booter";

/// What the pane is showing and what the user has typed into it.
pub struct View {
    pub tab: String,
    pub filter: String,
    /// The last action's outcome, shown once and plainly.
    pub notice: Option<String>,
}

impl Default for View {
    fn default() -> Self {
        Self { tab: TAB_TOPOLOGY.to_string(), filter: String::new(), notice: None }
    }
}

fn gb(kb: i64) -> String {
    format!("{:.1} GB", kb as f64 / 1024.0 / 1024.0)
}

fn mb(kb: i64) -> String {
    if kb >= 1024 * 1024 {
        gb(kb)
    } else {
        format!("{} MB", kb / 1024)
    }
}

fn duration(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    let (d, h, m) = (s / 86400, (s % 86400) / 3600, (s % 3600) / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

fn label(text: impl Into<String>) -> Value {
    json!({"kind": "label", "text": text.into()})
}

/// ⛔ THE FIELD IS `text`, NOT `title`. A `section` names its heading `text`
/// like a `label` does, while a `list-row` names its heading `title` — and a
/// hand-built schema that guesses wrong fails at RENDER, not at build, with the
/// whole pane refusing rather than one widget. Cost this app its first live
/// capture: "document schema is malformed: missing field `text`".
/// SSOT: `AppPaneWidget` in yggterm's shell crate.
fn section(text: impl Into<String>, card: bool) -> Value {
    json!({"kind": "section", "text": text.into(), "card": card})
}

/// ⛔ THE APP NAMES A DURABILITY CLASS; YGGTERM OWNS THE COLOUR. Reserved
/// tokens (amber, red) are deliberately NOT reachable from here — an app that
/// guessed at one would either be ignored or paint outside the product's own
/// vocabulary. An unreachable host therefore gets the EMPTY SLOT plus words
/// that say so, which is more honest than a colour that implies we measured it.
fn status_for(reading: &Value) -> &'static str {
    match (
        reading["ok"].as_bool().unwrap_or(false),
        reading["virt"].as_str().unwrap_or(""),
    ) {
        (false, _) => "",
        // The box itself outlives everything on it.
        (true, "none") => "durable",
        // A guest's existence is its host's to revoke.
        (true, _) => "transient",
    }
}

fn matches(filter: &str, haystack: &[&str]) -> bool {
    if filter.trim().is_empty() {
        return true;
    }
    let needle = filter.to_lowercase();
    haystack.iter().any(|h| h.to_lowercase().contains(&needle))
}

/// The tab strip, plus whatever the last action had to say.
fn header(view: &View) -> Vec<Value> {
    let mut widgets = vec![json!({
        "kind": "tabs",
        "id": "tab",
        "action": "tab",
        // ⛔ `active`, not `selected`. `list-row` spells its own version of this
        //    `selected`; `tabs` does not, and the wrong one is silently
        //    DEFAULTED rather than refused — so the pane renders with no tab
        //    highlighted and nothing says why.
        "active": view.tab,
        "tabs": [
            {"id": TAB_TOPOLOGY, "label": "Topology"},
            {"id": TAB_BOOTER, "label": "Booter"},
        ],
    })];
    if let Some(notice) = &view.notice {
        widgets.push(label(notice.clone()));
    }
    widgets
}

// ─── the topology tab ─────────────────────────────────────────────────────────

/// lstopo's shape — the machine, then what runs on it — with htop's numbers
/// laid over it.
pub fn topology(view: &View, machines: &[Machine]) -> Value {
    let mut widgets = header(view);
    widgets.push(json!({
        "kind": "search-box",
        "id": "filter",
        "action": "filter",
        "value": view.filter,
        "placeholder": "filter processes and containers",
    }));

    let (mut hosts, mut guests, mut procs, mut busy, mut ram_used, mut ram_total) =
        (0i64, 0i64, 0i64, 0f64, 0i64, 0i64);
    // ⛔ HOW MANY MACHINES COULD BE ASKED AT ALL. Without this the footer's
    //    container count is a lie by omission: a fleet of guests, none of which
    //    can see its siblings, would report "0 containers" — an assertion of
    //    absence built entirely out of our inability to look. Counting the
    //    machines that could answer is what turns that zero back into a
    //    measurement with a stated scope.
    let mut enumerable = 0i64;

    for machine in machines {
        let principal = machine.principal();
        if !machine.reachable() {
            widgets.push(section(
                format!("⚠ {} — could not be read", principal["label"].as_str()
                    .or(principal["host"].as_str()).unwrap_or("?")),
                true,
            ));
            // ⛔ "I could not look" is not "it is idle". Say which it is, and
            //    say what the failure was, because the next question is always
            //    "is the machine down or is my key wrong".
            widgets.push(label(format!(
                "{}. Nothing below is a measurement of this machine.",
                principal["error"].as_str().unwrap_or("no reason given")
            )));
            continue;
        }

        let cores = principal["cpu_count"].as_i64().unwrap_or(0);
        widgets.push(section(
            format!(
                "{}  ·  {} × {}",
                principal["hostname"].as_str().unwrap_or("?"),
                cores,
                principal["cpu_model"].as_str().unwrap_or("unknown cpu"),
            ),
            true,
        ));

        let load = principal["load"].as_array().cloned().unwrap_or_default();
        let load_text = load
            .iter()
            .filter_map(|v| v.as_f64())
            .map(|v| format!("{v:.2}"))
            .collect::<Vec<_>>()
            .join(" ");
        let total_kb = principal["mem_total_kb"].as_i64().unwrap_or(0);
        let avail_kb = principal["mem_available_kb"].as_i64().unwrap_or(0);
        let used_kb = (total_kb - avail_kb).max(0);
        let pct = if total_kb > 0 { used_kb * 100 / total_kb } else { 0 };
        ram_used += used_kb;
        ram_total += total_kb;

        widgets.push(label(format!(
            "kernel {}  ·  up {}  ·  load {load_text}",
            principal["kernel"].as_str().unwrap_or("?"),
            duration(principal["uptime_s"].as_f64().unwrap_or(0.0)),
        )));
        widgets.push(label(format!(
            "memory {} / {} · {pct}%    swap {} free of {}",
            gb(used_kb),
            gb(total_kb),
            mb(principal["swap_free_kb"].as_i64().unwrap_or(0)),
            mb(principal["swap_total_kb"].as_i64().unwrap_or(0)),
        )));

        // The yggterm hosts that live on this machine — one row each, which is
        // where a guest becomes visible AS a guest rather than as a machine.
        for reading in &machine.readings {
            hosts += 1;
            let host = reading["host"].as_str().unwrap_or("?");
            let shown = reading["label"].as_str().unwrap_or(host);
            let virt = reading["virt"].as_str().unwrap_or("?");
            let cpu = reading["cpu_busy_pct"].as_f64().unwrap_or(0.0);
            busy += cpu;
            procs += reading["procs_total"].as_i64().unwrap_or(0);
            widgets.push(json!({
                "kind": "list-row",
                "id": format!("host:{host}"),
                "title": format!("{shown}  ({})", if virt == "none" { "the machine itself" } else { virt }),
                "subtitle": format!(
                    "{:.0}% cpu · {} procs · sampled over {} ms",
                    cpu,
                    reading["procs_total"].as_i64().unwrap_or(0),
                    reading["sample_ms"].as_i64().unwrap_or(0),
                ),
                "status": status_for(reading),
            }));
        }

        // ⚠ CONTAINERS ARE FIRST-CLASS, AND AN EMPTY LIST IS NOT A FACT.
        //    Only a container HOST can enumerate guests; from inside one the
        //    answer is unknowable, and printing "0 containers" there would be
        //    an assertion we are not entitled to make.
        let tool = principal["container_tool"].as_str().unwrap_or("");
        if !tool.is_empty() {
            enumerable += 1;
        }
        let containers = principal["containers"].as_array().cloned().unwrap_or_default();
        if !containers.is_empty() {
            for c in containers {
                let name = c["name"].as_str().unwrap_or("?");
                let state = c["state"].as_str().unwrap_or("?");
                if !matches(&view.filter, &[name, state]) {
                    continue;
                }
                guests += 1;
                widgets.push(json!({
                    "kind": "list-row",
                    "id": format!("container:{name}"),
                    "title": format!("📦\u{fe0e} {name}"),
                    "subtitle": format!("container · {state} · via {tool}"),
                    "status": if state.eq_ignore_ascii_case("running") { "transient" } else { "" },
                }));
            }
        } else if tool.is_empty() {
            widgets.push(label(
                "containers: not enumerable from here — a guest cannot see its siblings",
            ));
        }

        // htop's half: what is actually burning the machine right now.
        for reading in &machine.readings {
            let host = reading["host"].as_str().unwrap_or("?");
            let host_label = reading["label"].as_str().unwrap_or(host);
            let top = reading["top"].as_array().cloned().unwrap_or_default();
            let shown: Vec<&Value> = top
                .iter()
                .filter(|p| {
                    matches(
                        &view.filter,
                        &[
                            p["comm"].as_str().unwrap_or(""),
                            p["cmd"].as_str().unwrap_or(""),
                            p["user"].as_str().unwrap_or(""),
                        ],
                    )
                })
                .collect();
            if shown.is_empty() {
                continue;
            }
            widgets.push(section(format!("{host_label} — busiest processes"), false));
            for p in shown {
                widgets.push(json!({
                    "kind": "list-row",
                    "id": format!("proc:{host}:{}", p["pid"].as_i64().unwrap_or(0)),
                    "title": format!(
                        "{:>6.1}%  {:>9}  {}",
                        p["cpu_pct"].as_f64().unwrap_or(0.0),
                        mb(p["rss_kb"].as_i64().unwrap_or(0)),
                        p["comm"].as_str().unwrap_or("?"),
                    ),
                    "subtitle": format!(
                        "pid {} · {} · {}",
                        p["pid"].as_i64().unwrap_or(0),
                        p["user"].as_str().unwrap_or("?"),
                        p["cmd"].as_str().unwrap_or(""),
                    ),
                }));
            }
        }
    }

    let ram_pct = if ram_total > 0 { ram_used * 100 / ram_total } else { 0 };
    json!({
        "title": "yggtopo — the fleet",
        "widgets": widgets,
        "footer": [
            label(format!(
                "{} machines · {hosts} hosts · containers: {} · {procs} processes · \
                 {busy:.0}% cpu · {} / {} ram · {ram_pct}%",
                machines.len(),
                if enumerable == 0 {
                    "not enumerable from any host here".to_string()
                } else {
                    format!("{guests} found, asking {enumerable} of {} machines",
                            machines.len())
                },
                gb(ram_used), gb(ram_total),
            )),
            json!({"kind": "button", "id": "refresh", "action": "refresh", "label": "Refresh"}),
        ],
    })
}

// ─── the same view, at document scale ─────────────────────────────────────────

/// The VIEWPORT pane.
///
/// ⛔⛔ A DOCUMENT SURFACE'S BODY IS `markdown` AND MULTILINE `text-input`,
/// AND NOTHING ELSE. Tabs, buttons, toggles, labels, sections and **list-rows**
/// are CHROME there: the host lifts them into a top bar and the body stays
/// empty. Measured on a real GUI, not a shadow — the identical schema rendered
/// completely in the rail and left the viewport blank in the same minute, which
/// is the control that separates "my schema is wrong" from "this placement does
/// not paint these widgets".
///
/// ⚠ The prose contract says list-rows "render at document scale"; the host's
/// own deserialiser says otherwise, and the host is the SSOT. An app that
/// believes the prose ships a blank page and has no way to find out why — every
/// telemetry field reads healthy, because nothing failed.
///
/// ⇒ So the viewport gets the SAME reading as markdown, and the rail keeps the
/// interactive version. Not a downgrade: a topology IS a document, and the
/// verbs that need pressing were always going to live beside it.
pub fn topology_document(view: &View, machines: &[Machine]) -> Value {
    let mut widgets = header(view);
    widgets.push(json!({
        "kind": "search-box",
        "id": "filter",
        "action": "filter",
        "value": view.filter,
        "placeholder": "filter processes and containers",
    }));
    widgets.push(json!({
        "kind": "markdown",
        "id": "body",
        "source": markdown_body(view, machines),
    }));
    json!({ "title": "yggtopo — the fleet", "widgets": widgets })
}

fn markdown_body(view: &View, machines: &[Machine]) -> String {
    let mut out = String::new();
    for machine in machines {
        let principal = machine.principal();
        let name = principal["label"]
            .as_str()
            .or(principal["host"].as_str())
            .unwrap_or("?");
        if !machine.reachable() {
            out.push_str(&format!(
                "## ⚠ {name} — could not be read\n\n{}. Nothing here is a measurement \
                 of this machine.\n\n",
                principal["error"].as_str().unwrap_or("no reason given"),
            ));
            continue;
        }
        let total_kb = principal["mem_total_kb"].as_i64().unwrap_or(0);
        let avail_kb = principal["mem_available_kb"].as_i64().unwrap_or(0);
        let used_kb = (total_kb - avail_kb).max(0);
        let load = principal["load"]
            .as_array()
            .map(|l| {
                l.iter()
                    .filter_map(|v| v.as_f64())
                    .map(|v| format!("{v:.2}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        out.push_str(&format!(
            "## {}  ·  {} × {}\n\nkernel {} · up {} · load {load} · memory {} / {} \
             ({}%)\n\n",
            principal["hostname"].as_str().unwrap_or("?"),
            principal["cpu_count"].as_i64().unwrap_or(0),
            principal["cpu_model"].as_str().unwrap_or("unknown cpu"),
            principal["kernel"].as_str().unwrap_or("?"),
            duration(principal["uptime_s"].as_f64().unwrap_or(0.0)),
            gb(used_kb),
            gb(total_kb),
            if total_kb > 0 { used_kb * 100 / total_kb } else { 0 },
        ));

        out.push_str("| host | kind | cpu | processes |\n|---|---|---:|---:|\n");
        for reading in &machine.readings {
            let virt = reading["virt"].as_str().unwrap_or("?");
            out.push_str(&format!(
                "| {} | {} | {:.0}% | {} |\n",
                reading["label"].as_str().or(reading["host"].as_str()).unwrap_or("?"),
                if virt == "none" { "the machine itself" } else { virt },
                reading["cpu_busy_pct"].as_f64().unwrap_or(0.0),
                reading["procs_total"].as_i64().unwrap_or(0),
            ));
        }
        out.push('\n');

        let tool = principal["container_tool"].as_str().unwrap_or("");
        let containers = principal["containers"].as_array().cloned().unwrap_or_default();
        if tool.is_empty() {
            out.push_str(
                "*containers: not enumerable from here — a guest cannot see its siblings*\n\n",
            );
        } else if containers.is_empty() {
            out.push_str(&format!("*containers: none running, asked via `{tool}`*\n\n"));
        } else {
            out.push_str("| container | state |\n|---|---|\n");
            for c in containers {
                let name = c["name"].as_str().unwrap_or("?");
                let state = c["state"].as_str().unwrap_or("?");
                if matches(&view.filter, &[name, state]) {
                    out.push_str(&format!("| 📦 {name} | {state} |\n"));
                }
            }
            out.push('\n');
        }

        for reading in &machine.readings {
            let rows: Vec<&Value> = reading["top"]
                .as_array()
                .map(|top| {
                    top.iter()
                        .filter(|p| {
                            matches(
                                &view.filter,
                                &[
                                    p["comm"].as_str().unwrap_or(""),
                                    p["cmd"].as_str().unwrap_or(""),
                                    p["user"].as_str().unwrap_or(""),
                                ],
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            if rows.is_empty() {
                continue;
            }
            out.push_str(&format!(
                "### {} — busiest processes\n\n| cpu | memory | process | pid | user |\n\
                 |---:|---:|---|---:|---|\n",
                reading["label"].as_str().or(reading["host"].as_str()).unwrap_or("?"),
            ));
            for p in rows {
                out.push_str(&format!(
                    "| {:.1}% | {} | {} | {} | {} |\n",
                    p["cpu_pct"].as_f64().unwrap_or(0.0),
                    mb(p["rss_kb"].as_i64().unwrap_or(0)),
                    p["comm"].as_str().unwrap_or("?"),
                    p["pid"].as_i64().unwrap_or(0),
                    p["user"].as_str().unwrap_or("?"),
                ));
            }
            out.push('\n');
        }
    }
    if out.is_empty() {
        // ⛔ NOT "the fleet is empty". Nothing has been read YET.
        out.push_str("*reading the fleet…*\n");
    }
    out
}

// ─── the booter tab ───────────────────────────────────────────────────────────

/// Who is armed, when they are due, and the switch that turns it off.
///
/// ⭐ THIS TAB IS THE POINT OF THE APP. The booter could always be disarmed —
/// by someone with a shell on the right machine who knew the verb. That is not
/// an off switch; it is a rumour of one. The complaint that produced this app
/// was that a human could not answer "is this what is hurting me" without
/// becoming an engineer first.
pub fn booter(view: &View, states: &[Value]) -> Value {
    let mut widgets = header(view);
    let (mut armed_n, mut subs_n) = (0i64, 0i64);
    let mut soonest: Option<(i64, String)> = None;

    for state in states {
        let host = state["host"].as_str().unwrap_or("?");
        if state.get("armed").is_none() {
            widgets.push(section(format!("⚠ {host} — the booter could not be read"), true));
            widgets.push(label(
                state["error"].as_str().or(state["message"].as_str())
                    .unwrap_or("no reason given").to_string(),
            ));
            continue;
        }
        let armed = state["armed"].as_bool().unwrap_or(false);
        if armed {
            armed_n += 1;
        }
        widgets.push(section(format!("{host}"), true));

        // ⛔ THE SWITCH IS A TOGGLE, NOT TWO BUTTONS. Arm and disarm are one
        //    state with two positions; a pair of buttons lets the surface show
        //    both as available at once, which is the shape that makes a human
        //    ask "which one am I in".
        widgets.push(json!({
            "kind": "toggle",
            "id": format!("armed:{host}"),
            "action": format!("armed:{host}"),
            "label": "Booter armed",
            "value": armed,
        }));

        let watcher = &state["watcher"];
        let mut line = if watcher["alive"].as_bool().unwrap_or(false) {
            format!("watcher pid {}", watcher["pid"].as_i64().unwrap_or(0))
        } else {
            "⛔ no watcher process — nothing is watching".to_string()
        };
        if let Some(age) = watcher["heartbeat_age_s"].as_i64() {
            line.push_str(&format!(" · heartbeat {age}s ago"));
        }
        // ⛔ ALIVE IS NOT AUDIBLE. A watcher that ticks into a closed log is
        //    indistinguishable from a healthy one from the outside, and drawing
        //    it green would republish exactly the lie its own status verb was
        //    fixed to stop telling.
        if watcher["mute"].as_bool().unwrap_or(false) {
            line.push_str(" · ⛔ MUTE: it is ticking silently and cannot be diagnosed");
        }
        widgets.push(label(line));

        if let Some(disarm) = state["disarm"].as_object() {
            let until = disarm.get("until").and_then(Value::as_f64).unwrap_or(0.0);
            let now = state["now"].as_f64().unwrap_or(0.0);
            widgets.push(label(format!(
                "⛔ disarmed {} — {}",
                if until == 0.0 {
                    "until re-armed by hand".to_string()
                } else {
                    format!("{} left", duration(until - now))
                },
                disarm.get("note").and_then(Value::as_str).unwrap_or("no reason given"),
            )));
        }
        if let Some(hold) = state["rate_limit_hold"].as_object() {
            widgets.push(label(format!(
                "⏸ quota hold — {} left. A boot into an exhausted quota is refused \
                 before the agent runs, so nobody is being kicked.",
                duration(hold.get("secs_left").and_then(Value::as_f64).unwrap_or(0.0)),
            )));
        }

        for sub in state["subscribers"].as_array().cloned().unwrap_or_default() {
            subs_n += 1;
            let uuid = sub["uuid"].as_str().unwrap_or("");
            let short = uuid.get(..8).unwrap_or(uuid);
            let campaign = sub["campaign"].as_str().unwrap_or("");
            let note = sub["note"].as_str().unwrap_or("");
            if !matches(&view.filter, &[short, campaign, note]) {
                continue;
            }
            // ⚠ `due_in_s` is present only when the state was actually
            //    classified, and null when the look failed. Absent and null are
            //    both "we do not know" and must read as that — never as "not
            //    due", which is a verdict nobody earned.
            let due = match (sub.get("due_in_s"), sub.get("state").and_then(Value::as_str)) {
                (Some(Value::Number(n)), _) => {
                    let secs = n.as_i64().unwrap_or(0);
                    if soonest.as_ref().map(|(s, _)| secs < *s).unwrap_or(true) {
                        soonest = Some((secs, format!("{short} on {host}")));
                    }
                    format!("due in {}", duration(secs as f64))
                }
                (_, Some(s)) => format!("{}, not due", s.to_lowercase()),
                _ => "due unknown — not classified".to_string(),
            };
            widgets.push(json!({
                "kind": "list-row",
                "id": format!("sub:{host}:{uuid}"),
                "title": format!("{short}  {}", if campaign.is_empty() { "—" } else { campaign }),
                "subtitle": format!(
                    "{} · {}h old · window {}m · boots {}{}{}",
                    due,
                    sub["age_h"].as_f64().unwrap_or(0.0),
                    sub["boot_window_secs"].as_i64().unwrap_or(0) / 60,
                    sub["boots"].as_i64().unwrap_or(0),
                    if sub["kind"].as_str() == Some("monitor") { " · monitor" } else { "" },
                    if note.is_empty() { String::new() } else { format!(" · {note}") },
                ),
                // A monitor's watch outlives the task that started it; a task's
                // does not. That is the durability difference the dot names.
                "status": if sub["kind"].as_str() == Some("monitor") { "durable" } else { "transient" },
                // ⛔ Buttons on the row, never a context menu (by requirement).
                "actions": [
                    {"action": format!("defer:{host}:{uuid}"), "label": "Defer 30m",
                     "title": "widen this row's boot window for one long wait"},
                    {"action": format!("unsub:{host}:{uuid}"), "label": "Unsubscribe",
                     "title": "stop watching this row"},
                ],
            }));
        }
    }

    let next_due = soonest
        .map(|(s, who)| format!("next due: {who} in {}", duration(s as f64)))
        .unwrap_or_else(|| "next due: unknown".to_string());
    json!({
        "title": "yggtopo — the booter",
        "widgets": widgets,
        "footer": [
            label(format!("{armed_n} of {} hosts armed · {subs_n} subscribers · {next_due}",
                          states.len())),
            json!({"kind": "button", "id": "refresh", "action": "refresh", "label": "Refresh"}),
        ],
    })
}

/// The booter tab at document scale.
///
/// ⭐ THE SWITCH SURVIVES THE TRANSLATION, WHICH IS WHY THIS TAB WORKS IN BOTH
/// PLACEMENTS. A `toggle` is chrome, so the host lifts it into the top bar and
/// it stays pressable; only the subscriber LIST has to become prose. The half a
/// human needs to act on is the half that travels.
pub fn booter_document(view: &View, states: &[Value]) -> Value {
    let mut widgets = header(view);
    for state in states {
        let host = state["host"].as_str().unwrap_or("?");
        if state.get("armed").is_none() {
            continue;
        }
        widgets.push(json!({
            "kind": "toggle",
            "id": format!("armed:{host}"),
            "action": format!("armed:{host}"),
            "label": format!("{host} armed"),
            "value": state["armed"].as_bool().unwrap_or(false),
        }));
    }
    let mut body = String::new();
    for state in states {
        let host = state["host"].as_str().unwrap_or("?");
        if state.get("armed").is_none() {
            body.push_str(&format!(
                "## ⚠ {host} — the booter could not be read\n\n{}\n\n",
                state["error"].as_str().or(state["message"].as_str())
                    .unwrap_or("no reason given"),
            ));
            continue;
        }
        let watcher = &state["watcher"];
        body.push_str(&format!(
            "## {host} — {}\n\n{}{}\n\n",
            if state["armed"].as_bool().unwrap_or(false) { "armed" } else { "⛔ DISARMED" },
            if watcher["alive"].as_bool().unwrap_or(false) {
                format!("watcher pid {}", watcher["pid"].as_i64().unwrap_or(0))
            } else {
                "⛔ no watcher process — nothing is watching".to_string()
            },
            if watcher["mute"].as_bool().unwrap_or(false) {
                " · ⛔ MUTE: ticking silently, its decisions cannot be diagnosed"
            } else {
                ""
            },
        ));
        if let Some(hold) = state["rate_limit_hold"].as_object() {
            body.push_str(&format!(
                "⏸ **quota hold** — {} left; a boot into an exhausted quota is refused \
                 before the agent runs.\n\n",
                duration(hold.get("secs_left").and_then(Value::as_f64).unwrap_or(0.0)),
            ));
        }
        let subs = state["subscribers"].as_array().cloned().unwrap_or_default();
        if subs.is_empty() {
            body.push_str("*no subscribers*\n\n");
            continue;
        }
        body.push_str("| row | campaign | due | window | boots | kind |\n\
                       |---|---|---|---:|---:|---|\n");
        for sub in subs {
            let uuid = sub["uuid"].as_str().unwrap_or("");
            let short = uuid.get(..8).unwrap_or(uuid);
            let campaign = sub["campaign"].as_str().unwrap_or("—");
            if !matches(&view.filter, &[short, campaign]) {
                continue;
            }
            // Same three-way reading as the rail: a number, a state, or an
            // admission. Never a silent blank, which reads as "fine".
            let due = match (sub.get("due_in_s"), sub.get("state").and_then(Value::as_str)) {
                (Some(Value::Number(n)), _) => duration(n.as_i64().unwrap_or(0) as f64),
                (_, Some(s)) => format!("{}, not due", s.to_lowercase()),
                _ => "unknown".to_string(),
            };
            body.push_str(&format!(
                "| `{short}` | {campaign} | {due} | {}m | {} | {} |\n",
                sub["boot_window_secs"].as_i64().unwrap_or(0) / 60,
                sub["boots"].as_i64().unwrap_or(0),
                sub["kind"].as_str().unwrap_or("task"),
            ));
        }
        body.push('\n');
    }
    widgets.push(json!({"kind": "markdown", "id": "body", "source": body}));
    json!({ "title": "yggtopo — the booter", "widgets": widgets })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fleet::Machine;

    fn reading(host: &str, ok: bool, virt: &str) -> Value {
        json!({"ok": ok, "host": host, "hostname": host, "virt": virt,
               "kernel": "9.9.9-invented", "btime": 1700000000, "cpu_count": 8,
               "cpu_model": "Example CPU E1", "load": [0.5, 0.4, 0.3],
               "mem_total_kb": 8_000_000, "mem_available_kb": 4_000_000,
               "swap_total_kb": 0, "swap_free_kb": 0, "uptime_s": 3600.0,
               "procs_total": 120, "cpu_busy_pct": 12.5, "sample_ms": 400,
               "containers": [], "container_tool": "",
               "top": [{"pid": 11, "comm": "example-daemon", "cmd": "example-daemon --serve",
                        "cpu_pct": 9.5, "rss_kb": 65_536, "user": "someone"}]})
    }

    fn kinds(schema: &Value) -> Vec<String> {
        schema["widgets"].as_array().unwrap().iter()
            .filter_map(|w| w["kind"].as_str().map(str::to_string)).collect()
    }

    /// ⛔⛔ THE FIELD NAMES ARE PART OF THE CONTRACT AND A HAND-BUILT SCHEMA
    /// CANNOT BE CHECKED BY THE COMPILER. This app is the third consumer to
    /// copy the app scaffolding by hand, and it paid the predicted price on its
    /// first live capture: `section` was given `title` (its heading is `text`)
    /// and `tabs` was given `selected` (its selection is `active`). One of
    /// those FAILED the whole pane; the other was silently defaulted, which is
    /// worse — the pane rendered with no tab highlighted and nothing said why.
    ///
    /// ⇒ Until the widget schema is lifted into a typed contract crate (the
    /// platform's own migration order calls for it once a second consumer
    /// exists — we are the third), this test is the only thing standing between
    /// a renamed field and a blank pane. SSOT for every name asserted here:
    /// `AppPaneWidget` / `AppPaneSchema` in yggterm's shell crate.
    #[test]
    fn every_widget_carries_the_field_names_the_host_deserialises() {
        let required: &[(&str, &[&str])] = &[
            ("section", &["text"]),
            ("label", &["text"]),
            ("tabs", &["id", "tabs", "active"]),
            ("search-box", &["id"]),
            ("toggle", &["id", "label"]),
            ("button", &["id", "label", "action"]),
            ("list-row", &["id", "title"]),
        ];
        let machines = vec![Machine {
            key: Some("k".into()),
            readings: vec![reading("alpha", true, "none")],
        }];
        let booter_state = json!({
            "host": "alpha", "armed": true, "now": 0.0, "disarm": null,
            "rate_limit_hold": null,
            "watcher": {"alive": true, "pid": 1, "heartbeat_age_s": 3, "mute": false},
            "subscribers": [{"uuid": "aaaabbbb-1111-2222-3333-444455556666",
                             "campaign": "demo", "note": "", "kind": "task",
                             "age_h": 1.0, "boots": 0, "boot_window_secs": 420}],
        });
        let view = View::default();
        for schema in [topology(&view, &machines), booter(&view, &[booter_state])] {
            // The wrapper's own shape, which decides whether anything below it
            // is even looked at.
            assert!(schema["title"].is_string());
            assert!(schema["widgets"].is_array());
            assert!(schema["footer"].is_array());
            let all: Vec<&Value> = schema["widgets"].as_array().unwrap().iter()
                .chain(schema["footer"].as_array().unwrap()).collect();
            for widget in all {
                let kind = widget["kind"].as_str().unwrap();
                let fields = required.iter().find(|(k, _)| *k == kind)
                    .unwrap_or_else(|| panic!("no contract recorded for kind {kind}")).1;
                for field in fields {
                    assert!(widget.get(field).is_some(),
                            "{kind} is missing the required field `{field}` — \
                             the pane would refuse to render");
                }
                // The two that actually bit, asserted as absences so a revert
                // to the guessed spelling fails here instead of on a screen.
                if kind == "section" {
                    assert!(widget.get("title").is_none(),
                            "a section's heading is `text`; `title` is the list-row spelling");
                }
                if kind == "tabs" {
                    assert!(widget.get("selected").is_none(),
                            "tabs select with `active`; `selected` is the list-row spelling");
                }
            }
        }
    }

    #[test]
    fn every_widget_kind_is_one_the_host_already_paints() {
        // ⛔ THE TEST THIS APP EXISTS TO PASS. Inventing a widget would make the
        //    pane fail to render rather than fail loudly here, and the whole
        //    claim of the app tier is that this list never needed to grow.
        const KNOWN: &[&str] = &["section", "label", "search-box", "text-input",
                                 "number-input", "toggle", "button", "list-row", "tabs"];
        let machines = vec![Machine {
            key: Some("k".into()),
            readings: vec![reading("alpha", true, "none")],
        }];
        let view = View::default();
        for schema in [topology(&view, &machines), booter(&view, &[json!({
            "host": "alpha", "armed": true, "now": 0.0, "disarm": null,
            "rate_limit_hold": null,
            "watcher": {"alive": true, "pid": 1, "heartbeat_age_s": 3, "mute": false},
            "subscribers": [{"uuid": "aaaabbbb-1111-2222-3333-444455556666",
                             "campaign": "demo", "note": "", "kind": "task",
                             "age_h": 1.0, "boots": 0, "boot_window_secs": 420}],
        })])] {
            for kind in kinds(&schema) {
                assert!(KNOWN.contains(&kind.as_str()), "invented widget kind: {kind}");
            }
            for widget in schema["footer"].as_array().unwrap() {
                let kind = widget["kind"].as_str().unwrap();
                assert!(matches!(kind, "label" | "toggle" | "button"),
                        "the footer vocabulary is a subset: {kind}");
            }
        }
    }

    #[test]
    fn no_row_offers_a_context_menu() {
        let machines = vec![Machine { key: Some("k".into()),
                                      readings: vec![reading("alpha", true, "none")] }];
        let schema = topology(&View::default(), &machines);
        for widget in schema["widgets"].as_array().unwrap() {
            assert!(widget.get("menu").is_none(), "this app declines context menus");
        }
    }

    #[test]
    fn an_unreachable_host_is_reported_as_unread_not_as_idle() {
        let mut down = reading("beta", false, "");
        down["error"] = json!("timed out");
        let machines = vec![Machine { key: None, readings: vec![down] }];
        let text = topology(&View::default(), &machines).to_string();
        assert!(text.contains("could not be read"));
        assert!(text.contains("Nothing below is a measurement"));
    }

    #[test]
    fn a_guest_cannot_claim_its_machine_has_no_containers() {
        // Absence of a container tool means we could not enumerate, and the
        // pane must say that instead of printing a zero.
        let machines = vec![Machine { key: Some("k".into()),
                                      readings: vec![reading("alpha", true, "lxc")] }];
        let text = topology(&View::default(), &machines).to_string();
        assert!(text.contains("not enumerable from here"));
        assert!(!text.contains("0 containers"));
    }

    #[test]
    fn a_subscriber_that_was_never_classified_is_not_reported_as_safe() {
        let state = json!({
            "host": "alpha", "armed": true, "now": 0.0, "disarm": null,
            "rate_limit_hold": null,
            "watcher": {"alive": true, "pid": 1, "heartbeat_age_s": 3, "mute": false},
            "subscribers": [{"uuid": "aaaabbbb-1111-2222-3333-444455556666",
                             "campaign": "demo", "note": "", "kind": "task",
                             "age_h": 1.0, "boots": 0, "boot_window_secs": 420}],
        });
        let text = booter(&View::default(), &[state]).to_string();
        assert!(text.contains("due unknown"), "unclassified must read as unknown");
    }

    #[test]
    fn a_mute_watcher_is_never_drawn_as_healthy() {
        let state = json!({
            "host": "alpha", "armed": true, "now": 0.0, "disarm": null,
            "rate_limit_hold": null,
            "watcher": {"alive": true, "pid": 1, "heartbeat_age_s": 3, "mute": true},
            "subscribers": [],
        });
        assert!(booter(&View::default(), &[state]).to_string().contains("MUTE"));
    }

    #[test]
    fn the_status_dot_only_ever_names_a_class_the_host_paints() {
        for reading in [reading("a", true, "none"), reading("b", true, "lxc"),
                        reading("c", false, "")] {
            assert!(matches!(status_for(&reading), "durable" | "transient" | ""));
        }
    }
}
