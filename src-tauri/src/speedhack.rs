use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::Duration;

use crate::commands::SpeedhackState;

fn find_game_pid() -> Option<u32> {
    let names = ["TaskbarHero", "Task Bar Hero"];
    for name in &names {
        if let Ok(out) = std::process::Command::new("pgrep")
            .args(["-f", "-i", name])
            .output()
            && out.status.success()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            if let Some(line) = s.lines().next()
                && let Ok(pid) = line.trim().parse::<u32>()
            {
                return Some(pid);
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let pid = name_str.parse::<u32>().ok()?;
            if let Ok(cmdline) = std::fs::read_to_string(entry.path().join("cmdline"))
                && names.iter().any(|g| cmdline.contains(g))
            {
                return Some(pid);
            }
        }
    }
    None
}

fn get_writable_scan_regions(pid: u32) -> Vec<(u64, u64)> {
    let Ok(maps) = std::fs::read_to_string(format!("/proc/{}/maps", pid)) else {
        return vec![];
    };
    let mut regions = Vec::new();
    for line in maps.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let perms = parts[1];
        if !perms.starts_with("rw") || !perms.contains('p') {
            continue;
        }
        let addrs: Vec<&str> = parts[0].split('-').collect();
        if addrs.len() != 2 {
            continue;
        }
        let start = u64::from_str_radix(addrs[0], 16).ok();
        let end = u64::from_str_radix(addrs[1], 16).ok();
        if let (Some(start), Some(end)) = (start, end) {
            let size = end.saturating_sub(start);
            if (4..=64 * 1024 * 1024).contains(&size) {
                regions.push((start, end));
            }
        }
    }
    regions
}

fn scan_for_float(pid: u32, value: f32) -> Vec<u64> {
    let target = value.to_le_bytes();
    let regions = get_writable_scan_regions(pid);
    let Ok(mut mem) = OpenOptions::new()
        .read(true)
        .open(format!("/proc/{}/mem", pid))
    else {
        return vec![];
    };
    let mut addrs = Vec::new();
    let mut buf = Vec::new();
    for (start, end) in &regions {
        let size = (end - start) as usize;
        buf.resize(size, 0);
        if mem.seek(SeekFrom::Start(*start)).is_err() || mem.read_exact(&mut buf).is_err() {
            continue;
        }
        for (offset, chunk) in buf.windows(4).enumerate() {
            if chunk == target {
                addrs.push(start + offset as u64);
                if addrs.len() >= 5000 {
                    return addrs;
                }
            }
        }
    }
    addrs
}

fn write_float(pid: u32, addrs: &[u64], value: f32) -> usize {
    let bytes = value.to_le_bytes();
    let Ok(mut mem) = OpenOptions::new()
        .write(true)
        .open(format!("/proc/{}/mem", pid))
    else {
        return 0;
    };
    let mut count = 0;
    for &addr in addrs {
        if mem.seek(SeekFrom::Start(addr)).is_ok() && mem.write_all(&bytes).is_ok() {
            count += 1;
        }
    }
    count
}

pub struct Speedhack {
    enabled: Arc<AtomicBool>,
    multiplier: Arc<Mutex<f32>>,
    game_running: Arc<AtomicBool>,
    addresses_found: Arc<AtomicUsize>,
    last_write_ok: Arc<AtomicBool>,
    last_verify_ok: Arc<AtomicBool>,
    stop_flag: Arc<AtomicBool>,
}

impl Speedhack {
    pub fn new(state: &SpeedhackState) -> Self {
        Self {
            enabled: state.enabled.clone(),
            multiplier: state.multiplier.clone(),
            game_running: state.game_running.clone(),
            addresses_found: state.addresses_found.clone(),
            last_write_ok: state.last_write_ok.clone(),
            last_verify_ok: state.last_verify_ok.clone(),
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start_loop(&self) {
        let enabled = self.enabled.clone();
        let multiplier = self.multiplier.clone();
        let game_running = self.game_running.clone();
        let addresses_found = self.addresses_found.clone();
        let last_write_ok = self.last_write_ok.clone();
        let last_verify_ok = self.last_verify_ok.clone();
        let stop = self.stop_flag.clone();
        std::thread::spawn(move || {
            let mut tracked: Vec<u64> = Vec::new();
            let mut scan_value: f32 = 1.0;
            while !stop.load(Ordering::Relaxed) {
                let pid = find_game_pid();
                game_running.store(pid.is_some(), Ordering::Relaxed);

                if !enabled.load(Ordering::Relaxed) {
                    if !tracked.is_empty() {
                        if let Some(pid) = pid {
                            write_float(pid, &tracked, 1.0);
                        }
                        tracked.clear();
                    }
                    addresses_found.store(0, Ordering::Relaxed);
                    last_write_ok.store(false, Ordering::Relaxed);
                    last_verify_ok.store(false, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(300));
                    continue;
                }

                let Some(pid) = pid else {
                    std::thread::sleep(Duration::from_millis(1000));
                    continue;
                };

                let mult = *multiplier.lock().unwrap();
                let changed = (mult - scan_value).abs() > 0.01;

                if tracked.is_empty() || (changed && tracked.len() > 100) {
                    let mut addrs = scan_for_float(pid, scan_value);
                    if addrs.is_empty() {
                        addrs = scan_for_float(pid, 1.0);
                    }
                    if addrs.is_empty() && !tracked.is_empty() {
                        addrs = tracked.clone();
                    }
                    if addrs.len() > 5000 {
                        addrs.truncate(5000);
                    }
                    tracked = addrs;
                    addresses_found.store(tracked.len(), Ordering::Relaxed);
                    if tracked.is_empty() {
                        last_write_ok.store(false, Ordering::Relaxed);
                    }
                }

                if changed {
                    scan_value = mult;
                }

                if !tracked.is_empty() {
                    let write_ok = write_float(pid, &tracked, mult) == tracked.len();
                    last_write_ok.store(write_ok, Ordering::Relaxed);
                    if write_ok {
                        let matches = verify_addresses(pid, &tracked, mult);
                        last_verify_ok.store(matches == tracked.len(), Ordering::Relaxed);
                    } else {
                        last_verify_ok.store(false, Ordering::Relaxed);
                    }
                }

                std::thread::sleep(Duration::from_millis(500));
            }
        });
    }

    #[allow(dead_code)]
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Relaxed);
    }
}

fn verify_addresses(pid: u32, addrs: &[u64], expected: f32) -> usize {
    let target = expected.to_le_bytes();
    let Ok(mut mem) = OpenOptions::new()
        .read(true)
        .open(format!("/proc/{}/mem", pid))
    else {
        return 0;
    };
    let mut buf = [0u8; 4];
    let mut count = 0;
    for &addr in addrs {
        if mem.seek(SeekFrom::Start(addr)).is_ok()
            && mem.read_exact(&mut buf).is_ok()
            && buf == target
        {
            count += 1;
        }
    }
    count
}
