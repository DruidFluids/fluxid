//! Live GPU metrics for non-NVIDIA (AMD / Intel, integrated + discrete) adapters
//! via the user-mode **D3DKMT** thunk interface (exports in `gdi32.dll`). This is
//! the same path Task Manager / LibreHardwareMonitor / HWiNFO use — no elevation,
//! no kernel driver, no vendor SDK. NVIDIA keeps using NVML; this fills the
//! clock / load / temperature that DXGI leaves as `None` for AMD & Intel.
#![cfg(windows)]
#![allow(dead_code)] // wired into read_gpu in a follow-up step

use std::time::Instant;
use windows::Wdk::Graphics::Direct3D::{
    D3DKMTCloseAdapter, D3DKMTOpenAdapterFromLuid, D3DKMTQueryAdapterInfo,
    D3DKMTQueryStatistics, D3DKMT_ADAPTER_PERFDATA, D3DKMT_CLOSEADAPTER,
    D3DKMT_NODE_PERFDATA, D3DKMT_OPENADAPTERFROMLUID, D3DKMT_QUERYADAPTERINFO,
    D3DKMT_QUERYSTATISTICS, D3DKMT_QUERYSTATISTICS_NODE, KMTQAITYPE_ADAPTERPERFDATA,
    KMTQAITYPE_NODEPERFDATA,
};
use windows::Win32::Foundation::{HANDLE, LUID};

/// How many engine nodes to scan for the live clock. The graphics engine isn't
/// always node 0 (notably on Intel), so we probe a handful. Matches the usage
/// sampler's node cap; the queries are cheap in-driver lookups.
const MAX_PERF_NODES: u32 = 16;

/// Query one engine node's perf data (clock/voltage). `None` if the query fails.
unsafe fn query_node_perf(hadapter: u32, node: u32) -> Option<D3DKMT_NODE_PERFDATA> {
    let mut perf = D3DKMT_NODE_PERFDATA::default();
    perf.NodeOrdinal = node;
    perf.PhysicalAdapterIndex = 0;
    let mut qai = D3DKMT_QUERYADAPTERINFO {
        hAdapter: hadapter,
        Type: KMTQAITYPE_NODEPERFDATA,
        pPrivateDriverData: &mut perf as *mut _ as *mut core::ffi::c_void,
        PrivateDriverDataSize: core::mem::size_of::<D3DKMT_NODE_PERFDATA>() as u32,
    };
    (D3DKMTQueryAdapterInfo(&mut qai).0 == 0).then_some(perf)
}

/// Query adapter-level perf data (memory clock / temperature / power / fan).
unsafe fn query_adapter_perf(hadapter: u32) -> Option<D3DKMT_ADAPTER_PERFDATA> {
    let mut ad = D3DKMT_ADAPTER_PERFDATA::default();
    ad.PhysicalAdapterIndex = 0;
    let mut qai = D3DKMT_QUERYADAPTERINFO {
        hAdapter: hadapter,
        Type: KMTQAITYPE_ADAPTERPERFDATA,
        pPrivateDriverData: &mut ad as *mut _ as *mut core::ffi::c_void,
        PrivateDriverDataSize: core::mem::size_of::<D3DKMT_ADAPTER_PERFDATA>() as u32,
    };
    (D3DKMTQueryAdapterInfo(&mut qai).0 == 0).then_some(ad)
}

/// Read `(clock_mhz, temp_c)` for the adapter with the given LUID. Either is
/// `None` when the driver doesn't populate it (returns 0 / a non-success status)
/// — many integrated GPUs report no separate temperature, which is expected.
pub fn read_clock_temp(luid: LUID) -> (Option<f32>, Option<f32>) {
    unsafe {
        let mut open = D3DKMT_OPENADAPTERFROMLUID { AdapterLuid: luid, hAdapter: 0 };
        if D3DKMTOpenAdapterFromLuid(&mut open).0 != 0 {
            return (None, None);
        }
        let hadapter = open.hAdapter;

        // Clock: the core clock is reported PER engine node, and node 0 is not
        // always the 3D/graphics engine — on Intel, node 0 reads 0 Hz, which is
        // what produced the blank "— MHz". Scan the nodes and take the highest live
        // frequency: under load that's the active 3D engine. Some(0.0) when the
        // queries work but every engine is idle/clock-gated (tile shows "—"); None
        // only when no node query succeeds (clock genuinely unsupported → row hidden).
        let mut any_node = false;
        let mut best_hz = 0u64;
        for node in 0..MAX_PERF_NODES {
            if let Some(perf) = query_node_perf(hadapter, node) {
                any_node = true;
                best_hz = best_hz.max(perf.Frequency);
            }
        }
        let clock_mhz = if best_hz > 0 {
            Some(best_hz as f32 / 1_000_000.0)
        } else if any_node {
            Some(0.0)
        } else {
            None
        };

        // Temperature: ADAPTER_PERFDATA.Temperature is in deci-Celsius (1 = 0.1°C).
        let temp_c = query_adapter_perf(hadapter)
            .filter(|ad| ad.Temperature > 0)
            .map(|ad| ad.Temperature as f32 / 10.0);

        let mut close = D3DKMT_CLOSEADAPTER { hAdapter: hadapter };
        let _ = D3DKMTCloseAdapter(&mut close);
        (clock_mhz, temp_c)
    }
}

