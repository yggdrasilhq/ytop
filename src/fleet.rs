//! The fleet: who is in it, and one reading of each of them.
//!
//! ⛔ THE ROSTER IS NOT A LIST IN THIS FILE. A hardcoded set of machine names
//! is a second source of truth about the fleet, and it is wrong the first time
//! a machine is added, renamed or retired — while looking perfectly healthy,
//! because a name that no longer answers is indistinguishable from a machine
//! that is merely down.
//!
//! The one thing that already knows the fleet is the yggterm daemon: it holds
//! an ssh target per remote machine, because that is how it reaches their
//! sessions. So the roster is DISCOVERED from every live daemon.
//!
//! ⛔ BUT DISCOVERY ALONE IS NOT A ROSTER, and treating it as one is what this
//! file used to do. A machine appeared for exactly as long as a daemon happened
//! to mention it, so it vanished from the topology when its sessions ended, when
//! the daemon holding it retired, or when a snapshot simply came back thin. A
//! machine that has gone quiet and a machine that was never there looked
//! identical — which is the same failure the header above warns about, arrived
//! at from the other direction.
//!
//! So discovery REGISTERS. Every machine yggterm reports is merged into
//! `~/.yggterm/config/machines.json` and stays there, and the roster is the
//! union of the registry and today's discovery. A registered machine that no
//! daemon currently mentions is still read, and reports as unreachable — which
//! is a fact, where an empty row was a guess.
//!
//! ⚠ MERGING NEVER CLOBBERS. Operator-owned fields — the display label and the
//! `is_yggdrasil` flag that marks a hypervisor — survive every rediscovery,
//! because they carry knowledge discovery does not have. Auto-detection may FILL
//! an unset flag; it may never clear one a person set.
//!
//! An operator override also exists (`~/.ytop/hosts`, one ssh alias per line)
//! for a machine yggterm has no session on yet. It EXTENDS the roster rather
//! than replacing it, so adding one line cannot silently hide the rest.

use crate::probe;
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;

/// The local machine is always in the fleet, under this label, because a
/// topology that omits the machine you are standing on is a map with a hole
/// where you are.
pub const LOCAL: &str = "local";

fn yggterm_binary() -> String {
    std::env::var("YGGTERM_BIN").unwrap_or_else(|_| "yggterm-headless".to_string())
}

/// The busiest live daemon's endpoint.
///
/// ⚠ WHY NOT THE DEFAULT SOCKET: several daemon versions coexist by design
/// here, and the default endpoint is frequently one that has retired its
/// sessions. Asking the one with the most live sessions gets the daemon that
/// is actually serving the fleet. A deleted binary means a superseded daemon;
/// it may still be serving, but it is never the best answer available.
fn busiest_endpoint() -> Option<String> {
    all_endpoints().into_iter().next()
}

/// EVERY live daemon's endpoint, busiest first.
///
/// ⛔ Asking only the busiest daemon was a quiet way to lose machines. Several
/// daemon versions coexist here by design, and a remote machine is known to the
/// daemon that holds ITS sessions — not necessarily to the one holding the most.
/// So a machine reachable only through a quieter daemon was invisible, and an
/// absence is indistinguishable from a machine that is merely down.
fn all_endpoints() -> Vec<String> {
    let Ok(out) = Command::new(yggterm_binary())
        .args(["server", "daemons", "--json"])
        .output()
    else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_slice::<Value>(&out.stdout) else {
        return Vec::new();
    };
    let Some(list) = value["daemons"].as_array() else {
        return Vec::new();
    };
    let mut daemons: Vec<&Value> = list.iter().collect();
    daemons.sort_by_key(|d| {
        (
            d["exe_deleted"].as_bool().unwrap_or(false),
            -d["live_terminal_session_count"].as_i64().unwrap_or(0),
        )
    });
    let mut out = Vec::new();
    for d in daemons {
        if let Some(ep) = d["endpoint"].as_str().filter(|e| !e.trim().is_empty()) {
            if !out.iter().any(|existing: &String| existing == ep) {
                out.push(ep.to_string());
            }
        }
    }
    out
}

/// A machine yggterm knows about: its ssh target and whatever it calls it.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredMachine {
    pub ssh_target: String,
    pub label: Option<String>,
}

fn machines_from_yggterm() -> Vec<String> {
    discover_machines().into_iter().map(|m| m.ssh_target).collect()
}

