use rustix::io::Errno;
use rustix::process::{Pid, Signal, kill_process};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sig {
    Term,
    Kill,
}

/// Why a signal did not land.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    Gone,
    Denied,
    Invalid,
    Other(i32),
}

impl Refusal {
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Gone => "no such process (already gone)".to_owned(),
            Self::Denied => "not permitted (another user's process)".to_owned(),
            Self::Invalid => "invalid pid".to_owned(),
            Self::Other(code) => format!("errno {code}"),
        }
    }
}

/// Sends a signal to one pid; never to a group, never to pid 0 or 1.
pub struct SignalDriver;

impl SignalDriver {
    /// # Errors
    /// The refusal: the process is gone, belongs to someone else, or the pid is not a real process.
    pub fn run(pid: i32, sig: Sig) -> Result<(), Refusal> {
        let target = Pid::from_raw(pid).filter(|p| !p.is_init()).ok_or(Refusal::Invalid)?;
        let signal = match sig {
            Sig::Term => Signal::TERM,
            Sig::Kill => Signal::KILL,
        };
        kill_process(target, signal).map_err(|e| match e {
            Errno::SRCH => Refusal::Gone,
            Errno::PERM => Refusal::Denied,
            other => Refusal::Other(other.raw_os_error()),
        })
    }
}

// ##### TESTS #####

#[cfg(test)]
mod tests {
    use super::{Refusal, Sig, SignalDriver};
    use std::process::Command;

    #[test]
    fn test_run_kills_own_child() -> std::io::Result<()> {
        let mut child = Command::new("sleep").arg("30").spawn()?;
        let pid = i32::try_from(child.id()).map_err(std::io::Error::other)?;
        assert_eq!(SignalDriver::run(pid, Sig::Term), Ok(()));
        assert!(!child.wait()?.success());
        Ok(())
    }

    #[test]
    fn test_run_gone_pid() -> std::io::Result<()> {
        let mut child = Command::new("true").spawn()?;
        let pid = i32::try_from(child.id()).map_err(std::io::Error::other)?;
        child.wait()?;
        assert_eq!(SignalDriver::run(pid, Sig::Kill), Err(Refusal::Gone));
        Ok(())
    }

    #[test]
    fn test_run_refuses_init_and_zero() {
        assert_eq!(SignalDriver::run(1, Sig::Term), Err(Refusal::Invalid));
        assert_eq!(SignalDriver::run(0, Sig::Term), Err(Refusal::Invalid));
    }
}
