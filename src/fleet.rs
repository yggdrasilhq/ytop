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
//! sessions. So the roster is READ from it, and a machine appears here for
//! exactly as long as yggterm can actually reach it.
//!
//! An operator override exists (`~/.ytop/hosts`, one ssh alias per line)
//! for a machine yggterm has no session on yet. It EXTENDS the derived roster
//! rather than replacing it, so adding one line cannot silently hide the rest.

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
    let out = Command::new(yggterm_binary())
        .args(["server", "daemons", "--json"])
        .output()
        .ok()?;
    let value: Value = serde_json::from_slice(&out.stdout).ok()?;
    let mut daemons: Vec<&Value> = value["daemons"].as_array()?.iter().collect();
    daemons.sort_by_key(|d| {
        (
            d["exe_deleted"].as_bool().unwrap_or(false),
            -d["live_terminal_session_count"].as_i64().unwrap_or(0),
        )
    });
    daemons
        .first()
        .and_then(|d| d["endpoint"].as_str())
        .map(str::to_string)
}

/// The ssh targets the local yggterm daemon knows about.
fn machines_from_yggterm() -> Vec<String> {
    let Some(endpoint) = busiest_endpoint() else {
        return Vec::new();
    };
    let Ok(out) = Command::new(yggterm_binary())
        .args(["server", "snapshot", "--endpoint", &endpoint])
        .output()
    else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_slice::<Value>(&out.stdout) else {
        return Vec::new();
    };
    // The payload is a list whose first element carries the snapshot.
    let snapshot = value.get(0).unwrap_or(&value);
    snapshot["remote_machines"]
        .as_array()
        .map(|machines| {
            machines
                .iter()
                .filter_map(|m| m["ssh_target"].as_str())
                .filter(|t| !t.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
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

/// Every machine to read, local first, each named once.
pub fn roster() -> Vec<String> {
    let mut out = vec![LOCAL.to_string()];
    for name in machines_from_yggterm().into_iter().chain(machines_from_operator()) {
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
    handles
        .into_iter()
        .zip(hosts.iter())
        .map(|(handle, host)| {
            handle.join().unwrap_or_else(|_| {
                serde_json::json!({"ok": false, "host": host, "error": "the probe thread panicked"})
            })
        })
        .collect()
}

/// One physical machine and the yggterm hosts that live on it.
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
