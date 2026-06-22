//! Formatting helpers for sensor values (byte-rate humanizing, temperature
//! units, model-name shortening).

use flux_core::settings::{AppSettings, TempUnit};

/// C# Shorten(): strip vendor prefixes, (R)/(TM), `"<N>-Core"`, trailing Processor/CPU/Graphics
pub fn shorten(name: &str) -> String {
    if name.trim().is_empty() {
        return String::new();
    }
    let original = name.trim();
    let mut n = original.to_string();
    let mut had_vendor = false;
    for p in ["AMD ", "NVIDIA ", "Intel(R) ", "Intel "] {
        if n.to_lowercase().starts_with(&p.to_lowercase()) {
            n = n[p.len()..].to_string();
            had_vendor = true;
            break;
        }
    }
    n = n.replace("(R)", "").replace("(TM)", "").replace("(tm)", "");
    // strip " <N>-Core" token
    let words: Vec<&str> = n.split_whitespace().collect();
    let filtered: Vec<&str> = words.into_iter()
        .filter(|w| {
            let lower = w.to_lowercase();
            !(lower.ends_with("-core") && lower.trim_end_matches("-core").parse::<u32>().is_ok())
        })
        .collect();
    n = filtered.join(" ");
    for s in [" Processor", " CPU", " Graphics"] {
        if n.to_lowercase().ends_with(&s.to_lowercase()) {
            n = n[..n.len() - s.len()].to_string();
            break;
        }
    }
    let n = n.trim();
    // If stripping collapsed the name to nothing or a lone generic word, keep the
    // vendor so the tile stays meaningful. Intel's Arrow Lake iGPU reports the bare
    // "Intel(R) Graphics", which would otherwise shorten to a context-free
    // "Graphics" — fall back to a cosmetically-cleaned original ("Intel Graphics").
    let generic = n.is_empty()
        || matches!(n.to_lowercase().as_str(), "graphics" | "processor" | "cpu");
    if generic && had_vendor {
        let cleaned = original
            .replace("(R)", "")
            .replace("(TM)", "")
            .replace("(tm)", "");
        return cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    n.to_string()
}

#[cfg(test)]
mod tests {
    use super::shorten;

    #[test]
    fn shorten_keeps_generic_intel_igpu_meaningful() {
        // Arrow Lake reports the bare "Intel(R) Graphics" — must not collapse to "Graphics".
        assert_eq!(shorten("Intel(R) Graphics"), "Intel Graphics");
    }

    #[test]
    fn shorten_still_trims_real_models() {
        assert_eq!(shorten("Intel(R) Iris(R) Xe Graphics"), "Iris Xe");
        assert_eq!(shorten("Intel(R) UHD Graphics 770"), "UHD Graphics 770");
        assert_eq!(shorten("AMD Radeon RX 7700 Graphics"), "Radeon RX 7700");
        assert_eq!(shorten("NVIDIA GeForce RTX 4070"), "GeForce RTX 4070");
    }

    #[test]
    fn shorten_empty_stays_empty() {
        assert_eq!(shorten("   "), "");
    }
}

/// (value, unit) pairs so tiles can render the unit in accent color.
/// Keep the readout to at most 3 significant chars so the unit never gets
/// squeezed onto a second line: one decimal only for single-digit values
/// ("9.9"), no decimal once we hit two digits ("25", "752").
fn short(v: f64) -> String {
    if v >= 10.0 { format!("{:.0}", v) } else { format!("{:.1}", v) }
}

pub fn fmt_net(bps: f64, bits: bool) -> (String, String) {
    if bits {
        // Bits/s (Kbps/Mbps/Gbps) use decimal (1000) units, as ISPs/NICs quote them.
        let b = bps * 8.0;
        return if !b.is_finite() || b < 1000.0 {
            (format!("{:.0}", b.max(0.0)), "bps".into())
        } else if b < 1_000_000.0 {
            (short(b / 1000.0), "Kbps".into())
        } else if b < 1_000_000_000.0 {
            (short(b / 1_000_000.0), "Mbps".into())
        } else {
            (short(b / 1_000_000_000.0), "Gbps".into())
        };
    }
    // NaN fails every `<` comparison, so without the `is_finite` guard a NaN
    // rate would fall through to the GB/s arm and render "NaN".
    if !bps.is_finite() || bps < 1024.0 {
        (format!("{:.0}", bps.max(0.0)), "B/s".into())
    } else if bps < 1024.0 * 1024.0 {
        (short(bps / 1024.0), "KB/s".into())
    } else if bps < 1024.0 * 1024.0 * 1024.0 {
        (short(bps / 1024.0 / 1024.0), "MB/s".into())
    } else {
        (short(bps / 1024.0 / 1024.0 / 1024.0), "GB/s".into())
    }
}

pub fn fmt_disk(bps: f64) -> (String, String) {
    if !bps.is_finite() || bps < 1024.0 {
        (format!("{:.0}", bps.max(0.0)), "B/s".into())
    } else if bps < 1024.0 * 1024.0 {
        (short(bps / 1024.0), "KB/s".into())
    } else if bps < 1024.0 * 1024.0 * 1024.0 {
        (short(bps / 1024.0 / 1024.0), "MB/s".into())
    } else {
        (short(bps / 1024.0 / 1024.0 / 1024.0), "GB/s".into())
    }
}

/// C# Temp(): em-dash when missing/<=0; (value, unit) otherwise
pub fn fmt_temp(temp_c: Option<f32>, settings: &AppSettings) -> Option<(String, String)> {
    let t = temp_c?;
    if t <= 0.0 {
        return None;
    }
    if settings.temperature_unit == TempUnit::Fahrenheit {
        Some((format!("{:.0}", t * 9.0 / 5.0 + 32.0), "\u{00B0}F".into()))
    } else {
        Some((format!("{:.0}", t), "\u{00B0}C".into()))
    }
}
