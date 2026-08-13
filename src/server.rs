//! yggtopo's control endpoint, and the sampling loop behind it.
//!
//! `GET /ping` (liveness + the change stamp), `GET /pane/<id>` (the schema the
//! GUI paints), `POST /action` (everything the user does in it). Hand-rolled
//! HTTP over loopback — the same shape every libyggterm app uses, and small
//! enough that a dependency would cost more than it saved.

use crate::{booter, fleet, probe, schema};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How often the machines are re-read.
///
/// ⚠ THIS IS AN SSH FAN-OUT, NOT A LOCAL READ. htop refreshes twice a second
/// because it reads `/proc`; a fleet view reaching three machines cannot, and
/// pretending otherwise just means every refresh overlaps the last. Two seconds
/// is fast enough to watch a build start and slow enough that the probes never
/// queue behind each other.
const TOPOLOGY_EVERY_DEFAULT_SECS: u64 = 2;

/// ⚠ A KNOB, BECAUSE THE RIGHT NUMBER IS A PROPERTY OF THE FLEET, NOT OF THIS
/// FILE. Three machines on a LAN want two seconds; a dozen over a slow link
/// want twenty, and hard-coding either makes the other unusable.
fn topology_every() -> Duration {
    Duration::from_secs(
        std::env::var("YGGTOPO_REFRESH_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|v| *v > 0)
            .unwrap_or(TOPOLOGY_EVERY_DEFAULT_SECS),
    )
}

/// How often the booter plane is re-read.
///
/// ⛔ SLOWER ON PURPOSE, BECAUSE `--due` CLASSIFIES. Answering "when is this row
/// due" costs a live row-list call and a transcript read per subscriber — the
/// same work the booter's own tick does. At the topology cadence that would
/// mean doing a watchdog's work several times a second to draw a label.
const BOOTER_EVERY: Duration = Duration::from_secs(15);

const PROBE_TIMEOUT: Duration = Duration::from_secs(12);

pub struct PaneState {
    pub view: schema::View,
    pub machines: Vec<fleet::Machine>,
    pub booter_states: Vec<Value>,
    /// Bumped whenever the painted content could have changed. The GUI refetches
    /// a pane only when this moves, so it is the whole refresh mechanism.
    pub stamp: u64,
}

impl PaneState {
    fn touch(&mut self) {
        self.stamp = self.stamp.wrapping_add(1);
    }
}

pub struct Server {
    pub url: String,
    pub state: Arc<Mutex<PaneState>>,
}

pub fn spawn() -> Result<Server> {
    let listener = TcpListener::bind("127.0.0.1:0").context("binding the yggtopo control server")?;
    let port = listener.local_addr()?.port();
    let state = Arc::new(Mutex::new(PaneState {
        view: schema::View::default(),
        machines: Vec::new(),
        booter_states: Vec::new(),
        stamp: 0,
    }));
    {
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            for incoming in listener.incoming() {
                let Ok(stream) = incoming else { continue };
                let state = Arc::clone(&state);
                std::thread::spawn(move || handle_conn(stream, &state));
            }
        });
    }
    {
        let state = Arc::clone(&state);
        std::thread::spawn(move || sampler(state));
    }
    Ok(Server { url: format!("http://127.0.0.1:{port}"), state })
}

/// The reading loop.
///
/// ⛔ IT SAMPLES OUTSIDE THE LOCK. Holding the pane mutex across an ssh fan-out
/// would block every schema GET for the length of the slowest machine, so a
/// host that is merely slow would look like an app that has frozen. Read first,
/// then take the lock only to swap the result in.
fn sampler(state: Arc<Mutex<PaneState>>) {
    let mut last_booter = Instant::now() - BOOTER_EVERY;
    loop {
        let hosts = fleet::roster();
        let readings = fleet::read_all(&hosts, PROBE_TIMEOUT);
        let machines = fleet::group(readings);
        {
            let mut pane = state.lock().unwrap();
            pane.machines = machines;
            pane.touch();
        }

        if last_booter.elapsed() >= BOOTER_EVERY {
            last_booter = Instant::now();
            let states: Vec<Value> = hosts
                .iter()
                .map(|host| {
                    let target = if host == fleet::LOCAL { None } else { Some(host.as_str()) };
                    booter::state(target, PROBE_TIMEOUT)
                })
                .collect();
            let mut pane = state.lock().unwrap();
            pane.booter_states = states;
            pane.touch();
        }
        std::thread::sleep(topology_every());
    }
}

