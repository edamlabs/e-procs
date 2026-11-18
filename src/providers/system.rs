use std::collections::HashMap;
use std::fs;
use std::time::Instant;

use procfs::net::{TcpState, UdpState};
use procfs::process::{FDTarget, Process, Stat};

use crate::classify::{self, Facts};
use crate::model::{Detail, Port, Proc, Proto};

const PF_KTHREAD: u32 = 0x0020_0000;
const CHAIN_DEPTH: usize = 32;

// ##### RAW #####

/// One process as read from /proc, before the tree around it is known.
struct Raw {
    stat: Stat,
    uid: u32,
    cmd: String,
    cgroup: String,
    inodes: Vec<u64>,
}

/// Reads every process, its sockets and its resources from /proc.
pub struct SystemDriver {
    me: u32,
    ticks: u64,
    page: u64,
    prev: HashMap<i32, u64>,
    at: Instant,
}

impl SystemDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            me: rustix::process::getuid().as_raw(),
            ticks: procfs::ticks_per_second(),
            page: procfs::page_size(),
            prev: HashMap::new(),
            at: Instant::now(),
        }
    }

    fn run_sockets() -> HashMap<u64, Port> {
        let tcp = [procfs::net::tcp(), procfs::net::tcp6()]
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.state == TcpState::Listen)
            .map(|e| {
                (
                    e.inode,
                    Port {
                        proto: Proto::Tcp,
                        addr: e.local_address,
                    },
                )
            });
        let udp = [procfs::net::udp(), procfs::net::udp6()]
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.state == UdpState::Close && e.local_address.port() != 0)
            .map(|e| {
                (
                    e.inode,
                    Port {
                        proto: Proto::Udp,
                        addr: e.local_address,
                    },
                )
            });
        tcp.chain(udp).collect()
    }

    fn run_raw(&self, process: &Process) -> Option<Raw> {
        let stat = process.stat().ok()?;
        let uid = process.uid().ok()?;
        // Another user's fds are unreadable without privilege: their ports stay unknown.
        let inodes = (uid == self.me)
            .then(|| process.fd().ok())
            .flatten()
            .map(|fds| {
                fds.flatten()
                    .filter_map(|fd| match fd.target {
                        FDTarget::Socket(inode) => Some(inode),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Raw {
            cmd: process.cmdline().map(|c| c.join(" ")).unwrap_or_default(),
            cgroup: fs::read_to_string(format!("/proc/{}/cgroup", stat.pid))
                .map(|s| {
                    s.lines()
                        .last()
                        .and_then(|l| l.splitn(3, ':').nth(2))
                        .unwrap_or_default()
                        .to_owned()
                })
                .unwrap_or_default(),
            stat,
            uid,
            inodes,
        })
    }

    fn run_chain(pid: i32, raws: &HashMap<i32, Raw>) -> Vec<(i32, String)> {
        std::iter::successors(raws.get(&pid), |r| {
            raws.get(&r.stat.ppid).filter(|p| p.stat.pid != r.stat.pid)
        })
        .take(CHAIN_DEPTH)
        .map(|r| (r.stat.pid, r.stat.comm.clone()))
        .collect()
    }

    #[allow(clippy::cast_precision_loss)]
    fn run_cpu(&self, pid: i32, ticks: u64, elapsed: f32) -> f32 {
        self.prev.get(&pid).map_or(0.0, |&before| {
            ticks.saturating_sub(before) as f32 / self.ticks as f32 / elapsed * 100.0
        })
    }

    // ############################################################

    /// Every process now, its kind, chain, ports, CPU since the last call, and VRAM from `vram` (pid → MiB).
    pub fn run(&mut self, vram: &HashMap<i32, u64>) -> Vec<Proc> {
        let sockets = Self::run_sockets();
        let raws: HashMap<i32, Raw> = procfs::process::all_processes()
            .map(|all| {
                all.flatten()
                    .filter_map(|p| self.run_raw(&p))
                    .map(|r| (r.stat.pid, r))
                    .collect()
            })
            .unwrap_or_default();
        let boot = procfs::boot_time_secs().unwrap_or_default();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let elapsed = self.at.elapsed().as_secs_f32().max(0.001);
        let procs = raws
            .values()
            .map(|r| {
                let chain = Self::run_chain(r.stat.pid, &raws);
                let names: Vec<&str> = chain.iter().map(|(_, n)| n.as_str()).collect();
                let kind = classify::kind(&Facts {
                    uid: r.uid,
                    me: self.me,
                    kthread: r.stat.flags & PF_KTHREAD != 0,
                    cgroup: &r.cgroup,
                    chain: &names,
                });
                Proc {
                    pid: r.stat.pid,
                    name: r.stat.comm.clone(),
                    cmd: r.cmd.clone(),
                    kind,
                    cgroup: r.cgroup.clone(),
                    ports: r.inodes.iter().filter_map(|i| sockets.get(i).copied()).collect(),
                    cpu: self.run_cpu(r.stat.pid, r.stat.utime + r.stat.stime, elapsed),
                    rss: r.stat.rss * self.page,
                    threads: r.stat.num_threads,
                    uptime: now.saturating_sub(boot + r.stat.starttime / self.ticks),
                    vram: vram.get(&r.stat.pid).copied(),
                    chain,
                }
            })
            .collect();
        self.prev = raws
            .iter()
            .map(|(&pid, r)| (pid, r.stat.utime + r.stat.stime))
            .collect();
        self.at = Instant::now();
        procs
    }

    /// What only the selected process is asked for: memory proportional share, swap, fds and disk I/O.
    #[must_use]
    pub fn detail(pid: i32) -> Detail {
        let process = Process::new(pid).ok();
        let rollup = fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).unwrap_or_default();
        let field = |key: &str| {
            rollup
                .lines()
                .find_map(|l| l.strip_prefix(key))
                .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
                .map(|kb| kb * 1024)
        };
        let io = process.as_ref().and_then(|p| p.io().ok());
        Detail {
            pid,
            exe: process.as_ref().and_then(|p| p.exe().ok()),
            cwd: process.as_ref().and_then(|p| p.cwd().ok()),
            pss: field("Pss:"),
            swap: field("Swap:"),
            fds: process.as_ref().and_then(|p| p.fd_count().ok()),
            read: io.as_ref().map(|i| i.read_bytes),
            write: io.as_ref().map(|i| i.write_bytes),
        }
    }
}

impl Default for SystemDriver {
    fn default() -> Self {
        Self::new()
    }
}
