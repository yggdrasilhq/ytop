//! Timeline ring for AXIOM-like profiling — 5-min TTL, 1s buckets.
//!
//! Keeps `t0 + Vec<(t,row,cpu,rss,log_events)>` in ytop daemon memory.
//! No eBPF in Slice 1; pure `proc` delta + trace tail.

use std::collections::VecDeque;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct Sample {
    pub t_ms: u64,
    pub row: String,
    pub cpu_pct: f32,
    pub rss_kb: i64,
    pub log_events: u32,
}

#[derive(Debug, Default)]
pub struct Ring {
    t0_ms: u64,
    buckets: VecDeque<Sample>,
    ttl_ms: u64,
    bucket_ms: u64,
}

impl Ring {
    pub fn new(ttl: Duration, bucket: Duration) -> Self {
        Self {
            t0_ms: now_ms(),
            buckets: VecDeque::new(),
            ttl_ms: ttl.as_millis() as u64,
            bucket_ms: bucket.as_millis() as u64,
        }
    }

    pub fn push(&mut self, row: &str, cpu_pct: f64, rss_kb: i64, log_events: u32) {
        let t = now_ms();
        // Downsample: if last bucket for same row is within bucket_ms, replace
        if let Some(last) = self.buckets.back_mut() {
            if last.row == row && t.saturating_sub(last.t_ms) < self.bucket_ms {
                last.cpu_pct = cpu_pct as f32;
                last.rss_kb = rss_kb;
                last.log_events = log_events;
                last.t_ms = t;
                self.evict_old(t);
                return;
            }
        }
        self.buckets.push_back(Sample {
            t_ms: t,
            row: row.to_string(),
            cpu_pct: cpu_pct as f32,
            rss_kb,
            log_events,
        });
        self.evict_old(t);
    }

    fn evict_old(&mut self, now: u64) {
        while let Some(front) = self.buckets.front() {
            if now.saturating_sub(front.t_ms) > self.ttl_ms {
                self.buckets.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn snapshot(&self) -> Vec<Sample> {
        self.buckets.iter().cloned().collect()
    }

    pub fn since_ms(&self, ago_ms: u64) -> Vec<Sample> {
        let now = now_ms();
        let cutoff = now.saturating_sub(ago_ms);
        self.buckets.iter().filter(|s| s.t_ms >= cutoff).cloned().collect()
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or(Duration::from_secs(0)).as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ring_evicts_old() {
        let mut r = Ring::new(Duration::from_millis(100), Duration::from_millis(10));
        r.push("a", 10.0, 100, 1);
        std::thread::sleep(Duration::from_millis(110));
        r.push("b", 20.0, 200, 2);
        assert!(r.snapshot().iter().all(|s| s.row == "b"));
    }
    #[test]
    fn ring_downsamples_same_row() {
        let mut r = Ring::new(Duration::from_secs(300), Duration::from_secs(1));
        r.push("a", 10.0, 100, 1);
        r.push("a", 20.0, 200, 2);
        assert_eq!(r.snapshot().len(), 1);
        assert_eq!(r.snapshot()[0].cpu_pct, 20.0);
    }
}