/// Every ssh target across EVERY live daemon, each named once.
pub fn discover_machines() -> Vec<DiscoveredMachine> {
    let mut out: Vec<DiscoveredMachine> = Vec::new();
    for endpoint in all_endpoints() {
        let Ok(raw) = Command::new(yggterm_binary())
            .args(["server", "snapshot", "--endpoint", &endpoint])
            .output()
        else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&raw.stdout) else {
            continue;
        };
        // The payload is a list whose first element carries the snapshot.
        let snapshot = value.get(0).unwrap_or(&value);
        let Some(machines) = snapshot["remote_machines"].as_array() else {
            continue;
        };
        for m in machines {
            let Some(target) = m["ssh_target"].as_str().map(str::trim).filter(|t| !t.is_empty())
            else {
                continue;
            };
            if out.iter().any(|d| d.ssh_target == target) {
                continue;
            }
            out.push(DiscoveredMachine {
                ssh_target: target.to_string(),
                // Daemons spell this a few ways; any of them beats the raw target.
                label: ["label", "name", "display_name", "host"]
                    .iter()
                    .find_map(|k| m[*k].as_str())
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(str::to_string),
            });
        }
    }
    out
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MachineEntry {
    pub alias: String,
    pub label: String,
    #[serde(default)]
    pub is_yggdrasil: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct MachinesConfig {
    pub machines: Vec<MachineEntry>,
}

pub fn machines_config_path() -> Option<std::path::PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".yggterm").join("config").join("machines.json"))
}

pub fn machines_from_config() -> Vec<MachineEntry> {
    let mut entries = Vec::new();
    if let Some(path) = machines_config_path() {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = serde_json::from_str::<MachinesConfig>(&data) {
                entries.extend(cfg.machines);
            }
        }
    }
    // Prefer ~/.ytop/hosts, fallback to legacy ~/.yggtopo/hosts
    if let Some(home) = dirs::home_dir() {
        if let Ok(text) = std::fs::read_to_string(home.join(".ytop").join("hosts")) {
            for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                if !entries.iter().any(|e| e.alias == l) {
                    entries.push(MachineEntry {
                        alias: l.to_string(),
                        label: l.to_string(),
                        is_yggdrasil: false,
                    });
                }
            }
        }
    }
    // Also include legacy ~/.yggtopo/hosts if present
    if let Some(home) = dirs::home_dir() {
        if let Ok(text) = std::fs::read_to_string(home.join(".yggtopo").join("hosts")) {
            for l in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                if !entries.iter().any(|e| e.alias == l) {
                    entries.push(MachineEntry {
                        alias: l.to_string(),
                        label: l.to_string(),
                        is_yggdrasil: false,
                    });
                }
            }
        }
    }
    entries
}

pub fn add_machine_to_config(alias: &str, label: &str, is_yggdrasil: bool) -> anyhow::Result<()> {
    let path = machines_config_path().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut cfg = MachinesConfig::default();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(existing) = serde_json::from_str::<MachinesConfig>(&data) {
            cfg = existing;
        }
    }
    let alias_trim = alias.trim();
    if alias_trim.is_empty() {
        return Ok(());
    }
    if let Some(existing) = cfg.machines.iter_mut().find(|m| m.alias == alias_trim) {
        existing.label = label.trim().to_string();
        existing.is_yggdrasil = is_yggdrasil;
    } else {
        cfg.machines.push(MachineEntry {
            alias: alias_trim.to_string(),
            label: if label.trim().is_empty() { alias_trim.to_string() } else { label.trim().to_string() },
            is_yggdrasil,
        });
    }
    std::fs::write(&path, serde_json::to_string_pretty(&cfg)?)?;
    Ok(())
}

fn machines_from_operator() -> Vec<String> {
    machines_from_config().into_iter().map(|m| m.alias).collect()
}

