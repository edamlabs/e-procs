#![forbid(unsafe_code)]

mod classify;
mod model;
mod providers;
mod tui;

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use ratatui::crossterm::execute;

use crate::model::{Container, Detail, Proc};
use crate::providers::docker::DockerDriver;
use crate::providers::gpu::GpuDriver;
use crate::providers::signal::SignalDriver;
use crate::providers::system::SystemDriver;
use crate::tui::app::{App, Effect, Logs};

const REFRESH: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(100);
/// CPU is a delta: a first sample this long before the first snapshot, or it shows every process at 0% and the
/// order jumps on the second.
const WARMUP: Duration = Duration::from_millis(250);

// ##### MESSAGES #####

/// From the UI to the collector.
enum Ask {
    Watch(Option<i32>),
    Now,
}

/// From the collector and the action workers to the UI.
enum Update {
    Snapshot(Vec<Proc>, Result<Vec<Container>, String>, Option<Detail>),
    Done(String),
    Logs(String, String),
}

// ##### COLLECTOR #####

/// Reads the world every `REFRESH`, or at once when asked; never on the UI thread.
fn collect(asks: &Receiver<Ask>, updates: &Sender<Update>) {
    let mut system = SystemDriver::new();
    let mut watch = None;
    system.run(&HashMap::new());
    thread::sleep(WARMUP);
    loop {
        let procs = system.run(&GpuDriver::run());
        let snapshot = Update::Snapshot(procs, DockerDriver::run(), watch.map(SystemDriver::detail));
        if updates.send(snapshot).is_err() {
            return;
        }
        match asks.recv_timeout(REFRESH) {
            Ok(Ask::Watch(pid)) => watch = pid,
            Ok(Ask::Now) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Carries out an effect off the UI thread and reports back.
fn act(effect: Effect, asks: &Sender<Ask>, updates: &Sender<Update>) {
    let updates = updates.clone();
    let asks = asks.clone();
    let job = move || {
        let update = match effect {
            Effect::Signal(pid, sig) => Update::Done(
                SignalDriver::run(pid, sig)
                    .map_or_else(|r| format!("{pid}: {}", r.label()), |()| format!("{pid}: {sig:?} sent")),
            ),
            Effect::Docker(id, action) => Update::Done(
                DockerDriver::act(&id, action)
                    .map_or_else(|e| format!("{action:?} failed: {e}"), |()| format!("{action:?} {id}")),
            ),
            Effect::Logs(id, name) => Update::Logs(name, DockerDriver::logs(&id).unwrap_or_else(|e| e)),
            Effect::None | Effect::Quit | Effect::Watch(_) => return,
        };
        let _ = updates.send(update);
        let _ = asks.send(Ask::Now);
    };
    thread::spawn(job);
}

// ##### LOOP #####

fn run(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    let (ask_tx, ask_rx) = mpsc::channel();
    let (up_tx, up_rx) = mpsc::channel();
    let collector_tx = up_tx.clone();
    thread::spawn(move || collect(&ask_rx, &collector_tx));
    let mut app = App::new();
    loop {
        for update in up_rx.try_iter() {
            match update {
                Update::Snapshot(procs, compose, detail) => app.update(procs, compose, detail),
                Update::Done(status) => app.status = status,
                Update::Logs(title, text) => {
                    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
                    let scroll = lines.len().saturating_sub(40);
                    app.logs = Some(Logs { title, lines, scroll });
                }
            }
        }
        terminal.draw(|frame| tui::view::draw(frame, &mut app))?;
        let effect = match event::poll(POLL)?.then(event::read).transpose()? {
            Some(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                app.status.clear();
                app.key(k)
            }
            Some(Event::Mouse(m)) => app.mouse(m),
            _ => continue,
        };
        match effect {
            Effect::Quit => return Ok(()),
            Effect::None => {}
            Effect::Watch(pid) => {
                let _ = ask_tx.send(Ask::Watch(pid));
            }
            other => act(other, &ask_tx, &up_tx),
        }
    }
}

/// Mouse reporting on for the life of the value; dropped on return and on panic, so the shell never inherits it.
struct Mouse;

impl Mouse {
    fn on() -> std::io::Result<Self> {
        execute!(std::io::stdout(), EnableMouseCapture).map(|()| Self)
    }
}

impl Drop for Mouse {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
    }
}

fn main() -> std::io::Result<()> {
    let _mouse = Mouse::on()?;
    ratatui::run(run)
}
