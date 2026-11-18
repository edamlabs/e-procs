use crate::model::Kind;

// ##### MARKERS #####

/// Process names that mark who launched a subtree; the nearest one up the chain decides.
const MARKERS: [(&str, Kind); 26] = [
    ("claude", Kind::Agent),
    ("codex", Kind::Agent),
    ("opencode", Kind::Agent),
    ("gemini", Kind::Agent),
    ("aider", Kind::Agent),
    ("goose", Kind::Agent),
    ("code", Kind::Ide),
    ("cursor", Kind::Ide),
    ("codium", Kind::Ide),
    ("windsurf", Kind::Ide),
    ("zed", Kind::Ide),
    ("zed-editor", Kind::Ide),
    ("idea", Kind::Ide),
    ("pycharm", Kind::Ide),
    ("rustrover", Kind::Ide),
    ("gnome-terminal-", Kind::Terminal),
    ("ptyxis", Kind::Terminal),
    ("ptyxis-agent", Kind::Terminal),
    ("kitty", Kind::Terminal),
    ("alacritty", Kind::Terminal),
    ("wezterm-gui", Kind::Terminal),
    ("konsole", Kind::Terminal),
    ("tilix", Kind::Terminal),
    ("xterm", Kind::Terminal),
    ("foot", Kind::Terminal),
    ("tmux: server", Kind::Terminal),
];

const CONTAINER_CGROUPS: [&str; 4] = ["/docker-", "/docker/", "/libpod-", "/kubepods"];

// ##### FACTS #####

/// What the classifier reads of one process: nothing it could fetch by itself.
pub struct Facts<'a> {
    pub uid: u32,
    pub me: u32,
    pub kthread: bool,
    pub cgroup: &'a str,
    /// The process name first, then each ancestor's, nearest first.
    pub chain: &'a [&'a str],
}

/// The kind of a process: kernel, then container, then foreign user, then the nearest marker, then its cgroup.
#[must_use]
pub fn kind(facts: &Facts) -> Kind {
    match () {
        () if facts.kthread => Kind::Kernel,
        () if CONTAINER_CGROUPS.iter().any(|c| facts.cgroup.contains(c)) => Kind::Container,
        () if facts.uid != facts.me => Kind::System,
        () => facts
            .chain
            .iter()
            .find_map(|name| MARKERS.iter().find(|(m, _)| m == name).map(|&(_, k)| k))
            .unwrap_or_else(|| unit(facts.cgroup)),
    }
}

/// An unmarked process of the user: a systemd unit is a service, anything else the desktop launched.
fn unit(cgroup: &str) -> Kind {
    cgroup
        .rsplit('/')
        .next()
        .filter(|leaf| leaf.ends_with(".service"))
        .map_or(Kind::Desktop, |_| Kind::Service)
}

// ##### TESTS #####

#[cfg(test)]
mod tests {
    use super::{Facts, kind};
    use crate::model::Kind;

    const VTE: &str =
        "/user.slice/user-1000.slice/user@1000.service/app.slice/app-org.gnome.Terminal.slice/vte-spawn-1.scope";

    fn of(uid: u32, kthread: bool, cgroup: &str, chain: &[&str]) -> Kind {
        kind(&Facts {
            uid,
            me: 1000,
            kthread,
            cgroup,
            chain,
        })
    }

    #[test]
    fn test_kind_kernel_wins() {
        assert_eq!(of(0, true, "/", &["kworker/0:1", "kthreadd"]), Kind::Kernel);
    }

    #[test]
    fn test_kind_container_by_cgroup() {
        assert_eq!(
            of(
                0,
                false,
                "/system.slice/docker-ab12.scope",
                &["postgres", "containerd-shim"]
            ),
            Kind::Container
        );
    }

    #[test]
    fn test_kind_foreign_uid_is_system() {
        assert_eq!(
            of(0, false, "/system.slice/ssh.service", &["sshd", "systemd"]),
            Kind::System
        );
    }

    #[test]
    fn test_kind_nearest_marker_decides() {
        assert_eq!(
            of(1000, false, VTE, &["python", "uv", "claude", "bash", "gnome-terminal-"]),
            Kind::Agent
        );
        assert_eq!(
            of(1000, false, VTE, &["python", "bash", "gnome-terminal-"]),
            Kind::Terminal
        );
        assert_eq!(of(1000, false, VTE, &["cargo", "bash", "code"]), Kind::Ide);
    }

    #[test]
    fn test_kind_unmarked_service_and_desktop() {
        let service = "/user.slice/user-1000.slice/user@1000.service/app.slice/syncthing.service";
        let scope = "/user.slice/user-1000.slice/user@1000.service/app.slice/app-gnome-slack-1.scope";
        assert_eq!(of(1000, false, service, &["syncthing", "systemd"]), Kind::Service);
        assert_eq!(of(1000, false, scope, &["slack", "gnome-shell"]), Kind::Desktop);
    }

    #[test]
    fn test_kind_empty_chain_falls_to_cgroup() {
        assert_eq!(of(1000, false, "", &[]), Kind::Desktop);
    }
}
