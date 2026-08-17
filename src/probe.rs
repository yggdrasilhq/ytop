//! The host probe — one reading of one machine, wherever that machine is.
//!
//! ⛔ ONE PROBE, TWO EXECUTIONS. The same script runs for the local host and
//! for a remote one; only how it is launched differs. Two probes would drift,
//! and the day they disagreed the fleet view would be reporting a difference
//! between its own instruments as a difference between machines.
//!
//! ⛔ AND IT IS FED ON STDIN, NEVER AS ARGV. `ssh host python3 -c <script>`
//! looks equivalent and is not: ssh JOINS its argv into ONE remote shell
//! command string, so a multi-line script with quotes is re-parsed by the
//! remote shell and arrives mangled — which fails as "host unreachable" and
//! reads like a dead machine. (Learned in the fleet's babysitter, paid for
//! once already; the same shape, so the same defence.)

use serde_json::Value;
use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

/// How long the probe waits between its two CPU samples.
///
/// ⛔⛔ THE ONE MEASUREMENT THIS FILE EXISTS TO GET RIGHT. `ps %CPU` is a
/// LIFETIME AVERAGE — total CPU divided by total age — so a process that
/// burned a core for an hour and has been asleep since reads as busy forever,
/// and one that started spinning ten seconds ago reads as idle. An htop-style
/// view built on it is not a live view at all; it is a biography.
///
/// So the probe samples `/proc/<pid>/stat` TWICE and reports the delta over
/// the interval, which is what htop actually does. The interval is a real
/// trade: too short and scheduling noise dominates, too long and every refresh
/// stalls on it. 400 ms is enough to separate a busy process from a sleeping
/// one and short enough to run inside a refresh.
const CPU_SAMPLE_MS: u64 = 400;

/// How many processes a host reports. The pane shows the busiest; the totals
/// in the footer are over ALL of them, so nothing is silently dropped from the
/// counts by being absent from the list.
const TOP_N: usize = 12;

const PROBE: &str = r#"
import json, os, time, subprocess, re

SAMPLE_MS = int(os.environ.get("YTOP_SAMPLE_MS", os.environ.get("YGGTOPO_SAMPLE_MS", "400")))
TOP_N = int(os.environ.get("YTOP_TOP_N", os.environ.get("YGGTOPO_TOP_N", "12")))

def read(path, default=""):
    try:
        with open(path) as fh:
            return fh.read()
    except OSError:
        return default

def meminfo():
    out = {}
    for line in read("/proc/meminfo").splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[0].endswith(":"):
            try:
                out[parts[0][:-1]] = int(parts[1])
            except ValueError:
                pass
    return out

def cpu_model():
    for line in read("/proc/cpuinfo").splitlines():
        if line.startswith("model name"):
            return line.split(":", 1)[1].strip()
    # Not every architecture spells it "model name"; say so rather than lie.
    return "unknown"

def stat_field(name, default=0):
    for line in read("/proc/stat").splitlines():
        if line.startswith(name + " "):
            try:
                return int(line.split()[1])
            except (ValueError, IndexError):
                return default
    return default

def total_jiffies():
    for line in read("/proc/stat").splitlines():
        if line.startswith("cpu "):
            return sum(int(v) for v in line.split()[1:])
    return 0

def pids():
    return [e for e in os.listdir("/proc") if e.isdigit()]

def sample():
    """pid -> (utime+stime jiffies). One pass, cheap, no allocation of names."""
    out = {}
    for pid in pids():
        raw = read("/proc/%s/stat" % pid)
        if not raw:
            continue
        # ⛔ THE COMM FIELD CAN CONTAIN SPACES AND PARENTHESES. Splitting the
        #    whole line on whitespace mis-indexes every field after it, which
        #    silently attributes one process's CPU to another. Cut at the LAST
        #    ')' — the kernel guarantees comm is wrapped in one pair.
        close = raw.rfind(")")
        if close < 0:
            continue
        rest = raw[close + 2:].split()
        if len(rest) < 13:
            continue
        try:
            out[pid] = int(rest[11]) + int(rest[12])   # utime, stime
        except ValueError:
            continue
    return out

