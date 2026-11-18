use std::net::SocketAddr;
use std::path::PathBuf;

// ##### KIND #####

/// What a process is, read from who launched it and where it lives.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Kind {
    Agent,
    Ide,
    Terminal,
    Service,
    Desktop,
    Container,
    System,
    Kernel,
}

impl Kind {
    pub const ALL: [Self; 8] = [
        Self::Agent,
        Self::Ide,
        Self::Terminal,
        Self::Service,
        Self::Desktop,
        Self::Container,
        Self::System,
        Self::Kernel,
    ];

    /// The kinds a fresh session shows: what the user launched.
    pub const MINE: [Self; 3] = [Self::Agent, Self::Ide, Self::Terminal];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Ide => "ide",
            Self::Terminal => "terminal",
            Self::Service => "service",
            Self::Desktop => "desktop",
            Self::Container => "container",
            Self::System => "system",
            Self::Kernel => "kernel",
        }
    }
}

// ##### NETWORK #####

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Proto {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Port {
    pub proto: Proto,
    pub addr: SocketAddr,
}

// ##### PROCESS #####

/// One process at one instant.
#[derive(Clone, Debug)]
pub struct Proc {
    pub pid: i32,
    pub name: String,
    pub cmd: String,
    pub kind: Kind,
    pub chain: Vec<(i32, String)>,
    pub cgroup: String,
    pub ports: Vec<Port>,
    pub cpu: f32,
    pub rss: u64,
    pub threads: i64,
    pub uptime: u64,
    pub vram: Option<u64>,
}

/// What only the selected process is asked for: too costly for every row.
#[derive(Clone, Debug, Default)]
pub struct Detail {
    pub pid: i32,
    pub exe: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub pss: Option<u64>,
    pub swap: Option<u64>,
    pub fds: Option<usize>,
    pub read: Option<u64>,
    pub write: Option<u64>,
}

// ##### CONTAINER #####

#[derive(Clone, Debug)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub project: Option<String>,
    pub service: Option<String>,
    pub state: String,
    pub status: String,
    pub ports: String,
}
