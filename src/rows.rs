//! Agent Fleet Rows — birds-eye view of all N.x rows, their liveness, twins, transcripts, and supervision.
//!
//! ⛔ RE-IMPLEMENTS NO RULES. Reads state from ~/.yggterm/relay and actual /proc trees across
//! the fleet to present an honest, comprehensive dashboard of all agent rows.

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
    pub twin_alert: bool,
    pub leaked_child_loops: usize,
    pub transcript_size_kb: i64,
    pub transcript_lines: usize,
    pub last_active_mtime: String,
    pub supervision_state: String,
    pub is_alive: bool,
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

# Quota hold check
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

# Never-arm check
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

# ps snapshot
ps_out = ""
try:
    ps_out = subprocess.run(["ps", "-eo", "pid,ppid,etimes,pcpu,pmem,args"], capture_output=True, text=True, timeout=10).stdout
except Exception:
    pass

rows_out = []

# If no seat-membership.json, fallback to scanning booter/monitor/claude
scanned_uuids = set()
for seat, info in seats.items():
    uuid = info.get("for_uuid") or ""
    scanned_uuids.add(uuid)

# Also check running agent processes that might not be in seat-membership
for line in ps_out.splitlines():
    m = re.search(r"--(session-id|resume)\s+([0-9a-f-]{36})", line)
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

for seat, info in seats.items():
    uuid = info.get("for_uuid") or ""
    role = info.get("role") or "relay"
    camp = info.get("campaign") or "unassigned"
    host = info.get("host") or ""

    matching_pids = []
    has_resume = False
    has_session_id = False
    child_loops = 0

    if uuid:
        for line in ps_out.splitlines():
            if uuid in line and any(k in line for k in ["claude", "codex", "agy"]):
                try:
                    p = int(line.split()[0])
                    matching_pids.append(p)
                    if "--resume" in line:
                        has_resume = True
                    if "--session-id" in line:
                        has_session_id = True
                except Exception:
                    pass

        # Check for child looping bash subshells under matched pids
        for p in matching_pids:
            for line in ps_out.splitlines():
                parts = line.split()
                if len(parts) >= 6 and parts[1] == str(p) and "sleep" in line and "until" in line:
                    child_loops += 1

    twin_alert = len(matching_pids) > 1 and has_resume and has_session_id

    # Check transcript
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

    # Check supervision
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
        # read first line of transcript if available
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

    // Sort rows by seat numbers
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
        campaigns,
    })
}

pub fn scan_all_hosts() -> FleetRowsReport {
    // Probes dev (or local fallback)
    let dev_report = probe_rows(Some("dev"), Duration::from_secs(12));
    if let Some(r) = dev_report {
        return r;
    }
    probe_rows(None, Duration::from_secs(5)).unwrap_or_default()
}
