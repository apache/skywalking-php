// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Periodic PHM meter collection in the forked reporter worker. Samples the
//! parent PHP process via `/proc` and reports through `skywalking::metrics`
//! `Metricer`, booted from `start_worker` alongside heartbeat reporting.

use crate::channel::TxReporter;
use skywalking::metrics::{
    meter::Gauge,
    metricer::{Booting, Metricer},
};
use std::{
    fs,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tracing::{debug, trace, warn};

const METRIC_PROCESS_CPU: &str = "instance_php_process_cpu_utilization";
const DEFAULT_CLK_TCK: f64 = 100.0;
const METRIC_MEMORY_USED_MB: &str = "instance_php_memory_used_mb";
const METRIC_MEMORY_PEAK_MB: &str = "instance_php_memory_peak_mb";
const METRIC_THREAD_COUNT: &str = "instance_php_thread_count";
const METRIC_VIRTUAL_MEMORY_MB: &str = "instance_php_virtual_memory_mb";
const METRIC_OPEN_FD_COUNT: &str = "instance_php_open_fd_count";

#[derive(Clone)]
pub struct PhmConfiguration {
    pub service_name: String,
    pub service_instance: String,
    pub report_period_secs: i64,
}

#[derive(Clone)]
struct PhmCollectorConfiguration {
    report_period_secs: i64,
}

#[derive(Clone, Default)]
pub struct PhmSamples {
    memory_used_mb: Arc<AtomicU64>,
    memory_peak_mb: Arc<AtomicU64>,
    virtual_memory_mb: Arc<AtomicU64>,
    thread_count: Arc<AtomicU64>,
    open_fd_count: Arc<AtomicU64>,
    process_cpu: Arc<AtomicU64>,
}

impl PhmSamples {
    fn store(cell: &AtomicU64, value: f64) {
        cell.store(value.to_bits(), Ordering::Relaxed);
    }

    fn gauge(cell: Arc<AtomicU64>) -> impl Fn() -> f64 + Send + Sync + 'static {
        move || f64::from_bits(cell.load(Ordering::Relaxed))
    }
}

pub fn register_gauges(metricer: &mut Metricer, samples: PhmSamples) {
    metricer.register(Gauge::new(
        METRIC_MEMORY_USED_MB,
        PhmSamples::gauge(samples.memory_used_mb.clone()),
    ));
    metricer.register(Gauge::new(
        METRIC_MEMORY_PEAK_MB,
        PhmSamples::gauge(samples.memory_peak_mb.clone()),
    ));
    metricer.register(Gauge::new(
        METRIC_VIRTUAL_MEMORY_MB,
        PhmSamples::gauge(samples.virtual_memory_mb.clone()),
    ));
    metricer.register(Gauge::new(
        METRIC_THREAD_COUNT,
        PhmSamples::gauge(samples.thread_count.clone()),
    ));
    metricer.register(Gauge::new(
        METRIC_OPEN_FD_COUNT,
        PhmSamples::gauge(samples.open_fd_count.clone()),
    ));
    metricer.register(Gauge::new(
        METRIC_PROCESS_CPU,
        PhmSamples::gauge(samples.process_cpu.clone()),
    ));
}

struct CpuStatSample {
    utime: u64,
    stime: u64,
    wall_ms: u128,
}

fn update_samples(samples: &PhmSamples, cpu_sample: &mut Option<CpuStatSample>) -> Option<i32> {
    let pid = unsafe { libc::getppid() as i32 };
    if !process_alive(pid) {
        warn!(pid, "PHM target PHP process is gone, skip sample");
        return None;
    }

    let status = read_proc_status(pid);
    if let Some(mb) = status.vm_rss_mb {
        PhmSamples::store(&samples.memory_used_mb, mb);
    }
    if let Some(mb) = status.vm_hwm_mb {
        PhmSamples::store(&samples.memory_peak_mb, mb);
    }
    if let Some(mb) = status.vm_size_mb {
        PhmSamples::store(&samples.virtual_memory_mb, mb);
    }
    if let Some(count) = status.threads {
        PhmSamples::store(&samples.thread_count, count as f64);
    }
    if let Some(count) = read_open_fd_count(pid) {
        PhmSamples::store(&samples.open_fd_count, count);
    }
    if let Some((utime, stime)) = read_proc_stat_cpu(pid) {
        let now_ms = current_time_millis();
        let cpu = match cpu_sample {
            None => {
                *cpu_sample = Some(CpuStatSample {
                    utime,
                    stime,
                    wall_ms: now_ms,
                });
                None
            }
            Some(sample) => {
                let delta_jiffies =
                    utime.saturating_sub(sample.utime) + stime.saturating_sub(sample.stime);
                let delta_wall_ms = now_ms.saturating_sub(sample.wall_ms);
                sample.utime = utime;
                sample.stime = stime;
                sample.wall_ms = now_ms;
                Some(cpu_percent(delta_jiffies, delta_wall_ms))
            }
        };
        if let Some(cpu) = cpu {
            trace!(pid, cpu, "update PHM process CPU sample");
            PhmSamples::store(&samples.process_cpu, cpu);
        }
    } else {
        warn!(pid, "failed to read /proc stat for PHM CPU sampling");
    }
    debug!(pid, "PHM proc samples updated");
    Some(pid)
}

