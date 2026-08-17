//! Agent Fleet Rows & Resource Jankbox Diagnostics.
//!
//! Real-time per-row resource aggregation (CPU % & RSS RAM), context budget tracking,
//! and jankbox leak detection across all N.x rows.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowInfo {
    pub seat: String,
    pub role: String,
    pub campaign: String,
    pub uuid: String,
    pub title: String,
    pub host: String,
    pub pids: Vec<i32>,
    pub cpu_pct: f64,
    pub rss_kb: i64,
    pub twin_alert: bool,
    pub leaked_child_loops: usize,
    pub transcript_size_kb: i64,
    pub transcript_lines: usize,
    pub last_active_mtime: String,
    pub supervision_state: String,
    pub is_alive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JankboxDiagnosis {
    pub leaked_subshell_pids: Vec<i32>,
    pub twin_stale_pids: Vec<i32>,
    pub bloated_transcripts_mb: Vec<(String, f64)>,
    pub total_jank_procs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FleetRowsReport {
    pub quota_hold: Option<String>,
    pub rows: Vec<RowInfo>,
    pub total_rows: usize,
    pub live_count: usize,
    pub twin_count: usize,
    pub leak_count: usize,
    pub total_transcript_mb: f64,
    pub total_agent_cpu_pct: f64,
    pub total_agent_rss_mb: f64,
    pub jankbox: JankboxDiagnosis,
    pub campaigns: BTreeMap<String, Vec<RowInfo>>,
}

const PROBE_SCRIPT: &str = r#"
import json, os, glob, subprocess, time, re

home = os.path.expanduser("~")
relay = os.path.join(home, ".yggterm", "relay")
seats_file = os.path.join(relay, "seat-membership.json")

seats = {}
if os.path.exists(seats_file):
    try:
        with open(seats_file) as f:
            seats = json.load(f)
    except Exception:
        pass

quota_hold = None
rl_file = os.path.join(relay, "booter.rate-limit-hold")
if os.path.exists(rl_file):
    try:
        with open(rl_file) as f:
            rl = json.load(f)
            if rl.get("indefinite"):
                quota_hold = "INDEFINITE (" + rl.get("reason", "") + ")"
            elif time.time() < (rl.get("until") or 0):
                left_m = int(((rl.get("until") or 0) - time.time()) // 60)
                quota_hold = f"{left_m}m left ({rl.get('reason', '')})"
    except Exception:
        pass

never_armed = set()
na_file = os.path.join(relay, "never-arm.tsv")
if os.path.exists(na_file):
    try:
        with open(na_file) as f:
            for l in f:
                parts = l.strip().split("\t")
                if parts:
                    never_armed.add(parts[0][:8])
    except Exception:
        pass

ps_out = ""
try:
    ps_out = subprocess.run(["ps", "-eo", "pid,ppid,pcpu,rss,args"], capture_output=True, text=True, timeout=10).stdout
except Exception:
    pass

procs_by_ppid = {}
all_procs = {}
for line in ps_out.splitlines()[1:]:
    parts = line.split(None, 4)
    if len(parts) >= 5:
        pid, ppid, pcpu, rss, args = parts
        try:
            pid, ppid, pcpu, rss = int(pid), int(ppid), float(pcpu), int(rss)
            all_procs[pid] = {"ppid": ppid, "pcpu": pcpu, "rss": rss, "args": args}
            procs_by_ppid.setdefault(ppid, []).append(pid)
        except Exception:
            pass

def get_descendants(pid):
    res = [pid]
    for child in procs_by_ppid.get(pid, []):
        res.extend(get_descendants(child))
    return res

scanned_uuids = set()
for seat, info in seats.items():
    uuid = info.get("for_uuid") or ""
    if uuid:
        scanned_uuids.add(uuid)

for pid, pdata in all_procs.items():
    m = re.search(r"--(session-id|resume)\s+([0-9a-f-]{36})", pdata["args"])
    if m:
        u = m.group(2)
        if u not in scanned_uuids:
            scanned_uuids.add(u)
            seats[f"? ({u[:8]})"] = {
                "for_uuid": u,
                "role": "agent",
                "campaign": "unregistered",
                "host": "",
            }

rows_out = []
leaked_subshell_pids = []
twin_stale_pids = []
bloated_transcripts = []

for seat, info in seats.items():
    uuid = info.get("for_uuid") or ""
    role = info.get("role") or "relay"
    camp = info.get("campaign") or "unassigned"
    host = info.get("host") or ""

    matching_pids = []
    resume_pids = []
    session_id_pids = []
    child_loops = 0

    if uuid:
        for pid, pdata in all_procs.items():
            args = pdata["args"]
            if uuid in args and any(k in args for k in ["claude", "codex", "agy"]):
                matching_pids.append(pid)
                if "--resume" in args:
                    resume_pids.append(pid)
                if "--session-id" in args:
                    session_id_pids.append(pid)

        # Check child loop leaks under matched pids
        for p in matching_pids:
            desc = get_descendants(p)
            for d in desc:
                if d != p and d in all_procs:
                    d_args = all_procs[d]["args"]
                    if "sleep" in d_args and "until" in d_args:
                        child_loops += 1
                        leaked_subshell_pids.append(d)

    twin_alert = len(resume_pids) > 0 and len(session_id_pids) > 0
    if twin_alert:
        twin_stale_pids.extend(session_id_pids)

    # Sum total CPU and RSS across all descendant processes of the agent
    total_cpu = 0.0
    total_rss = 0
    all_desc_pids = set()
    for p in matching_pids:
        all_desc_pids.update(get_descendants(p))
    for dp in all_desc_pids:
        if dp in all_procs:
            total_cpu += all_procs[dp]["pcpu"]
            total_rss += all_procs[dp]["rss"]

    transcripts = glob.glob(f"{home}/.claude/projects/*/{uuid}.jsonl") if uuid else []
    t_size = 0
    t_lines = 0
    t_mtime = 0
    if transcripts:
        t_path = transcripts[0]
        try:
            t_size = os.path.getsize(t_path)
            t_mtime = os.path.getmtime(t_path)
            with open(t_path, "r", errors="ignore") as tf:
                for _ in tf:
                    t_lines += 1
        except Exception:
            pass

    if t_size > 10 * 1024 * 1024:
        bloated_transcripts.append((seat, round(t_size / 1024 / 1024, 1)))

    booter_sub = os.path.exists(os.path.join(relay, "booter", f"{uuid}.json")) if uuid else False
    monitor_sub = os.path.exists(os.path.join(relay, "monitor", f"{uuid}.json")) if uuid else False

    sup_state = "Unsupervised"
    if quota_hold:
        sup_state = "⏸ Quota Held"
    elif uuid[:8] in never_armed:
        sup_state = "🛡️ Never-Arm"
    elif booter_sub and monitor_sub:
        sup_state = "⚡ Armed (Both)"
    elif booter_sub:
        sup_state = "⚡ Armed (Booter)"
    elif monitor_sub:
        sup_state = "⚡ Armed (Monitor)"

    title = info.get("title") or info.get("intent") or ""
    if not title and transcripts:
        try:
            with open(transcripts[0], "r", errors="ignore") as tf:
                for line in tf:
                    obj = json.loads(line)
                    if "session_title" in obj:
                        title = obj["session_title"]
                        break
        except Exception:
            pass

    mtime_str = time.strftime("%Y-%m-%d %H:%M", time.localtime(t_mtime)) if t_mtime else "N/A"

    rows_out.append({
        "seat": seat,
        "role": role,
        "campaign": camp,
        "uuid": uuid,
        "title": title or f"Session {uuid[:8]}",
        "host": host or "local",
        "pids": matching_pids,
        "cpu_pct": round(total_cpu, 1),
        "rss_kb": total_rss,
        "twin_alert": twin_alert,
        "leaked_child_loops": child_loops,
        "transcript_size_kb": int(t_size / 1024),
        "transcript_lines": t_lines,
        "last_active_mtime": mtime_str,
        "supervision_state": sup_state,
        "is_alive": len(matching_pids) > 0,
    })

print(json.dumps({
    "quota_hold": quota_hold,
    "rows": rows_out,
    "jankbox": {
        "leaked_subshell_pids": leaked_subshell_pids,
        "twin_stale_pids": twin_stale_pids,
        "bloated_transcripts_mb": bloated_transcripts,
        "total_jank_procs": len(leaked_subshell_pids) + len(twin_stale_pids),
    }
}))
"#;

pub fn probe_rows(host: Option<&str>, timeout: Duration) -> Option<FleetRowsReport> {
    let output = match host {
        None => Command::new("python3")
            .arg("-c")
            .arg(PROBE_SCRIPT)
            .output()
            .ok()?,
        Some(h) => Command::new("ssh")
            .arg("-o")
            .arg(format!("ConnectTimeout={}", timeout.as_secs().max(1)))
            .arg("-o")
            .arg("BatchMode=yes")
            .arg(h)
            .arg(format!("su - pi -c 'python3 -c \"{}\"'", PROBE_SCRIPT.replace('"', "\\\"").replace('$', "\\$")))
            .output()
            .ok()?,
    };

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let json_start = text.find('{')?;
    let val: Value = serde_json::from_str(&text[json_start..]).ok()?;

    let quota_hold = val["quota_hold"].as_str().map(str::to_string);
    let mut rows: Vec<RowInfo> = serde_json::from_value(val["rows"].clone()).ok()?;

    rows.sort_by(|a, b| {
        let parse_seat = |s: &str| -> Vec<i32> {
            s.split('.').filter_map(|p| p.parse::<i32>().ok()).collect()
        };
        let sa = parse_seat(&a.seat);
        let sb = parse_seat(&b.seat);
        if !sa.is_empty() && !sb.is_empty() {
            sa.cmp(&sb)
        } else {
            a.seat.cmp(&b.seat)
        }
    });

    let total_rows = rows.len();
    let live_count = rows.iter().filter(|r| r.is_alive).count();
    let twin_count = rows.iter().filter(|r| r.twin_alert).count();
    let leak_count = rows.iter().map(|r| r.leaked_child_loops).sum();
    let total_transcript_mb = rows.iter().map(|r| r.transcript_size_kb as f64 / 1024.0).sum::<f64>();
    let total_agent_cpu_pct = rows.iter().map(|r| r.cpu_pct).sum::<f64>();
    let total_agent_rss_mb = rows.iter().map(|r| r.rss_kb as f64 / 1024.0).sum::<f64>();

    let jankbox: JankboxDiagnosis = serde_json::from_value(val["jankbox"].clone()).unwrap_or_default();

    let mut campaigns: BTreeMap<String, Vec<RowInfo>> = BTreeMap::new();
    for r in &rows {
        campaigns.entry(r.campaign.clone()).or_default().push(r.clone());
    }

    Some(FleetRowsReport {
        quota_hold,
        rows,
        total_rows,
        live_count,
        twin_count,
        leak_count,
        total_transcript_mb,
        total_agent_cpu_pct,
        total_agent_rss_mb,
        jankbox,
        campaigns,
    })
}

fn probe_yggterm_fallback() -> Option<FleetRowsReport> {
    // Live probe per diagnostics: server snapshot is the daemon ground truth
    // (live_sessions with terminal_lines), not relay seat-membership.
    // Used when relay is empty (jojo) but daemon holds 50+ Live Sessions.
    fn yggterm_bin() -> String {
        if let Ok(v) = std::env::var("YGGTERM_BIN") {
            if !v.trim().is_empty() { return v; }
        }
        if let Some(home) = dirs::home_dir() {
            let cand = home.join(".local/bin/yggterm-headless");
            if cand.exists() { return cand.to_string_lossy().to_string(); }
            let cand2 = home.join(".local/bin/yggterm");
            if cand2.exists() { return cand2.to_string_lossy().to_string(); }
        }
        "yggterm-headless".to_string()
    }
    let output = Command::new(yggterm_bin())
        .args(["server", "snapshot"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let val: Value = serde_json::from_slice(&output.stdout).ok()?;
    let live = val.get("live_sessions")?.as_array()?;
    if live.is_empty() {
        return None;
    }
    // Also need descendant CPU/RSS via ps like the relay path
    let ps_out = Command::new("ps")
        .args(["-eo", "pid,ppid,pcpu,rss,args"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    let mut all_procs: std::collections::BTreeMap<i32, (f64, i64, String)> = std::collections::BTreeMap::new();
    let mut procs_by_ppid: std::collections::BTreeMap<i32, Vec<i32>> = std::collections::BTreeMap::new();
    for line in ps_out.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            if let (Ok(pid), Ok(ppid), Ok(pcpu), Ok(rss)) = (
                parts[0].parse::<i32>(),
                parts[1].parse::<i32>(),
                parts[2].parse::<f64>(),
                parts[3].parse::<i64>(),
            ) {
                let args = parts[4..].join(" ");
                all_procs.insert(pid, (pcpu, rss, args));
                procs_by_ppid.entry(ppid).or_default().push(pid);
            }
        }
    }
    fn descendants(pid: i32, by_ppid: &std::collections::BTreeMap<i32, Vec<i32>>, out: &mut std::collections::BTreeSet<i32>) {
        out.insert(pid);
        if let Some(children) = by_ppid.get(&pid) {
            for &c in children {
                descendants(c, by_ppid, out);
            }
        }
    }
    let mut rows: Vec<RowInfo> = Vec::new();
    for sess in live {
        let uuid = sess.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if uuid.is_empty() {
            continue;
        }
        let title = sess.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let kind = sess.get("kind").and_then(|v| v.as_str()).unwrap_or("shell").to_string();
        let host = sess.get("host_label").and_then(|v| v.as_str()).unwrap_or("local").to_string();
        // Find pids matching uuid
        let mut matching: Vec<i32> = Vec::new();
        for (pid, (_, _, args)) in &all_procs {
            if args.contains(&uuid) {
                matching.push(*pid);
            }
        }
        let mut all_desc = std::collections::BTreeSet::new();
        for &p in &matching {
            descendants(p, &procs_by_ppid, &mut all_desc);
        }
        let mut total_cpu = 0.0;
        let mut total_rss = 0i64;
        for d in &all_desc {
            if let Some((pcpu, rss, _)) = all_procs.get(d) {
                total_cpu += pcpu;
                total_rss += rss;
            }
        }
        let is_alive = true; // snapshot live_sessions are live by definition
        // Seat: try outline_prefix from title? Fall back to short uuid
        let seat = uuid.chars().take(8).collect::<String>();
        rows.push(RowInfo {
            seat,
            role: kind.clone(),
            campaign: "yggterm".to_string(),
            uuid: uuid.clone(),
            title: if title.is_empty() { format!("Session {}", &uuid[..8.min(uuid.len())]) } else { title },
            host,
            pids: matching,
            cpu_pct: total_cpu,
            rss_kb: total_rss,
            twin_alert: false,
            leaked_child_loops: 0,
            transcript_size_kb: 0,
            transcript_lines: 0,
            last_active_mtime: "N/A".to_string(),
            supervision_state: if is_alive { "Live".to_string() } else { "Dead".to_string() },
            is_alive,
        });
    }
    if rows.is_empty() {
        return None;
    }
    rows.sort_by(|a, b| a.uuid.cmp(&b.uuid));
    let total_rows = rows.len();
    let live_count = rows.iter().filter(|r| r.is_alive).count();
    let total_cpu = rows.iter().map(|r| r.cpu_pct).sum();
    let total_rss = rows.iter().map(|r| r.rss_kb as f64 / 1024.0).sum();
    let mut campaigns: std::collections::BTreeMap<String, Vec<RowInfo>> = std::collections::BTreeMap::new();
    for r in &rows {
        campaigns.entry(r.campaign.clone()).or_default().push(r.clone());
    }
    Some(FleetRowsReport {
        quota_hold: None,
        rows,
        total_rows,
        live_count,
        twin_count: 0,
        leak_count: 0,
        total_transcript_mb: 0.0,
        total_agent_cpu_pct: total_cpu,
        total_agent_rss_mb: total_rss,
        jankbox: JankboxDiagnosis::default(),
        campaigns,
    })
}

pub fn scan_all_hosts() -> FleetRowsReport {
    let dev_report = probe_rows(Some("dev"), Duration::from_secs(12));
    if let Some(r) = dev_report {
        if r.total_rows > 0 {
            return r;
        }
    }
    if let Some(r) = probe_rows(None, Duration::from_secs(5)) {
        if r.total_rows > 0 {
            return r;
        }
    }
    probe_yggterm_fallback().unwrap_or_default()
}

pub fn clean_jankbox_on_dev() -> anyhow::Result<usize> {
    let report = scan_all_hosts();
    let mut pids_to_kill: Vec<i32> = Vec::new();
    pids_to_kill.extend(&report.jankbox.leaked_subshell_pids);
    pids_to_kill.extend(&report.jankbox.twin_stale_pids);
    if pids_to_kill.is_empty() {
        return Ok(0);
    }
    let pid_str = pids_to_kill
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let cmd = format!("su - pi -c 'kill -9 {} 2>/dev/null || true'", pid_str);
    let _ = Command::new("ssh").arg("dev").arg(cmd).output()?;
    Ok(pids_to_kill.len())
}