/// Diagnostic dump for `--gpu-debug`: every engine node's live/max frequency plus
/// the adapter-level perf data, so we can see exactly which node carries the clock
/// on a given GPU (and whether the driver exposes WDDM perf data at all).
pub fn debug_dump(luid: LUID) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    unsafe {
        let mut open = D3DKMT_OPENADAPTERFROMLUID { AdapterLuid: luid, hAdapter: 0 };
        if D3DKMTOpenAdapterFromLuid(&mut open).0 != 0 {
            return "  D3DKMT: OpenAdapterFromLuid failed (no WDDM perf data — old driver/OS)\n".into();
        }
        let hadapter = open.hAdapter;
        for node in 0..MAX_PERF_NODES {
            match query_node_perf(hadapter, node) {
                Some(p) => {
                    let _ = writeln!(
                        s,
                        "  node {node:>2}: freq={:>6.0} MHz  max={:>6.0} MHz  volt={}",
                        p.Frequency as f64 / 1e6,
                        p.MaxFrequency as f64 / 1e6,
                        p.Voltage
                    );
                }
                None => {
                    let _ = writeln!(s, "  node {node:>2}: query failed");
                }
            }
        }
        match query_adapter_perf(hadapter) {
            Some(a) => {
                let _ = writeln!(
                    s,
                    "  adapter: memclk={:.0} MHz  maxmemclk={:.0} MHz  temp={:.1}C  power={}  fan={} rpm",
                    a.MemoryFrequency as f64 / 1e6,
                    a.MaxMemoryFrequency as f64 / 1e6,
                    a.Temperature as f64 / 10.0,
                    a.Power,
                    a.FanRPM
                );
            }
            None => {
                let _ = writeln!(s, "  adapter perf: query failed");
            }
        }
        let mut close = D3DKMT_CLOSEADAPTER { hAdapter: hadapter };
        let _ = D3DKMTCloseAdapter(&mut close);
    }
    s
}

/// Holds the previous utilization sample so usage % can be derived from the
/// delta of each engine's cumulative busy time between two polls.
#[derive(Default)]
pub struct UsageSampler {
    prev: Option<(Instant, Vec<i64>)>, // (sampled_at, running_time_100ns per node)
}

impl UsageSampler {
    /// Overall GPU usage % = the busiest engine's load (what Task Manager's
    /// headline GPU % shows), from the RunningTime delta vs the previous sample.
    /// `None` on the first call (no baseline) or when statistics are unavailable.
    pub fn read(&mut self, luid: LUID) -> Option<f32> {
        const MAX_NODES: u32 = 16;
        let now = Instant::now();
        let cur: Vec<i64> = (0..MAX_NODES)
            .map(|n| unsafe { query_node_running_time(luid, n) }.unwrap_or(0))
            .collect();

        let usage = match &self.prev {
            Some((t0, prev)) if prev.len() == cur.len() => {
                let elapsed_100ns = now.duration_since(*t0).as_nanos() as f64 / 100.0;
                if elapsed_100ns <= 0.0 {
                    None
                } else {
                    let max = cur
                        .iter()
                        .zip(prev)
                        .map(|(c, p)| ((c - p).max(0) as f64 / elapsed_100ns * 100.0).clamp(0.0, 100.0))
                        .fold(0.0f64, f64::max);
                    Some(max as f32)
                }
            }
            _ => None,
        };
        self.prev = Some((now, cur));
        usage
    }
}

/// Cumulative busy time (100ns ticks) of one engine/node, system-wide.
unsafe fn query_node_running_time(luid: LUID, node_id: u32) -> Option<i64> {
    let mut q: D3DKMT_QUERYSTATISTICS = core::mem::zeroed();
    q.Type = D3DKMT_QUERYSTATISTICS_NODE;
    q.AdapterLuid = luid;
    q.hProcess = HANDLE::default(); // NULL = system-wide (all processes)
    q.Anonymous.QueryNode.NodeId = node_id;
    if D3DKMTQueryStatistics(&q).0 != 0 {
        return None;
    }
    Some(q.QueryResult.NodeInformation.GlobalInformation.RunningTime)
}