/// Populate gauges once before `Metricer::boot()` so the first report is not
/// all zeros.
pub fn warmup_samples(samples: &PhmSamples) {
    let mut cpu_sample = None;
    update_samples(samples, &mut cpu_sample);
}

pub fn boot_phm_metrics(config: PhmConfiguration, reporter: TxReporter) -> Booting {
    let samples = PhmSamples::default();
    let report_period = Duration::from_secs(config.report_period_secs.max(1) as u64);
    let collector_config = PhmCollectorConfiguration {
        report_period_secs: config.report_period_secs,
    };
    warmup_samples(&samples);
    run_phm_collector(collector_config, samples.clone());
    let mut metricer = Metricer::new(config.service_name, config.service_instance, reporter);
    metricer.set_report_interval(report_period);
    register_gauges(&mut metricer, samples);
    metricer.boot()
}

fn run_phm_collector(config: PhmCollectorConfiguration, samples: PhmSamples) {
    tokio::spawn(async move {
        let period = Duration::from_secs(config.report_period_secs.max(1) as u64);
        let mut cpu_sample: Option<CpuStatSample> = None;
        loop {
            if update_samples(&samples, &mut cpu_sample).is_none() {
                break;
            }
            tokio::time::sleep(period).await;
        }
    });
}

fn process_alive(pid: i32) -> bool {
    fs::metadata(format!("/proc/{pid}")).is_ok()
}

#[derive(Default)]
struct ProcStatusFields {
    vm_rss_mb: Option<f64>,
    vm_hwm_mb: Option<f64>,
    vm_size_mb: Option<f64>,
    threads: Option<u64>,
}

fn read_proc_status(pid: i32) -> ProcStatusFields {
    let Ok(content) = fs::read_to_string(format!("/proc/{pid}/status")) else {
        return ProcStatusFields::default();
    };
    let mut fields = ProcStatusFields::default();
    for line in content.lines() {
        if fields.vm_rss_mb.is_none() {
            fields.vm_rss_mb = parse_status_kib_line(line, "VmRSS");
        }
        if fields.vm_hwm_mb.is_none() {
            fields.vm_hwm_mb = parse_status_kib_line(line, "VmHWM");
        }
        if fields.vm_size_mb.is_none() {
            fields.vm_size_mb = parse_status_kib_line(line, "VmSize");
        }
        if fields.threads.is_none() {
            fields.threads = parse_status_count_line(line, "Threads");
        }
        if fields.vm_rss_mb.is_some()
            && fields.vm_hwm_mb.is_some()
            && fields.vm_size_mb.is_some()
            && fields.threads.is_some()
        {
            break;
        }
    }
    fields
}

fn parse_status_kib_line(line: &str, key: &str) -> Option<f64> {
    let prefix = format!("{key}:");
    if !line.starts_with(&prefix) {
        return None;
    }
    let kb: f64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb / 1024.0)
}

fn parse_status_count_line(line: &str, key: &str) -> Option<u64> {
    let prefix = format!("{key}:");
    if !line.starts_with(&prefix) {
        return None;
    }
    line.split_whitespace().nth(1)?.parse().ok()
}

fn read_open_fd_count(pid: i32) -> Option<f64> {
    let count = fs::read_dir(format!("/proc/{pid}/fd"))
        .ok()?
        .filter_map(|entry| entry.ok())
        .count();
    Some(count as f64)
}

fn read_proc_stat_cpu(pid: i32) -> Option<(u64, u64)> {
    let content = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rparen = content.rfind(')')?;
    let fields: Vec<&str> = content[rparen + 2..].split_whitespace().collect();
    let utime: u64 = fields.get(11)?.parse().ok()?;
    let stime: u64 = fields.get(12)?.parse().ok()?;
    Some((utime, stime))
}

fn cpu_percent(delta_jiffies: u64, delta_wall_ms: u128) -> f64 {
    if delta_wall_ms == 0 {
        return 0.0;
    }
    let clk_tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    let clk_tck = if clk_tck > 0 {
        clk_tck as f64
    } else {
        warn!(
            clk_tck,
            "sysconf(_SC_CLK_TCK) unavailable, using default {DEFAULT_CLK_TCK}"
        );
        DEFAULT_CLK_TCK
    };
    let cpu_sec = delta_jiffies as f64 / clk_tck;
    let wall_sec = delta_wall_ms as f64 / 1000.0;
    cpu_sec / wall_sec * 100.0
}

fn current_time_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default()
}