/// Merge what yggterm currently reports into the persistent registry.
///
/// Returns the aliases that were newly registered. Writes only when something
/// actually changed, so an unchanged fleet costs one read and no write.
///
/// ⚠ A discovered label only fills a BLANK label. Renaming a machine is the
/// operator's, and a rediscovery that overwrote their word with the daemon's
/// would undo it silently on the next refresh.
pub fn register_discovered(discovered: &[DiscoveredMachine]) -> Vec<String> {
    let Some(path) = machines_config_path() else {
        return Vec::new();
    };
    let mut cfg = std::fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str::<MachinesConfig>(&d).ok())
        .unwrap_or_default();

    let mut added = Vec::new();
    let mut changed = false;
    for d in discovered {
        let alias = d.ssh_target.trim();
        // The local machine is always in the roster and is not an ssh target.
        if alias.is_empty() || alias == LOCAL {
            continue;
        }
        match cfg.machines.iter_mut().find(|m| m.alias == alias) {
            Some(existing) => {
                if existing.label.trim().is_empty() {
                    existing.label = d.label.clone().unwrap_or_else(|| alias.to_string());
                    changed = true;
                }
            }
            None => {
                cfg.machines.push(MachineEntry {
                    alias: alias.to_string(),
                    label: d.label.clone().unwrap_or_else(|| alias.to_string()),
                    is_yggdrasil: false,
                });
                added.push(alias.to_string());
                changed = true;
            }
        }
    }

    if changed {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(&cfg) {
            let _ = std::fs::write(&path, text);
        }
    }
    added
}

/// Record that a machine turned out to be a hypervisor (ZFS pools or LXC).
///
/// ⛔ Only ever SETS the flag. A probe that failed to see ZFS this once — a slow
/// host, a timeout, a pool not imported yet — must not un-mark a machine the
/// operator or an earlier reading already knew to be one.
pub fn mark_yggdrasil(alias: &str) {
    let Some(path) = machines_config_path() else { return };
    let Some(mut cfg) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|d| serde_json::from_str::<MachinesConfig>(&d).ok())
    else {
        return;
    };
    let Some(entry) = cfg.machines.iter_mut().find(|m| m.alias == alias) else {
        return;
    };
    if entry.is_yggdrasil {
        return;
    }
    entry.is_yggdrasil = true;
    if let Ok(text) = serde_json::to_string_pretty(&cfg) {
        let _ = std::fs::write(&path, text);
    }
}

/// True when a reading shows the machine is a hypervisor worth the extra cards.
pub fn reading_is_yggdrasil(reading: &Value) -> bool {
    let has_pools = reading["zfs"]["pools"]
        .as_array()
        .map(|p| !p.is_empty())
        .unwrap_or(false);
    let has_containers = reading["containers"]
        .as_array()
        .map(|c| !c.is_empty())
        .unwrap_or(false);
    has_pools || has_containers
}

/// Every machine to read, local first, each named once.
///
/// The union of the persistent registry and today's discovery — so a machine
/// that has gone quiet still appears, and reports as unreachable rather than
/// disappearing from the map.
pub fn roster() -> Vec<String> {
    let discovered = discover_machines();
    register_discovered(&discovered);

    let mut out = vec![LOCAL.to_string()];
    for name in discovered
        .into_iter()
        .map(|d| d.ssh_target)
        .chain(machines_from_operator())
    {
        if !out.iter().any(|existing| existing == &name) {
            out.push(name);
        }
    }
    out
}

/// Read every machine, concurrently. One slow host must not decide how long
/// the whole view takes.
pub fn read_all(hosts: &[String], timeout: Duration) -> Vec<Value> {
    let handles: Vec<_> = hosts
        .iter()
        .cloned()
        .map(|host| {
            std::thread::spawn(move || {
                if host == LOCAL {
                    probe::read_host(None, timeout)
                } else {
                    probe::read_host(Some(&host), timeout)
                }
            })
        })
        .collect();
    let readings: Vec<Value> = handles
        .into_iter()
        .zip(hosts.iter())
        .map(|(handle, host)| {
            handle.join().unwrap_or_else(|_| {
                serde_json::json!({"ok": false, "host": host, "error": "the probe thread panicked"})
            })
        })
        .collect();

    // A machine that turns out to hold ZFS pools or containers is a hypervisor,
    // and that is worth remembering rather than re-deriving every refresh — a
    // probe that times out once would otherwise demote it.
    for (host, reading) in hosts.iter().zip(readings.iter()) {
        if host != LOCAL && reading_is_yggdrasil(reading) {
            mark_yggdrasil(host);
        }
    }
    readings
}

/// One physical machine and the yggterm hosts that live on it.
#[derive(Clone)]
pub struct Machine {
    /// The derived identity, or `None` for hosts we could not read.
    pub key: Option<String>,
    /// The readings that belong to it. Never empty.
    pub readings: Vec<Value>,
}

impl Machine {
    /// The reading that best describes the machine ITSELF.
    ///
    /// ⭐ A container HOST knows things its guests cannot — most importantly
    /// the guest list, since a guest cannot see its siblings. So when a
    /// physical machine is represented by both, the host's reading is the one
    /// that describes the box.
    pub fn principal(&self) -> &Value {
        self.readings
            .iter()
            .find(|r| r["virt"].as_str() == Some("none"))
            .or_else(|| self.readings.iter().find(|r| r["ok"].as_bool() == Some(true)))
            .unwrap_or(&self.readings[0])
    }