def details(pid):
    raw = read("/proc/%s/stat" % pid)
    close = raw.rfind(")")
    open_ = raw.find("(")
    comm = raw[open_ + 1:close] if 0 <= open_ < close else "?"
    cmd = read("/proc/%s/cmdline" % pid).replace("\0", " ").strip()
    rss_kb = 0
    for line in read("/proc/%s/status" % pid).splitlines():
        if line.startswith("VmRSS:"):
            try:
                rss_kb = int(line.split()[1])
            except (ValueError, IndexError):
                pass
            break
    try:
        uid = os.stat("/proc/%s" % pid).st_uid
    except OSError:
        uid = -1
    return comm, (cmd or "[%s]" % comm), rss_kb, uid

def usernames():
    names = {}
    for line in read("/etc/passwd").splitlines():
        parts = line.split(":")
        if len(parts) > 2:
            try:
                names[int(parts[2])] = parts[0]
            except ValueError:
                pass
    return names

def which(*names):
    for d in os.environ.get("PATH", "/usr/sbin:/usr/bin:/sbin:/bin").split(":"):
        for n in names:
            p = os.path.join(d, n)
            if os.path.isfile(p) and os.access(p, os.X_OK):
                return p, n
    return None, None

def virt():
    """What this machine IS. Container guests cannot see their siblings, so
    this decides whether we even try to enumerate containers here."""
    if os.path.exists("/dev/.lxc") or os.path.exists("/proc/vz"):
        return "lxc"
    path, _ = which("systemd-detect-virt")
    if path:
        try:
            r = subprocess.run([path], capture_output=True, text=True, timeout=5)
            v = (r.stdout or "").strip()
            if v and v != "none":
                return v
        except Exception:
            pass
    envs = read("/proc/1/environ")
    if "container=" in envs:
        return envs.split("container=", 1)[1].split("\0")[0] or "container"
    return "none"

def containers():
    """The guests THIS machine hosts, if it hosts any.

    ⚠ An empty list is not 'no containers' — it is 'none that this host can
    enumerate', which is also the honest answer from inside a guest. The caller
    must not render absence as a fact about the machine."""
    path, name = which("lxc-ls", "pct", "incus", "lxc")
    if not path:
        return [], None
    try:
        if name == "lxc-ls":
            r = subprocess.run([path, "-1", "--running"], capture_output=True,
                               text=True, timeout=10)
            found = [{"name": n.strip(), "state": "RUNNING"}
                     for n in (r.stdout or "").splitlines() if n.strip()]
            r2 = subprocess.run([path, "-1"], capture_output=True, text=True, timeout=10)
            running = {c["name"] for c in found}
            for n in (r2.stdout or "").splitlines():
                if n.strip() and n.strip() not in running:
                    found.append({"name": n.strip(), "state": "STOPPED"})
            return found, name
        if name == "pct":
            r = subprocess.run([path, "list"], capture_output=True, text=True, timeout=10)
            found = []
            for line in (r.stdout or "").splitlines()[1:]:
                f = line.split()
                if len(f) >= 3:
                    found.append({"name": f[-1], "state": f[1], "id": f[0]})
            return found, name
        r = subprocess.run([path, "list", "-f", "csv"], capture_output=True,
                           text=True, timeout=10)
        found = []
        for line in (r.stdout or "").splitlines():
            f = line.split(",")
            if len(f) >= 2 and f[0]:
                found.append({"name": f[0], "state": f[1]})
        return found, name
    except Exception:
        return [], name

hz = os.sysconf("SC_CLK_TCK") or 100
before = sample()
t0 = time.time()
time.sleep(SAMPLE_MS / 1000.0)
after = sample()
elapsed = max(time.time() - t0, 0.001)

rows = []
for pid, later in after.items():
    earlier = before.get(pid)
    if earlier is None:
        continue                      # born during the window: no honest delta
    rows.append((pid, (later - earlier) / hz / elapsed * 100.0))
rows.sort(key=lambda r: -r[1])

names = usernames()
top = []
container_procs = {}

for pid, pct in rows:
    comm, cmd, rss_kb, uid = details(pid)
    cgroup = read(f"/proc/{pid}/cgroup")
    cont_name = None
    m = re.search(r"/(?:lxc\.payload\.|lxc/|lxc@|docker/)([a-zA-Z0-9_-]+)", cgroup)
    if m:
        cont_name = m.group(1)
    proc_entry = {
        "pid": int(pid), "comm": comm, "cmd": cmd[:180],
        "cpu_pct": round(pct, 1), "rss_kb": rss_kb,
        "user": names.get(uid, str(uid)),
        "container": cont_name,
    }
    if len(top) < TOP_N:
        top.append(proc_entry)
    if cont_name:
        container_procs.setdefault(cont_name, []).append(proc_entry)

