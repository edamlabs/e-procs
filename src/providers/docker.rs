use std::process::Command;

use serde::Deserialize;

use crate::model::Container;

const PROJECT: &str = "com.docker.compose.project";
const SERVICE: &str = "com.docker.compose.service";
const LOG_TAIL: &str = "300";

/// A compose action on one container.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Stop,
    Start,
    Restart,
}

impl Action {
    const fn verb(self) -> &'static str {
        match self {
            Self::Stop => "stop",
            Self::Start => "start",
            Self::Restart => "restart",
        }
    }
}

/// One line of `docker ps --format json`.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Row {
    #[serde(rename = "ID")]
    id: String,
    names: String,
    state: String,
    status: String,
    ports: String,
    labels: String,
}

/// Containers through the `docker` CLI (~55 ms a listing): no daemon client, no async runtime.
pub struct DockerDriver;

impl DockerDriver {
    fn common_docker(args: &[&str]) -> Result<String, String> {
        let out = Command::new("docker")
            .args(args)
            .output()
            .map_err(|e| format!("docker: {e}"))?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
            .ok_or_else(|| String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }

    fn parse_label(labels: &str, key: &str) -> Option<String> {
        labels
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.to_owned())
    }

    // ############################################################

    /// Every container, running or not, sorted by compose project then name.
    ///
    /// # Errors
    /// The daemon is unreachable or the CLI is missing.
    pub fn run() -> Result<Vec<Container>, String> {
        Self::common_docker(&["ps", "-a", "--format", "json"]).map(|out| Self::parse(&out))
    }

    /// One JSON object per line; a line that does not parse is skipped.
    #[must_use]
    pub fn parse(lines: &str) -> Vec<Container> {
        let mut all: Vec<Container> = lines
            .lines()
            .filter_map(|l| serde_json::from_str::<Row>(l).ok())
            .map(|r| Container {
                project: Self::parse_label(&r.labels, PROJECT),
                service: Self::parse_label(&r.labels, SERVICE),
                id: r.id,
                name: r.names,
                state: r.state,
                status: r.status,
                ports: r.ports,
            })
            .collect();
        all.sort_by(|a, b| (&a.project, &a.name).cmp(&(&b.project, &b.name)));
        all
    }

    /// # Errors
    /// The daemon refused the action.
    pub fn act(id: &str, action: Action) -> Result<(), String> {
        Self::common_docker(&[action.verb(), id]).map(|_| ())
    }

    /// The last lines a container wrote, stdout and stderr together.
    ///
    /// # Errors
    /// The daemon is unreachable or the container is gone.
    pub fn logs(id: &str) -> Result<String, String> {
        let out = Command::new("docker")
            .args(["logs", "--tail", LOG_TAIL, id])
            .output()
            .map_err(|e| format!("docker: {e}"))?;
        out.status
            .success()
            .then(|| {
                format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                )
            })
            .ok_or_else(|| String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

// ##### TESTS #####

#[cfg(test)]
mod tests {
    use super::DockerDriver;

    const LINES: &str = concat!(
        r#"{"ID":"b2","Names":"findr-gcs","Image":"gcs","State":"running","Status":"Up 2h","Ports":"0.0.0.0:4443->4443/tcp","Labels":"com.docker.compose.project=findr,com.docker.compose.service=gcs"}"#,
        "\n",
        r#"{"ID":"a1","Names":"loose","Image":"alpine","State":"exited","Status":"Exited (0)","Ports":"","Labels":""}"#,
        "\n",
        "not json\n",
        r#"{"ID":"c3","Names":"aivo-gate-1","Image":"gate","State":"running","Status":"Up","Ports":"","Labels":"com.docker.compose.project=aivo,com.docker.compose.service=gate"}"#,
    );

    #[test]
    fn test_parse_groups_by_project_and_skips_garbage() {
        let all = DockerDriver::parse(LINES);
        let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["loose", "aivo-gate-1", "findr-gcs"]);
        assert_eq!(all[2].project.as_deref(), Some("findr"));
        assert_eq!(all[2].service.as_deref(), Some("gcs"));
        assert_eq!(all[0].project, None);
    }
}
