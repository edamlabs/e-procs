use std::collections::VecDeque;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::widgets::TableState;

use crate::model::{Container, Detail, Kind, Proc};
use crate::providers::docker::Action;
use crate::providers::signal::Sig;

const HISTORY: usize = 120;

// ##### STATE #####

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Procs,
    Compose,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    Cpu,
    Mem,
    Pid,
    Name,
}

impl Sort {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Cpu => Self::Mem,
            Self::Mem => Self::Pid,
            Self::Pid => Self::Name,
            Self::Name => Self::Cpu,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Mem => "mem",
            Self::Pid => "pid",
            Self::Name => "name",
        }
    }
}

/// What a key asks the world to do; the root carries it out off the UI thread.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Effect {
    None,
    Quit,
    Signal(i32, Sig),
    Docker(String, Action),
    Logs(String, String),
    Watch(Option<i32>),
}

/// A destructive action waiting for `y`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pending {
    pub label: String,
    pub effect: Effect,
}

pub struct Logs {
    pub title: String,
    pub lines: Vec<String>,
    pub scroll: usize,
}

pub struct App {
    pub tab: Tab,
    pub procs: Vec<Proc>,
    pub rows: Vec<usize>,
    pub kinds: [bool; Kind::ALL.len()],
    pub listening: bool,
    pub sort: Sort,
    pub filter: String,
    pub typing: bool,
    pub table: TableState,
    pub compose: Result<Vec<Container>, String>,
    pub boxes: TableState,
    pub detail: Option<Detail>,
    pub open: bool,
    pub history: VecDeque<u64>,
    pub pending: Option<Pending>,
    pub logs: Option<Logs>,
    pub status: String,
}