/// ⛔ THE TWO PLACEMENTS PAINT DIFFERENT VOCABULARIES, SO THEY GET DIFFERENT
/// SCHEMAS OF THE SAME VIEW. The rail renders every widget kind; the document
/// body renders only `markdown` and multiline `text-input` and lifts the rest
/// into a top bar. Serving the rail's schema to the viewport is how an app ends
/// up with a blank page and a clean bill of health.
///
/// ⚠ ONE STATE, TWO RENDERINGS — never two states. The tab and the filter are
/// the user's, not the pane's; a rail showing a different tab from the viewport
/// beside it would be two answers to "what am I looking at".
fn schema_for(pane: &PaneState, document: bool) -> Value {
    match (pane.view.tab == schema::TAB_BOOTER, document) {
        (true, true) => schema::booter_document(&pane.view, &pane.booter_states),
        (true, false) => schema::booter(&pane.view, &pane.booter_states),
        (false, true) => schema::topology_document(&pane.view, &pane.machines),
        (false, false) => schema::topology(&pane.view, &pane.machines),
    }
}

fn handle_conn(stream: TcpStream, state: &Mutex<PaneState>) {
    let Ok(peek) = stream.try_clone() else { return };
    let mut reader = BufReader::new(peek);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let (path, _query) = target.split_once('?').unwrap_or((target, ""));

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let body: Value = if content_length > 0 {
        let mut raw = vec![0u8; content_length];
        if reader.read_exact(&mut raw).is_err() {
            return;
        }
        serde_json::from_slice(&raw).unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    match (method, path) {
        ("GET", "/ping") => {
            let pane = state.lock().unwrap();
            respond(stream, 200, &json!({
                "ok": true,
                "app_name": "Yggtopo",
                "document_version": pane.stamp.to_string(),
            }));
        }
        // Both panes render the same view. One app, one state: the rail and the
        // viewport showing different tabs would be two sources of truth about
        // what the user is looking at.
        ("GET", "/pane/topo") => {
            let pane = state.lock().unwrap();
            respond(stream, 200, &schema_for(&pane, true));
        }
        ("GET", "/pane/rail") => {
            let pane = state.lock().unwrap();
            respond(stream, 200, &schema_for(&pane, false));
        }
        ("POST", "/action") => {
            let reply = handle_action(state, &body);
            respond(stream, 200, &reply);
        }
        _ => respond(stream, 404, &json!({})),
    }
}

/// The widget's own value rides `values.value`; a field may also arrive under
/// its widget id. Read both, prefer the explicit one.
fn posted_value(values: &Value, id: &str) -> String {
    values
        .get("value")
        .and_then(Value::as_str)
        .or_else(|| values.get(id).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}

fn handle_action(state: &Mutex<PaneState>, body: &Value) -> Value {
    let action = body["action"].as_str().unwrap_or_default().to_string();
    let values = body["values"].clone();

    // ⛔ THE WRITE VERBS RUN OUTSIDE THE LOCK. `disarm` on a remote host is an
    //    ssh round trip; doing it under the pane mutex would freeze the pane
    //    for its duration, and a surface that stops repainting the instant you
    //    press its button is indistinguishable from one that crashed.
    let outcome: Option<String> = if let Some(rest) = action.strip_prefix("armed:") {
        let host = rest.to_string();
        let target = (host != fleet::LOCAL).then_some(host.as_str());
        // The toggle posts the state it is moving TO.
        let want_armed = matches!(
            posted_value(&values, &action).to_ascii_lowercase().as_str(),
            "true" | "1" | "on" | "yes"
        );
        let reply = if want_armed {
            booter::arm(target, PROBE_TIMEOUT)
        } else {
            booter::disarm(
                target,
                Some(4.0),
                "disarmed from yggtopo",
                PROBE_TIMEOUT,
            )
        };
        Some(summarise(
            &reply,
            &if want_armed {
                format!("{host}: booter armed")
            } else {
                format!("{host}: booter disarmed for 4h — subscriptions kept, and it re-arms itself")
            },
        ))
    } else if let Some(rest) = action.strip_prefix("defer:") {
        let (host, uuid) = rest.split_once(':').unwrap_or((rest, ""));
        let target = (host != fleet::LOCAL).then_some(host);
        let reply = booter::defer(target, uuid, 1800, PROBE_TIMEOUT);
        Some(summarise(&reply, &format!("{}: boot window widened", &uuid[..8.min(uuid.len())])))
    } else if let Some(rest) = action.strip_prefix("unsub:") {
        let (host, uuid) = rest.split_once(':').unwrap_or((rest, ""));
        let target = (host != fleet::LOCAL).then_some(host);
        let reply = booter::unsubscribe(target, uuid, PROBE_TIMEOUT);
        // ⛔ A MONITOR'S REFUSAL IS THE ANSWER, NOT AN ERROR. The booter declines
        //    to unsubscribe a watch without `--force`, deliberately; showing that
        //    sentence is the whole point of the refusal existing.
        Some(summarise(&reply, &format!("{}: no longer watched", &uuid[..8.min(uuid.len())])))
    } else {
        None
    };

    let mut pane = state.lock().unwrap();
    match action.as_str() {
        "tab" => {
            let tab = posted_value(&values, "tab");
            if tab == schema::TAB_BOOTER || tab == schema::TAB_TOPOLOGY {
                pane.view.tab = tab;
                pane.view.notice = None;
            }
        }
        "filter" => pane.view.filter = posted_value(&values, "filter"),
        "refresh" => pane.view.notice = None,
        _ => {}
    }
    if let Some(text) = outcome {
        pane.view.notice = Some(text);
    }
    pane.touch();
    // Answer with the schema of the pane that POSTED, so a click repaints
    // without waiting for the next stamp poll.
    // ⚠ The reply is the RAIL's shape: it is the pane whose widgets are
    //   pressable, so it is the pane a POST comes from. A viewport press lands
    //   in the top bar, whose chrome is identical in both.
    schema_for(&pane, false)
}

/// Say what actually happened, in the verb's own words when it had any.
fn summarise(reply: &Value, on_success: &str) -> String {
    if let Some(message) = reply.get("message").and_then(Value::as_str) {
        if !message.trim().is_empty() {
            // The booter's verbs narrate themselves; the last line is the verdict.
            let last = message.lines().last().unwrap_or(message).trim();
            if !last.is_empty() {
                return last.to_string();
            }
        }
    }
    if let Some(error) = reply.get("error").and_then(Value::as_str) {
        return format!("⛔ {error}");
    }
    on_success.to_string()
}

fn respond(mut stream: TcpStream, status: u16, body: &Value) {
    let payload = body.to_string();
    let head = format!(
        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n",
        payload.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(payload.as_bytes());
    let _ = stream.flush();
}

/// One reading, printed. The app is a useful CLI outside yggterm too, and that
/// is also how its data is checked without a GUI in the loop.
pub fn print_once(as_json: bool) -> Result<()> {
    let hosts = fleet::roster();
    let readings = fleet::read_all(&hosts, PROBE_TIMEOUT);
    let machines = fleet::group(readings);
    if as_json {
        // ⭐ THE GROUPING IS PART OF THE OUTPUT, NOT A HIDDEN STEP. Which hosts
        //    the app believes share a physical machine — and the derived key it
        //    believes it from — is the one claim here that cannot be checked by
        //    looking at a single reading. Printing it is what makes it
        //    falsifiable from outside.
        println!("{}", json!({
            "hosts": hosts,
            "machines": machines.iter().map(|m| json!({
                "key": m.key,
                "reachable": m.reachable(),
                "hosts": m.readings.iter()
                    .map(|r| r["host"].clone()).collect::<Vec<_>>(),
                "readings": m.readings,
            })).collect::<Vec<_>>(),
        }));
        return Ok(());
    }
    for machine in machines {
        let principal = machine.principal();
        if !machine.reachable() {
            println!(
                "⚠ {} — could not be read: {}",
                principal["label"].as_str().or(principal["host"].as_str()).unwrap_or("?"),
                principal["error"].as_str().unwrap_or("no reason given")
            );
            continue;
        }
        println!(
            "\n{} · {} × {} · kernel {}",
            principal["hostname"].as_str().unwrap_or("?"),
            principal["cpu_count"].as_i64().unwrap_or(0),
            principal["cpu_model"].as_str().unwrap_or("?"),
            principal["kernel"].as_str().unwrap_or("?"),
        );
        for reading in &machine.readings {
            println!(
                "  ├─ {:<12} {:>6.1}% cpu  {:>5} procs  ({})",
                reading["label"].as_str().or(reading["host"].as_str()).unwrap_or("?"),
                reading["cpu_busy_pct"].as_f64().unwrap_or(0.0),
                reading["procs_total"].as_i64().unwrap_or(0),
                reading["virt"].as_str().unwrap_or("?"),
            );
        }
        for container in principal["containers"].as_array().cloned().unwrap_or_default() {
            println!(
                "  ├─ 📦 {:<12} {}",
                container["name"].as_str().unwrap_or("?"),
                container["state"].as_str().unwrap_or("?"),
            );
        }
        for p in principal["top"].as_array().cloned().unwrap_or_default().iter().take(5) {
            println!(
                "     {:>6.1}%  {:>8} KB  {}",
                p["cpu_pct"].as_f64().unwrap_or(0.0),
                p["rss_kb"].as_i64().unwrap_or(0),
                p["comm"].as_str().unwrap_or("?"),
            );
        }
    }
    Ok(())
}

/// Where a reading came from, for the `--probe` self-check.
pub fn probe_once(host: Option<&str>) -> Result<()> {
    println!("{}", probe::read_host(host, PROBE_TIMEOUT));
    Ok(())
}