mem = meminfo()
guests, tool = containers()
for g in guests:
    c_name = g.get("name")
    c_p = container_procs.get(c_name, [])
    g["cpu_busy_pct"] = round(sum(p["cpu_pct"] for p in c_p), 1)
    g["mem_rss_kb"] = sum(p["rss_kb"] for p in c_p)
    g["procs_count"] = len(c_p)
    g["top_procs"] = c_p[:6]

# ZFS Check
zfs_info = {"has_zfs": False, "pools": [], "iostat": None, "datasets": []}
zp_bin, _ = which("zpool")
if zp_bin:
    try:
        zp = subprocess.run([zp_bin, "list", "-Hp", "-o", "name,size,alloc,free,frag,cap,health"],
                            capture_output=True, text=True, timeout=5)
        if zp.returncode == 0:
            zfs_info["has_zfs"] = True
            for line in zp.stdout.strip().splitlines():
                parts = line.split()
                if len(parts) >= 7:
                    zfs_info["pools"].append({
                        "name": parts[0],
                        "size_bytes": int(parts[1]),
                        "alloc_bytes": int(parts[2]),
                        "free_bytes": int(parts[3]),
                        "frag_pct": int(parts[4].replace("%","").replace("-","0")),
                        "cap_pct": int(parts[5].replace("%","")),
                        "health": parts[6]
                    })
        io = subprocess.run([zp_bin, "iostat", "-p", "1", "2"],
                            capture_output=True, text=True, timeout=5)
        if io.returncode == 0:
            lines = [l for l in io.stdout.strip().splitlines() if l and not l.startswith("---") and not l.startswith("capacity") and not l.startswith("pool")]
            if lines:
                last = lines[-1].split()
                if len(last) >= 7:
                    zfs_info["iostat"] = {
                        "pool": last[0],
                        "read_ops": int(last[3]),
                        "write_ops": int(last[4]),
                        "read_bytes_s": int(last[5]),
                        "write_bytes_s": int(last[6])
                    }
    except Exception:
        pass

zfs_bin, _ = which("zfs")
if zfs_bin and zfs_info["has_zfs"]:
    try:
        zf = subprocess.run([zfs_bin, "list", "-Hp", "-o", "name,used,avail,refer,mountpoint"],
                            capture_output=True, text=True, timeout=5)
        if zf.returncode == 0:
            for line in zf.stdout.strip().splitlines():
                parts = line.split()
                if len(parts) >= 5:
                    zfs_info["datasets"].append({
                        "name": parts[0],
                        "used_bytes": int(parts[1]),
                        "avail_bytes": int(parts[2]),
                        "refer_bytes": int(parts[3]),
                        "mountpoint": parts[4]
                    })
    except Exception:
        pass

# eBPF tools detection (opt-in, no overhead if missing)
ebpf_tools = []
for _t in ["bpftrace", "perf", "bpftool"]:
    _p, _ = which(_t)
    if _p:
        ebpf_tools.append(_t)
ebpf_available = len(ebpf_tools) > 0

# ytrace discovery — file-first, no daemon needed (Dash exclusively ytrace)
ytrace_info = {"has_ytrace": False, "providers": []}
try:
    import glob as _glob
    _xdg = os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share")
    _seen = []
    for _pat in [os.path.join(_xdg, "ytrace", "*", "ytrace.jsonl"), os.path.join(os.path.expanduser("~/.yggterm"), "ytrace.jsonl")]:
        for _p in _glob.glob(_pat):
            try:
                _sz = os.path.getsize(_p)
                _root = os.path.dirname(_p)
                _app = os.path.basename(_root) if _root != os.path.expanduser("~/.yggterm") else "yggterm"
                if _app == "ytrace":
                    _app = "yggterm"
                _seen.append({"app": _app, "home": _root, "live_bytes": _sz})
            except Exception:
                pass
    _yh = os.environ.get("YTRACE_HOME")
    if _yh:
        for _p in _glob.glob(os.path.join(_yh, "*", "ytrace.jsonl")):
            try:
                _sz = os.path.getsize(_p)
                _root = os.path.dirname(_p)
                _app = os.path.basename(_root)
                _seen.append({"app": _app, "home": _root, "live_bytes": _sz})
            except Exception:
                pass
    if _seen:
        # keep distinct homes, not just app (yggterm has XDG and legacy)
        ytrace_info["providers"] = _seen
        ytrace_info["has_ytrace"] = True