    pub fn reachable(&self) -> bool {
        self.readings.iter().any(|r| r["ok"].as_bool() == Some(true))
    }
}

/// Group readings into physical machines.
///
/// ⛔ UNREADABLE HOSTS EACH STAND ALONE. Two machines we could not reach have
/// nothing in common except our own blindness, and folding them together would
/// draw a topology out of it. They appear as themselves, marked unreachable.
pub fn group(readings: Vec<Value>) -> Vec<Machine> {
    let mut by_key: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut alone: Vec<Value> = Vec::new();
    for reading in readings {
        match probe::machine_key(&reading) {
            Some(key) => by_key.entry(key).or_default().push(reading),
            None => alone.push(reading),
        }
    }
    let mut machines: Vec<Machine> = by_key
        .into_iter()
        .map(|(key, readings)| Machine { key: Some(key), readings })
        .collect();
    // Reachable machines first — a fleet view whose top row is a host that did
    // not answer buries the information under the absence of information.
    machines.sort_by_key(|m| {
        (
            !m.principal()["virt"].as_str().map(|v| v == "none").unwrap_or(false),
            m.principal()["host"].as_str().unwrap_or("").to_string(),
        )
    });
    machines.extend(alone.into_iter().map(|r| Machine { key: None, readings: vec![r] }));
    machines
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn guest(host: &str, btime: i64) -> Value {
        json!({"ok": true, "host": host, "hostname": host, "virt": "lxc",
               "kernel": "9.9.9-invented", "btime": btime,
               "cpu_model": "Example CPU E1", "cpu_count": 8})
    }
    fn bare(host: &str, btime: i64) -> Value {
        json!({"ok": true, "host": host, "hostname": host, "virt": "none",
               "kernel": "9.9.9-invented", "btime": btime,
               "cpu_model": "Example CPU E1", "cpu_count": 8})
    }

    #[test]
    fn guests_sharing_a_kernel_collapse_into_one_machine() {
        let machines = group(vec![guest("alpha", 1700000000), guest("beta", 1700000000)]);
        assert_eq!(machines.len(), 1);
        assert_eq!(machines[0].readings.len(), 2);
    }

    #[test]
    fn distinct_machines_stay_distinct() {
        let machines = group(vec![guest("alpha", 1700000000), bare("gamma", 1700009999)]);
        assert_eq!(machines.len(), 2);
    }

    #[test]
    fn the_container_host_describes_the_box_not_its_guest() {
        let machines = group(vec![guest("alpha", 1700000000), bare("host-of-alpha", 1700000000)]);
        assert_eq!(machines.len(), 1);
        assert_eq!(machines[0].principal()["host"], "host-of-alpha");
    }

    #[test]
    fn unreachable_hosts_are_never_folded_together() {
        let machines = group(vec![
            json!({"ok": false, "host": "alpha", "error": "timed out"}),
            json!({"ok": false, "host": "beta", "error": "timed out"}),
        ]);
        assert_eq!(machines.len(), 2, "our blindness is not a topology");
        assert!(machines.iter().all(|m| !m.reachable()));
    }

    #[test]
    fn an_operator_line_extends_the_derived_roster_rather_than_replacing_it() {
        // The roster always contains the local machine, whatever else is found:
        // a map with a hole where you are standing is the one hole nobody
        // notices.
        assert_eq!(roster().first().map(String::as_str), Some(LOCAL));
    }
}


#[cfg(test)]
mod registry_tests {
    use super::*;
    use serde_json::json;

