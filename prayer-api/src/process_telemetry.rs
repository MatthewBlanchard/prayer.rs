//! Host-process telemetry for the Prayer API binary.

use std::fs;
use std::time::Duration;

const DEFAULT_INTERVAL_SECS: u64 = 30;
const DEFAULT_WARN_RSS_MB: u64 = 8 * 1024;

/// Start periodic process telemetry unless disabled with `PRAYER_TELEMETRY=0`.
///
/// Configuration:
/// - `PRAYER_TELEMETRY_INTERVAL_SECS`: log interval, defaults to 30 seconds.
/// - `PRAYER_TELEMETRY_WARN_RSS_MB`: warning threshold, defaults to 8192 MiB.
pub fn start_process_telemetry(process_name: &'static str) {
    if env_flag_disabled("PRAYER_TELEMETRY") {
        tracing::info!(process = process_name, "process telemetry disabled");
        return;
    }

    let interval = env_u64("PRAYER_TELEMETRY_INTERVAL_SECS")
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_INTERVAL_SECS);
    let warn_rss_mb = env_u64("PRAYER_TELEMETRY_WARN_RSS_MB").unwrap_or(DEFAULT_WARN_RSS_MB);

    tracing::info!(
        process = process_name,
        interval_secs = interval,
        warn_rss_mb,
        "process telemetry enabled"
    );

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(interval));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            match ProcessSnapshot::capture() {
                Ok(snapshot) => {
                    if snapshot.rss_mb >= warn_rss_mb {
                        tracing::warn!(
                            process = process_name,
                            pid = snapshot.pid,
                            rss_mb = snapshot.rss_mb,
                            vm_size_mb = snapshot.vm_size_mb,
                            peak_rss_mb = snapshot.peak_rss_mb,
                            peak_vm_size_mb = snapshot.peak_vm_size_mb,
                            rss_anon_mb = snapshot.rss_anon_mb,
                            vm_data_mb = snapshot.vm_data_mb,
                            anon_maps = snapshot.anon_maps,
                            anon_rss_mb = snapshot.anon_rss_mb,
                            anon_128m_maps = snapshot.anon_128m_maps,
                            threads = snapshot.threads,
                            fd_count = snapshot.fd_count,
                            "process memory above warning threshold"
                        );
                    } else {
                        tracing::info!(
                            process = process_name,
                            pid = snapshot.pid,
                            rss_mb = snapshot.rss_mb,
                            vm_size_mb = snapshot.vm_size_mb,
                            peak_rss_mb = snapshot.peak_rss_mb,
                            peak_vm_size_mb = snapshot.peak_vm_size_mb,
                            rss_anon_mb = snapshot.rss_anon_mb,
                            vm_data_mb = snapshot.vm_data_mb,
                            anon_maps = snapshot.anon_maps,
                            anon_rss_mb = snapshot.anon_rss_mb,
                            anon_128m_maps = snapshot.anon_128m_maps,
                            threads = snapshot.threads,
                            fd_count = snapshot.fd_count,
                            "process telemetry"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        process = process_name,
                        error = %err,
                        "process telemetry capture failed"
                    );
                }
            }
        }
    });
}

#[derive(Debug)]
struct ProcessSnapshot {
    pid: u32,
    rss_mb: u64,
    vm_size_mb: u64,
    peak_rss_mb: u64,
    peak_vm_size_mb: u64,
    rss_anon_mb: u64,
    vm_data_mb: u64,
    anon_maps: u64,
    anon_rss_mb: u64,
    anon_128m_maps: u64,
    threads: u64,
    fd_count: u64,
}