impl App {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tab: Tab::Procs,
            procs: Vec::new(),
            rows: Vec::new(),
            kinds: Kind::ALL.map(|k| Kind::MINE.contains(&k)),
            listening: false,
            sort: Sort::Cpu,
            filter: String::new(),
            typing: false,
            table: TableState::default().with_selected(Some(0)),
            compose: Ok(Vec::new()),
            boxes: TableState::default().with_selected(Some(0)),
            detail: None,
            open: false,
            history: VecDeque::with_capacity(HISTORY),
            pending: None,
            logs: None,
            status: String::new(),
        }
    }

    fn common_kind_on(&self, kind: Kind) -> bool {
        Kind::ALL.iter().position(|&k| k == kind).is_some_and(|i| self.kinds[i])
    }

    fn common_matches(&self, p: &Proc) -> bool {
        let needle = self.filter.to_lowercase();
        // A number is a pid or a port, never a substring: it would match every argv that mentions it.
        let text = match needle.parse::<u32>() {
            Ok(n) => {
                u32::try_from(p.pid).is_ok_and(|pid| pid == n)
                    || p.ports.iter().any(|port| u32::from(port.addr.port()) == n)
            }
            Err(_) => {
                needle.is_empty() || p.name.to_lowercase().contains(&needle) || p.cmd.to_lowercase().contains(&needle)
            }
        };
        self.common_kind_on(p.kind) && (!self.listening || !p.ports.is_empty()) && text
    }

    fn common_reindex(&mut self) {
        let keep = self.selected().map(|p| p.pid);
        let mut rows: Vec<usize> = (0..self.procs.len())
            .filter(|&i| self.common_matches(&self.procs[i]))
            .collect();
        let procs = &self.procs;
        match self.sort {
            Sort::Cpu => rows.sort_by(|&a, &b| procs[b].cpu.total_cmp(&procs[a].cpu)),
            Sort::Mem => rows.sort_by_key(|&i| std::cmp::Reverse(procs[i].rss)),
            Sort::Pid => rows.sort_by_key(|&i| procs[i].pid),
            Sort::Name => rows.sort_by(|&a, &b| procs[a].name.cmp(&procs[b].name)),
        }
        self.rows = rows;
        let at = keep.and_then(|pid| self.rows.iter().position(|&i| self.procs[i].pid == pid));
        self.table.select(at.or(Some(0)).filter(|_| !self.rows.is_empty()));
    }

    fn key_procs(&mut self, code: KeyCode) -> Effect {
        match code {
            KeyCode::Char(c @ '1'..='8') => {
                let i = (c as usize) - ('1' as usize);
                self.kinds[i] = !self.kinds[i];
                self.common_reindex();
                Effect::None
            }
            KeyCode::Char('a') => {
                let all = self.kinds.iter().all(|&on| on);
                self.kinds = [!all; Kind::ALL.len()];
                self.common_reindex();
                Effect::None
            }
            KeyCode::Char('m') => {
                self.kinds = Kind::ALL.map(|k| Kind::MINE.contains(&k));
                self.common_reindex();
                Effect::None
            }
            KeyCode::Char('p') => {
                self.listening = !self.listening;
                self.common_reindex();
                Effect::None
            }
            KeyCode::Char('s') => {
                self.sort = self.sort.next();
                self.common_reindex();
                Effect::None
            }
            KeyCode::Char('/') => {
                self.typing = true;
                Effect::None
            }
            KeyCode::Enter => {
                self.open = !self.open;
                self.history.clear();
                Effect::Watch(self.open.then(|| self.selected().map(|p| p.pid)).flatten())
            }
            KeyCode::Char('t') => self.key_procs_signal(Sig::Term),
            KeyCode::Char('K') => self.key_procs_signal(Sig::Kill),
            other => self.common_move(other, Tab::Procs),
        }
    }

    fn key_procs_signal(&mut self, sig: Sig) -> Effect {
        self.pending = self.selected().map(|p| Pending {
            label: format!(
                "{} {} ({})?",
                if sig == Sig::Kill { "SIGKILL" } else { "SIGTERM" },
                p.pid,
                p.name
            ),
            effect: Effect::Signal(p.pid, sig),
        });
        Effect::None
    }

    fn key_compose(&mut self, code: KeyCode) -> Effect {
        let Some(c) = self.container().cloned() else {
            return self.common_move(code, Tab::Compose);
        };
        let ask = |verb: &str, action: Action| Pending {
            label: format!("{verb} {}?", c.name),
            effect: Effect::Docker(c.id.clone(), action),
        };
        match code {
            KeyCode::Char('x') => self.pending = Some(ask("stop", Action::Stop)),
            KeyCode::Char('r') => self.pending = Some(ask("restart", Action::Restart)),
            KeyCode::Char('u') => return Effect::Docker(c.id, Action::Start),
            KeyCode::Char('l') => return Effect::Logs(c.id, c.name),
            other => return self.common_move(other, Tab::Compose),
        }
        Effect::None
    }

    fn key_typing(&mut self, code: KeyCode) -> Effect {
        match code {
            KeyCode::Char(c) => self.filter.push(c),
            KeyCode::Backspace => {
                self.filter.pop();
            }
            KeyCode::Esc => {
                self.filter.clear();
                self.typing = false;
            }
            KeyCode::Enter => self.typing = false,
            _ => {}
        }
        self.common_reindex();
        Effect::None
    }

    fn key_logs(&mut self, code: KeyCode) -> Effect {
        if let Some(logs) = self.logs.as_mut() {
            match code {
                KeyCode::Up | KeyCode::Char('k') => logs.scroll = logs.scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => logs.scroll = (logs.scroll + 1).min(logs.lines.len()),
                KeyCode::PageUp => logs.scroll = logs.scroll.saturating_sub(20),
                KeyCode::PageDown => logs.scroll = (logs.scroll + 20).min(logs.lines.len()),
                _ => self.logs = None,
            }
        }
        Effect::None
    }

    fn common_move(&mut self, code: KeyCode, tab: Tab) -> Effect {
        let (state, len) = match tab {
            Tab::Procs => (&mut self.table, self.rows.len()),
            Tab::Compose => (&mut self.boxes, self.compose.as_ref().map_or(0, Vec::len)),
        };
        let last = len.saturating_sub(1);
        let at = state.selected().unwrap_or(0);
        let to = match code {
            KeyCode::Down | KeyCode::Char('j') => (at + 1).min(last),
            KeyCode::Up | KeyCode::Char('k') => at.saturating_sub(1),
            KeyCode::PageDown => (at + 20).min(last),
            KeyCode::PageUp => at.saturating_sub(20),
            KeyCode::Home | KeyCode::Char('g') => 0,
            KeyCode::End | KeyCode::Char('G') => last,
            _ => return Effect::None,
        };
        state.select(Some(to));
        match tab {
            Tab::Procs if self.open => {
                self.history.clear();
                Effect::Watch(self.selected().map(|p| p.pid))
            }
            _ => Effect::None,
        }
    }

    // ############################################################

    #[must_use]
    pub fn selected(&self) -> Option<&Proc> {
        self.table
            .selected()
            .and_then(|i| self.rows.get(i))
            .map(|&i| &self.procs[i])
    }

    #[must_use]
    pub fn container(&self) -> Option<&Container> {
        self.compose
            .as_ref()
            .ok()
            .and_then(|all| self.boxes.selected().and_then(|i| all.get(i)))
    }

    /// How many processes of each kind the snapshot holds, in `Kind::ALL` order.
    #[must_use]
    pub fn counts(&self) -> [usize; Kind::ALL.len()] {
        Kind::ALL.map(|k| self.procs.iter().filter(|p| p.kind == k).count())
    }

    pub fn update(&mut self, procs: Vec<Proc>, compose: Result<Vec<Container>, String>, detail: Option<Detail>) {
        self.procs = procs;
        self.common_reindex();
        let len = compose.as_ref().map_or(0, Vec::len);
        self.compose = compose;
        self.boxes
            .select(self.boxes.selected().map(|i| i.min(len.saturating_sub(1))));
        let cpu = self.detail_proc().map(|p| p.cpu);
        self.detail = detail;
        if let Some(c) = cpu {
            // A sparkline wants integers: CPU is kept in tenths of a percent.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            self.history.push_back((c * 10.0) as u64);
            (self.history.len() > HISTORY).then(|| self.history.pop_front());
        }
    }

    /// The process the detail pane follows.
    #[must_use]
    pub fn detail_proc(&self) -> Option<&Proc> {
        self.detail
            .as_ref()
            .and_then(|d| self.procs.iter().find(|p| p.pid == d.pid))
    }

    pub fn key(&mut self, key: KeyEvent) -> Effect {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Effect::Quit,
            _ if self.logs.is_some() => self.key_logs(key.code),
            _ if self.pending.is_some() => {
                let pending = self.pending.take();
                pending
                    .filter(|_| key.code == KeyCode::Char('y'))
                    .map_or(Effect::None, |p| p.effect)
            }
            _ if self.typing => self.key_typing(key.code),
            (KeyCode::Char('q'), _) => Effect::Quit,
            (KeyCode::Esc, _) if self.open => {
                self.open = false;
                Effect::Watch(None)
            }
            (KeyCode::Tab, _) => {
                self.tab = match self.tab {
                    Tab::Procs => Tab::Compose,
                    Tab::Compose => Tab::Procs,
                };
                Effect::None
            }
            (code, _) => match self.tab {
                Tab::Procs => self.key_procs(code),
                Tab::Compose => self.key_compose(code),
            },
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

// ##### TESTS #####

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use super::{App, Effect};
    use crate::model::{Kind, Port, Proc, Proto};
    use crate::providers::signal::Sig;

    fn proc(pid: i32, kind: Kind, cpu: f32, port: Option<u16>) -> Proc {
        Proc {
            pid,
            name: format!("p{pid}"),
            cmd: format!("cmd {pid}"),
            kind,
            chain: vec![(pid, format!("p{pid}"))],
            cgroup: String::new(),
            ports: port
                .map(|p| Port {
                    proto: Proto::Tcp,
                    addr: SocketAddr::from(([127, 0, 0, 1], p)),
                })
                .into_iter()
                .collect(),
            cpu,
            rss: 0,
            threads: 1,
            uptime: 0,
            vram: None,
        }
    }

    fn press(app: &mut App, code: KeyCode) -> Effect {
        app.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn loaded() -> App {
        let mut app = App::new();
        app.update(
            vec![
                proc(10, Kind::Terminal, 1.0, Some(8000)),
                proc(11, Kind::Agent, 5.0, None),
                proc(12, Kind::System, 9.0, Some(22)),
                proc(13, Kind::Desktop, 2.0, None),
            ],
            Ok(Vec::new()),
            None,
        );
        app
    }

    fn pids(app: &App) -> Vec<i32> {
        app.rows.iter().map(|&i| app.procs[i].pid).collect()
    }

    #[test]
    fn test_update_shows_mine_sorted_by_cpu() {
        assert_eq!(pids(&loaded()), [11, 10]);
    }

    #[test]
    fn test_key_toggles_kind_and_all() {
        let mut app = loaded();
        press(&mut app, KeyCode::Char('7'));
        assert_eq!(pids(&app), [12, 11, 10]);
        press(&mut app, KeyCode::Char('a'));
        assert_eq!(pids(&app), [12, 11, 13, 10]);
        press(&mut app, KeyCode::Char('m'));
        assert_eq!(pids(&app), [11, 10]);
    }

    #[test]
    fn test_key_listening_and_filter_by_port() {
        let mut app = loaded();
        press(&mut app, KeyCode::Char('a'));
        press(&mut app, KeyCode::Char('p'));
        assert_eq!(pids(&app), [12, 10]);
        press(&mut app, KeyCode::Char('/'));
        "8000".chars().for_each(|c| {
            press(&mut app, KeyCode::Char(c));
        });
        assert_eq!(pids(&app), [10]);
    }

    #[test]
    fn test_filter_number_never_matches_argv() {
        let mut app = loaded();
        app.procs[1].cmd = "driver.py 8000".to_owned();
        press(&mut app, KeyCode::Char('/'));
        "8000".chars().for_each(|c| {
            press(&mut app, KeyCode::Char(c));
        });
        assert_eq!(pids(&app), [10]);
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Char('/'));
        "driver".chars().for_each(|c| {
            press(&mut app, KeyCode::Char(c));
        });
        assert_eq!(pids(&app), [11]);
    }

    #[test]
    fn test_key_kill_asks_then_fires_on_yes_only() {
        let mut app = loaded();
        assert_eq!(press(&mut app, KeyCode::Char('K')), Effect::None);
        assert_eq!(press(&mut app, KeyCode::Char('n')), Effect::None);
        assert!(app.pending.is_none());
        press(&mut app, KeyCode::Char('t'));
        assert_eq!(press(&mut app, KeyCode::Char('y')), Effect::Signal(11, Sig::Term));
    }

    #[test]
    fn test_selection_follows_pid_across_updates() {
        let mut app = loaded();
        press(&mut app, KeyCode::Down);
        let procs = app.procs.iter().cloned().map(|mut p| {
            p.cpu = if p.pid == 10 { 50.0 } else { p.cpu };
            p
        });
        app.update(procs.collect(), Ok(Vec::new()), None);
        assert_eq!(app.selected().map(|p| p.pid), Some(10));
    }

    #[test]
    fn test_key_on_empty_table_is_harmless() {
        let mut app = App::new();
        assert_eq!(press(&mut app, KeyCode::Char('K')), Effect::None);
        assert_eq!(press(&mut app, KeyCode::Down), Effect::None);
        assert!(app.pending.is_none());
    }
}