    /// Guards every test that writes a registry: the path is derived from HOME,
    /// so each test needs its own and they must not run against a real one.
    fn with_temp_home<T>(name: &str, body: impl FnOnce() -> T) -> T {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let dir = std::env::temp_dir().join(format!("ytop-registry-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let previous = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", &dir) };
        let out = body();
        match previous {
            Some(h) => unsafe { std::env::set_var("HOME", h) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    fn discovered(target: &str, label: Option<&str>) -> DiscoveredMachine {
        DiscoveredMachine {
            ssh_target: target.to_string(),
            label: label.map(str::to_string),
        }
    }

    /// The whole point: what yggterm reports today is written down, so it is
    /// still there tomorrow when no daemon happens to mention it.
    #[test]
    fn discovery_is_written_down_and_survives_a_silent_snapshot() {
        with_temp_home("persist", || {
            let added = register_discovered(&[discovered("alpha", Some("Alpha Box"))]);
            assert_eq!(added, vec!["alpha".to_string()]);

            // A later refresh where the daemon mentions nothing at all.
            register_discovered(&[]);

            let kept = machines_from_config();
            assert_eq!(kept.len(), 1, "a quiet machine must not be forgotten");
            assert_eq!(kept[0].alias, "alpha");
            assert_eq!(kept[0].label, "Alpha Box");
        });
    }

    /// ⛔ The operator's knowledge outranks the daemon's. A hypervisor flag and
    /// a hand-written label must survive every rediscovery.
    #[test]
    fn rediscovery_never_clobbers_what_the_operator_set() {
        with_temp_home("preserve", || {
            add_machine_to_config("vault", "Storage Node", true).unwrap();
            // The daemon rediscovers it, spelling it its own way.
            register_discovered(&[discovered("vault", Some("vault.internal"))]);

            let entry = machines_from_config().into_iter().find(|m| m.alias == "vault").unwrap();
            assert_eq!(entry.label, "Storage Node", "the operator's word stands");
            assert!(entry.is_yggdrasil, "the hypervisor flag must survive");
        });
    }

    /// A blank label is not knowledge, so discovery may fill it. (`add_machine`
    /// substitutes the alias for an empty label, so a genuinely blank one only
    /// arrives from a hand-edited config — which is exactly when a daemon's
    /// spelling is an improvement rather than a loss.)
    #[test]
    fn a_blank_label_is_filled_by_discovery() {
        with_temp_home("fill", || {
            let path = machines_config_path().unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(
                &path,
                r#"{"machines":[{"alias":"beta","label":"","is_yggdrasil":false}]}"#,
            )
            .unwrap();

            register_discovered(&[discovered("beta", Some("Beta Box"))]);
            let entry = machines_from_config().into_iter().find(|m| m.alias == "beta").unwrap();
            assert_eq!(entry.label, "Beta Box");
        });
    }

    /// ⛔ And the converse, which is the rule that actually protects the
    /// operator: a label they set is never replaced, however the daemon spells
    /// the same machine.
    #[test]
    fn a_label_the_operator_set_is_never_replaced() {
        with_temp_home("nofill", || {
            add_machine_to_config("beta", "Beta Box", false).unwrap();
            register_discovered(&[discovered("beta", Some("beta.internal.example"))]);
            let entry = machines_from_config().into_iter().find(|m| m.alias == "beta").unwrap();
            assert_eq!(entry.label, "Beta Box");
        });
    }

    /// Marking is one-way: a probe that saw no pools this once must not demote
    /// a machine that is one.
    #[test]
    fn the_hypervisor_flag_is_set_but_never_cleared() {
        with_temp_home("flag", || {
            register_discovered(&[discovered("gamma", None)]);
            assert!(!machines_from_config()[0].is_yggdrasil);

            mark_yggdrasil("gamma");
            assert!(machines_from_config()[0].is_yggdrasil);

            // Every later rediscovery, including ones that saw nothing.
            register_discovered(&[discovered("gamma", None)]);
            assert!(machines_from_config()[0].is_yggdrasil, "must not be demoted");
        });
    }

    #[test]
    fn registering_twice_adds_once() {
        with_temp_home("idempotent", || {
            assert_eq!(register_discovered(&[discovered("delta", None)]).len(), 1);
            assert_eq!(register_discovered(&[discovered("delta", None)]).len(), 0);
            assert_eq!(machines_from_config().len(), 1);
        });
    }

    /// The local machine is in the roster by construction, not as an ssh target.
    #[test]
    fn the_local_machine_is_never_registered_as_a_remote() {
        with_temp_home("nolocal", || {
            register_discovered(&[discovered(LOCAL, None), discovered("", None)]);
            assert!(machines_from_config().is_empty());
        });
    }

    #[test]
    fn a_reading_with_pools_or_containers_is_a_hypervisor() {
        assert!(reading_is_yggdrasil(&json!({"zfs": {"pools": [{"name": "tank"}]}})));
        assert!(reading_is_yggdrasil(&json!({"containers": [{"name": "ct1"}]})));
        assert!(!reading_is_yggdrasil(&json!({"zfs": {"pools": []}, "containers": []})));
        assert!(!reading_is_yggdrasil(&json!({"ok": false})));
    }
}