except Exception:
    pass

uptime = read("/proc/uptime").split()
print(json.dumps({
    "ok": True,
    "hostname": os.uname().nodename,
    "kernel": os.uname().release,
    "arch": os.uname().machine,
    "btime": stat_field("btime"),
    "cpu_model": cpu_model(),
    "cpu_count": os.cpu_count() or 0,
    "virt": virt(),
    "containers": guests,
    "container_tool": tool,
    "zfs": zfs_info,
    "ebpf_available": ebpf_available,
    "ebpf_tools": ebpf_tools,
    "uptime_s": float(uptime[0]) if uptime else 0.0,
    "load": [float(v) for v in read("/proc/loadavg").split()[:3]] or [0, 0, 0],
    "mem_total_kb": mem.get("MemTotal", 0),
    "mem_available_kb": mem.get("MemAvailable", 0),
    "swap_total_kb": mem.get("SwapTotal", 0),
    "swap_free_kb": mem.get("SwapFree", 0),
    "procs_total": len(after),
    "cpu_busy_pct": round(sum(p for _, p in rows), 1),
    "top": top,
    "sample_ms": round(elapsed * 1000),
    "ytrace": ytrace_info,
}))
"#;

/// Multiplexed-connection options, in ytop dash's own runtime directory.
///
/// ⚠ ITS OWN SOCKET DIRECTORY, NOT THE USER'S. Writing control sockets into
/// `~/.ssh/` would leave this app's plumbing in a directory whose contents
/// people reasonably assume they put there, and a stale socket in it looks like
/// a configuration problem rather than ours.
fn control_master_options() -> Vec<String> {
    let base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ytop")
        .join("ssh");
    if std::fs::create_dir_all(&base).is_err() {
        return Vec::new();
    }
    vec![
        "ControlMaster=auto".to_string(),
        format!("ControlPath={}/%C", base.display()),
        "ControlPersist=45s".to_string(),
    ]
}

/// One reading of one host. `host` is the ssh alias, or `None` for this
/// machine.
///
/// ⛔ A FAILURE IS REPORTED, NEVER SWALLOWED. "I could not look" and "the
/// machine is idle" are different facts, and a topology that renders the first
/// as the second is worse than one that shows nothing: it invites a decision.
pub fn read_host(host: Option<&str>, timeout: Duration) -> Value {
    let mut command = match host {
        None => {
            let mut c = Command::new("python3");
            c.arg("-");
            c
        }
        Some(h) => {
            let mut c = Command::new("ssh");
            c.arg("-o")
                .arg(format!("ConnectTimeout={}", timeout.as_secs().max(1)))
                .arg("-o")
                .arg("BatchMode=yes");
            // ⭐ REUSE THE CONNECTION. A refresh loop pays the full ssh
            //    handshake per host per frame otherwise, which is most of the
            //    wall clock and none of the information: measured at 13.6 s for
            //    a three-machine read that returns in well under a second once
            //    the sockets are up. ControlPersist keeps them just past a few
            //    refreshes, so an idle app does not sit on open sessions.
            for option in control_master_options() {
                c.arg("-o").arg(option);
            }
            c.arg(h)
                // The script arrives on STDIN. See the note at the top of this
                // file for why it may never be argv.
                .arg("python3 -");
            c
        }
    };
    command
        .env("YTOP_SAMPLE_MS", CPU_SAMPLE_MS.to_string())
        .env("YGGTOPO_SAMPLE_MS", CPU_SAMPLE_MS.to_string())
        .env("YTOP_TOP_N", TOP_N.to_string())
        .env("YGGTOPO_TOP_N", TOP_N.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let label = host.unwrap_or("local");
    let Ok(mut child) = command.spawn() else {
        return unreachable(label, "could not start the probe");
    };
    if let Some(stdin) = child.stdin.as_mut() {
        // A broken pipe here means the far side died before reading; the wait
        // below reports what it actually said.
        let _ = stdin.write_all(PROBE.as_bytes());
    }
    drop(child.stdin.take());
    let Ok(out) = child.wait_with_output() else {
        return unreachable(label, "the probe did not finish");
    };
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr);
        let why = why.lines().last().unwrap_or("no output").trim();
        return unreachable(label, if why.is_empty() { "no output" } else { why });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // The probe prints exactly one JSON object as its last line; anything a
    // login shell printed before it is somebody's motd, not our data.
    let Some(line) = text.lines().rev().find(|l| l.trim_start().starts_with('{')) else {
        return unreachable(label, "the probe printed no json");
    };
    match serde_json::from_str::<Value>(line) {
        Ok(mut value) => {
            // ⛔ THE KEY AND THE NAME ARE DIFFERENT THINGS. `host` is how this
            //    app addresses the machine (an ssh alias, or the sentinel for
            //    "here"); `label` is what a human should read. Rendering the
            //    sentinel put a row called "local" under a card titled with the
            //    machine's real name, which reads as two different machines.
            value["host"] = Value::String(label.to_string());
            value["label"] = Value::String(
                if host.is_none() {
                    value["hostname"].as_str().unwrap_or(label).to_string()
                } else {
                    label.to_string()
                },
            );
            value
        }
        Err(e) => unreachable(label, &format!("unreadable probe output: {e}")),
    }
}

