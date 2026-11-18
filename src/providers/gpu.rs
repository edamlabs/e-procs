use std::collections::HashMap;
use std::process::Command;

/// VRAM per process from `nvidia-smi` (~20 ms); empty without an NVIDIA driver.
pub struct GpuDriver;

impl GpuDriver {
    /// pid → MiB in use on any GPU.
    #[must_use]
    pub fn run() -> HashMap<i32, u64> {
        Command::new("nvidia-smi")
            .args(["--query-compute-apps=pid,used_memory", "--format=csv,noheader,nounits"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| Self::parse(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    }

    /// One `pid, MiB` line per process; a process on several GPUs is summed.
    #[must_use]
    pub fn parse(csv: &str) -> HashMap<i32, u64> {
        csv.lines()
            .filter_map(|l| l.split_once(','))
            .filter_map(|(pid, mib)| Some((pid.trim().parse().ok()?, mib.trim().parse::<u64>().ok()?)))
            .fold(HashMap::new(), |mut acc, (pid, mib)| {
                *acc.entry(pid).or_insert(0) += mib;
                acc
            })
    }
}

// ##### TESTS #####

#[cfg(test)]
mod tests {
    use super::GpuDriver;

    #[test]
    fn test_parse_sums_and_skips_garbage() {
        let vram = GpuDriver::parse("697512, 93\n1812805, 1588\n697512, 7\n[N/A], x\n\n");
        assert_eq!(vram.get(&697_512), Some(&100));
        assert_eq!(vram.get(&1_812_805), Some(&1588));
        assert_eq!(vram.len(), 2);
    }
}
