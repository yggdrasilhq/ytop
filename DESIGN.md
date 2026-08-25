# DESIGN.md — ytop Architectural Design & Visual Contract

## 1. Visual Standard: The "R / ggplot2" Scientific Standard
Every visual element in `ytop` must meet scientific publication standards:
* **Sparklines & Histograms:** Unicode block quantization (` `, `▂`, `▃`, `▄`, `▅`, `▆`, `▇`, `█`) with explicit delta timeframes.
* **Proportional Flamegraphs:** Hierarchical folded stacks (`level1 › level2 › level3`) with proportional ASCII/Unicode percentage bars (`[████████░░░░░░░░] 50.0%`) and duration units in `ms`.
* **Structured Diagnostic Cards:** Rendered using the `emd-renderer` (enhanced markdown) banded card system matching Yggterm's Settings layout.

---

## 2. Layout & Partitioning Specification

### Top Mode
```
┌──────────────────────────────────────────┐
│ 🌐 CONNECTED FLEET (≤30% area)           │
│ 🖥️ jojo (GUI) · 15.0% CPU · 5.4 GB RAM  │
│ 🖥️ dev        ·  3.2% CPU · 2.1 GB RAM  │
│ 🖥️ manin (Ygg)· 37.6% CPU · 306.8 GB RAM│
├──────────────────────────────────────────┤
│ 📚 OPERATIONAL NOTEBOOKS                 │
│ 📁 Host Operations (Super-htop)       [3]│
│    📊 1. Health & CPU/Memory Breakdown   │
│    ⚡ 2. Runaway Processes & KILL Signals │
│    💾 3. Disk I/O & Memory Pressure      │
│ 📁 Yggdrasil Hypervisor & LXC        [2]│
│    📦 1. Container Fleet & Cgroups       │
│    🔥 2. Storage IOSTAT Flamegraph       │
│ 📁 Service Mesh & Uptime (Kuma)      [2]│
│    🌐 1. Service Status & Port Latency   │
│    📜 2. Outage Incident History         │
└──────────────────────────────────────────┘
```

### Dash Mode
```
┌──────────────────────────────────────────┐
│ 📚 APPLICATION OBSERVABILITY NOTEBOOKS   │
│ 📁 yggterm SysInternals (Base)       [5]│
│    🌳 1. Daemon & Client Process Graph   │
│    💺 2. Per-Seat Census & Attribution   │
│    📈 3. Resource Usage Trends (R Plots) │
│    🔬 4. Live ytrace Incident Stream     │
│    🛡️ 5. Supervision & Booter Watchdog    │
│ 📁 Ychrome Super-DevTools            [2]│
│    🔍 1. Profile & Tab Inspector         │
│    📊 2. DOM Latency & WebKit IPC        │
│ 📁 End-to-End Multi-Tier Trace       [2]│
│    🔥 1. Full-Stack Flamegraph           │
│    ⚡ 2. Keystroke → PTY → Render Journey│
│ 📁 Fleet Jankbox & Process Reaper    [1]│
│    💀 1. Subshell Leaks & Twin Reaping   │
│ 📁 Autonomous Diagnostic Watchdog    [1]│
│    🧠 1. Telemetry Anomalies & LLM Logs  │
└──────────────────────────────────────────┘
```

---

## 3. Super-htop Interactive Signal Dispatch
In `Host Operations`, unruly processes provide quick action affordances:
* `[ 🔴 SIGKILL (9) ]` — Force kill uncooperative processes immediately
* `[ 🟡 SIGTERM (15) ]` — Graceful termination signal
* `[ 🔵 SIGINT (2) ]` — Terminal interrupt signal
* `[ 🔄 SIGHUP (1) ]` — Configuration reload signal

---

## 4. Autonomous Agentic Harness
* Evaluates notebook telemetry invariants in a lightweight 10s evaluation loop.
* On anomaly trip (e.g. unpinned render storm >50 renders/s, subshell leak, or twin duplicate process):
  1. Emits structured `ytrace` incident (`payload.incident=true, complaint_for="llm"`).
  2. Escalates diagnostic report to Interface LLM or active campaign orchestrator.