impl ProcessSnapshot {
    fn capture() -> Result<Self, String> {
        let status = fs::read_to_string("/proc/self/status")
            .map_err(|err| format!("read /proc/self/status: {err}"))?;

        let anon = AnonMapSnapshot::capture().unwrap_or_default();
        Ok(Self {
            pid: std::process::id(),
            rss_mb: status_kb(&status, "VmRSS").unwrap_or(0) / 1024,
            vm_size_mb: status_kb(&status, "VmSize").unwrap_or(0) / 1024,
            peak_rss_mb: status_kb(&status, "VmHWM").unwrap_or(0) / 1024,
            peak_vm_size_mb: status_kb(&status, "VmPeak").unwrap_or(0) / 1024,
            rss_anon_mb: status_kb(&status, "RssAnon").unwrap_or(0) / 1024,
            vm_data_mb: status_kb(&status, "VmData").unwrap_or(0) / 1024,
            anon_maps: anon.maps,
            anon_rss_mb: anon.rss_kb / 1024,
            anon_128m_maps: anon.maps_128m,
            threads: status_value(&status, "Threads").unwrap_or(0),
            fd_count: fd_count().unwrap_or(0),
        })
    }
}

#[derive(Debug, Default)]
struct AnonMapSnapshot {
    maps: u64,
    maps_128m: u64,
    rss_kb: u64,
}

impl AnonMapSnapshot {
    fn capture() -> Result<Self, String> {
        let smaps = fs::read_to_string("/proc/self/smaps")
            .map_err(|err| format!("read /proc/self/smaps: {err}"))?;
        let mut snapshot = Self::default();
        let mut current = MapEntry::default();
        for line in smaps.lines() {
            if is_smaps_header(line) {
                current.flush_into(&mut snapshot);
                current = MapEntry::from_header(line);
            } else if let Some(value) = smaps_kb(line, "Size") {
                current.size_kb = value;
            } else if let Some(value) = smaps_kb(line, "Rss") {
                current.rss_kb = value;
            }
        }
        current.flush_into(&mut snapshot);
        Ok(snapshot)
    }
}

#[derive(Debug, Default)]
struct MapEntry {
    anonymous: bool,
    size_kb: u64,
    rss_kb: u64,
}

impl MapEntry {
    fn from_header(line: &str) -> Self {
        Self {
            anonymous: line.split_whitespace().count() <= 5,
            size_kb: 0,
            rss_kb: 0,
        }
    }

    fn flush_into(&self, snapshot: &mut AnonMapSnapshot) {
        if !self.anonymous {
            return;
        }
        snapshot.maps = snapshot.maps.saturating_add(1);
        snapshot.rss_kb = snapshot.rss_kb.saturating_add(self.rss_kb);
        if self.size_kb == 128 * 1024 {
            snapshot.maps_128m = snapshot.maps_128m.saturating_add(1);
        }
    }
}

fn env_flag_disabled(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.trim(), "0" | "false" | "FALSE" | "off" | "OFF"))
        .unwrap_or(false)
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok()?.trim().parse().ok()
}

fn status_kb(status: &str, key: &str) -> Option<u64> {
    let raw = status_value(status, key)?;
    Some(raw)
}

fn status_value(status: &str, key: &str) -> Option<u64> {
    let prefix = format!("{key}:");
    let line = status.lines().find(|line| line.starts_with(&prefix))?;
    line[prefix.len()..].split_whitespace().next()?.parse().ok()
}

fn is_smaps_header(line: &str) -> bool {
    let Some(first) = line.split_whitespace().next() else {
        return false;
    };
    let Some((start, end)) = first.split_once('-') else {
        return false;
    };
    !start.is_empty()
        && !end.is_empty()
        && start.bytes().all(|b| b.is_ascii_hexdigit())
        && end.bytes().all(|b| b.is_ascii_hexdigit())
}

fn smaps_kb(line: &str, key: &str) -> Option<u64> {
    let prefix = format!("{key}:");
    line.strip_prefix(&prefix)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

fn fd_count() -> Option<u64> {
    let count = fs::read_dir("/proc/self/fd")
        .ok()?
        .filter_map(Result::ok)
        .count();
    u64::try_from(count).ok()
}