fn unreachable(host: &str, why: &str) -> Value {
    serde_json::json!({
        "ok": false,
        "host": host, "label": host,
        "error": why,
    })
}

/// The DERIVED identity of the physical machine a reading came from.
///
/// ⭐ WHY THIS IS NOT CONFIGURED. A hand-maintained "these two are the same
/// box" list is a second source of truth about the topology, and it is wrong
/// the first time a guest moves. `btime` is the kernel's boot instant, shared
/// by every container on that kernel and not virtualised by lxcfs; the CPU and
/// core count are carried only to make an accidental collision — two distinct
/// machines booting in the same second on the same kernel — impossible in
/// practice rather than merely unlikely.
///
/// Returns `None` when the reading failed: an unreachable host must not be
/// grouped WITH anything, least of all with every other unreachable host.
pub fn machine_key(reading: &Value) -> Option<String> {
    if !reading["ok"].as_bool().unwrap_or(false) {
        return None;
    }
    let btime = reading["btime"].as_i64().unwrap_or(0);
    if btime == 0 {
        return None;
    }
    Some(format!(
        "{}|{}|{}|{}",
        reading["kernel"].as_str().unwrap_or("?"),
        btime,
        reading["cpu_model"].as_str().unwrap_or("?"),
        reading["cpu_count"].as_i64().unwrap_or(0),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reading(kernel: &str, btime: i64, cpu: &str, cores: i64) -> Value {
        json!({"ok": true, "kernel": kernel, "btime": btime,
               "cpu_model": cpu, "cpu_count": cores})
    }

    #[test]
    fn two_guests_on_one_kernel_share_a_machine_key() {
        let a = reading("9.9.9-invented", 1700000000, "Example CPU E1", 8);
        let b = reading("9.9.9-invented", 1700000000, "Example CPU E1", 8);
        assert_eq!(machine_key(&a), machine_key(&b));
        assert!(machine_key(&a).is_some());
    }

    #[test]
    fn a_different_machine_gets_a_different_key() {
        let a = reading("9.9.9-invented", 1700000000, "Example CPU E1", 8);
        let b = reading("8.8.8-invented", 1700000000, "Example CPU E1", 8);
        let c = reading("9.9.9-invented", 1700009999, "Example CPU E1", 8);
        let d = reading("9.9.9-invented", 1700000000, "Example CPU E2", 8);
        assert_ne!(machine_key(&a), machine_key(&b), "a different kernel is a different box");
        assert_ne!(machine_key(&a), machine_key(&c), "a different boot is a different box");
        assert_ne!(machine_key(&a), machine_key(&d), "a different cpu is a different box");
    }

    #[test]
    fn an_unreachable_host_is_grouped_with_nothing() {
        // ⛔ Two hosts we could not reach are not "the same machine". Grouping
        //    them would invent a topology out of our own blindness.
        let down = json!({"ok": false, "host": "example-a", "error": "timed out"});
        let also = json!({"ok": false, "host": "example-b", "error": "timed out"});
        assert_eq!(machine_key(&down), None);
        assert_eq!(machine_key(&also), None);
    }

    #[test]
    fn a_reading_with_no_btime_is_not_identified() {
        let vague = json!({"ok": true, "kernel": "9.9.9-invented",
                           "cpu_model": "Example CPU E1", "cpu_count": 8});
        assert_eq!(machine_key(&vague), None);
    }
}
