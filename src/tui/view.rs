use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Sparkline, Table, Wrap};

use crate::model::{Kind, Port, Proc, Proto};
use crate::tui::app::{App, Tab};

const SELECTED: Style = Style::new().bg(Color::DarkGray).add_modifier(Modifier::BOLD);
const KEYS_PROCS: &str =
    " 1-8 kind · a all · m mine · p ports · / filter · s sort · ⏎ detail · t term · K kill · ⇥ compose · q quit ";
const KEYS_COMPOSE: &str = " x stop · u start · r restart · l logs · ⇥ procs · q quit ";

// ##### FORMAT #####

#[allow(clippy::cast_precision_loss)]
fn bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let exp = (0..UNITS.len()).rev().find(|&e| n >= 1u64 << (10 * e)).unwrap_or(0);
    let value = n as f64 / (1u64 << (10 * exp)) as f64;
    format!("{value:.1}{}", UNITS[exp])
}

fn age(secs: u64) -> String {
    match secs {
        s if s < 3600 => format!("{}m", s / 60),
        s if s < 86_400 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86_400),
    }
}

fn ports(all: &[Port]) -> String {
    all.iter()
        .map(|p| match p.proto {
            Proto::Tcp => p.addr.port().to_string(),
            Proto::Udp => format!("{}/u", p.addr.port()),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn origin(p: &Proc) -> String {
    p.chain
        .iter()
        .skip(1)
        .map(|(_, n)| n.as_str())
        .collect::<Vec<_>>()
        .join(" ← ")
}

const fn tint(kind: Kind) -> Color {
    match kind {
        Kind::Agent => Color::Magenta,
        Kind::Ide => Color::Blue,
        Kind::Terminal => Color::Green,
        Kind::Service => Color::Cyan,
        Kind::Desktop => Color::Yellow,
        Kind::Container => Color::LightBlue,
        Kind::System => Color::Gray,
        Kind::Kernel => Color::DarkGray,
    }
}

// ##### PANES #####

fn kinds(frame: &mut Frame, area: Rect, app: &App) {
    let counts = app.counts();
    let mut lines: Vec<Line> = Kind::ALL
        .iter()
        .enumerate()
        .map(|(i, &k)| {
            let mark = if app.kinds[i] { "[x]" } else { "[ ]" };
            Line::from(vec![
                Span::raw(format!("{} {mark} ", i + 1)),
                Span::styled(format!("{:<9}", k.label()), Style::new().fg(tint(k))),
                Span::raw(format!("{:>4}", counts[i])).dim(),
            ])
        })
        .collect();
    lines.push(Line::raw(""));
    lines.push(Line::raw(format!(
        "p {} ports only",
        if app.listening { "[x]" } else { "[ ]" }
    )));
    lines.push(Line::raw(format!("s sort: {}", app.sort.label())).dim());
    let filter = match (app.typing, app.filter.is_empty()) {
        (true, _) => format!("/{}▏", app.filter),
        (false, true) => String::new(),
        (false, false) => format!("/{}", app.filter),
    };
    lines.push(Line::raw(filter).yellow());
    frame.render_widget(Paragraph::new(lines).block(Block::bordered().title(" kinds ")), area);
}

fn procs(frame: &mut Frame, area: Rect, app: &mut App) {
    let rows = app.rows.iter().map(|&i| {
        let p = &app.procs[i];
        Row::new(vec![
            Cell::from(p.pid.to_string()),
            Cell::from(p.kind.label()).style(Style::new().fg(tint(p.kind))),
            Cell::from(p.name.clone()),
            Cell::from(format!("{:5.1}", p.cpu)),
            Cell::from(bytes(p.rss)),
            Cell::from(p.vram.map(|m| bytes(m << 20)).unwrap_or_default()),
            Cell::from(ports(&p.ports)).style(Style::new().fg(Color::LightGreen)),
            Cell::from(age(p.uptime)),
            Cell::from(origin(p)).dim(),
        ])
    });
    let widths = [
        Constraint::Length(8),
        Constraint::Length(9),
        Constraint::Length(16),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Length(14),
        Constraint::Length(5),
        Constraint::Min(10),
    ];
    let title = format!(" processes {}/{} ", app.rows.len(), app.procs.len());
    let table = Table::new(rows, widths)
        .header(Row::new(["PID", "KIND", "NAME", "CPU%", "MEM", "VRAM", "PORTS", "UP", "ORIGIN"]).bold())
        .row_highlight_style(SELECTED)
        .block(Block::bordered().title(title));
    frame.render_stateful_widget(table, area, &mut app.table);
}

fn detail(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::bordered().title(" detail ");
    let (Some(d), Some(p)) = (app.detail.as_ref(), app.detail_proc()) else {
        frame.render_widget(Paragraph::new("gone or loading…").block(block), area);
        return;
    };
    let opt = |v: Option<u64>| v.map_or_else(|| "—".to_owned(), bytes);
    let path = |v: Option<&std::path::PathBuf>| v.map_or_else(|| "—".to_owned(), |x| x.display().to_string());
    let lines = vec![
        Line::from(vec![
            Span::raw(format!("{} ", p.pid)).bold(),
            Span::raw(p.name.clone()).fg(tint(p.kind)),
        ]),
        Line::raw(format!("cmd   {}", p.cmd)),
        Line::raw(format!("exe   {}", path(d.exe.as_ref()))),
        Line::raw(format!("cwd   {}", path(d.cwd.as_ref()))),
        Line::raw(format!(
            "chain {}",
            p.chain
                .iter()
                .map(|(pid, n)| format!("{n}:{pid}"))
                .collect::<Vec<_>>()
                .join(" ← ")
        )),
        Line::raw(format!("unit  {}", p.cgroup)),
        Line::raw(format!(
            "cpu {:.1}%  rss {}  pss {}  swap {}  vram {}",
            p.cpu,
            bytes(p.rss),
            opt(d.pss),
            opt(d.swap),
            p.vram.map_or_else(|| "—".to_owned(), |m| bytes(m << 20))
        )),
        Line::raw(format!(
            "threads {}  fds {}  read {}  write {}  up {}",
            p.threads,
            d.fds.map_or_else(|| "—".to_owned(), |n| n.to_string()),
            opt(d.read),
            opt(d.write),
            age(p.uptime)
        )),
        Line::raw(format!("ports {}", ports(&p.ports))),
    ];
    let [text, spark] = Layout::vertical([Constraint::Min(0), Constraint::Length(5)]).areas(block.inner(area));
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), text);
    let history: Vec<u64> = app.history.iter().copied().collect();
    frame.render_widget(
        Sparkline::default()
            .data(&history)
            .style(Style::new().fg(Color::Magenta))
            .block(Block::new().title("cpu")),
        spark,
    );
}

fn compose(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::bordered();
    let all = match &app.compose {
        Ok(all) => all,
        Err(e) => {
            frame.render_widget(
                Paragraph::new(format!("docker unavailable: {e}"))
                    .red()
                    .block(block.title(" compose ")),
                area,
            );
            return;
        }
    };
    let rows = all.iter().map(|c| {
        let running = c.state == "running";
        Row::new(vec![
            Cell::from(c.project.clone().unwrap_or_else(|| "—".to_owned())).bold(),
            Cell::from(c.name.clone()),
            Cell::from(c.service.clone().unwrap_or_default()),
            Cell::from(c.state.clone()).style(Style::new().fg(if running { Color::Green } else { Color::Red })),
            Cell::from(c.status.clone()),
            Cell::from(c.ports.clone()).dim(),
        ])
    });
    let widths = [
        Constraint::Length(18),
        Constraint::Length(30),
        Constraint::Length(16),
        Constraint::Length(9),
        Constraint::Length(22),
        Constraint::Min(10),
    ];
    let table = Table::new(rows, widths)
        .header(Row::new(["PROJECT", "CONTAINER", "SERVICE", "STATE", "STATUS", "PORTS"]).bold())
        .row_highlight_style(SELECTED)
        .block(block.title(format!(" compose {} ", all.len())));
    frame.render_stateful_widget(table, area, &mut app.boxes);
}

fn popup(frame: &mut Frame, area: Rect, title: &str, body: Paragraph) {
    let [_, mid, _] = Layout::vertical([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
    ])
    .areas(area);
    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage(8),
        Constraint::Percentage(84),
        Constraint::Percentage(8),
    ])
    .areas(mid);
    frame.render_widget(Clear, center);
    frame.render_widget(body.block(Block::bordered().title(title.to_owned())), center);
}

// ##### ROOT #####

pub fn draw(frame: &mut Frame, app: &mut App) {
    let [head, body, foot] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());
    let tab = |t: Tab, label: &'static str| {
        if app.tab == t {
            Span::raw(label).reversed().bold()
        } else {
            Span::raw(label).dim()
        }
    };
    frame.render_widget(
        Line::from(vec![
            Span::raw(" procs ").bold(),
            tab(Tab::Procs, " processes "),
            Span::raw(" "),
            tab(Tab::Compose, " compose "),
        ]),
        head,
    );
    match app.tab {
        Tab::Procs => {
            let [side, main] = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(body);
            kinds(frame, side, app);
            if app.open {
                let [top, bottom] =
                    Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(main);
                procs(frame, top, app);
                detail(frame, bottom, app);
            } else {
                procs(frame, main, app);
            }
        }
        Tab::Compose => compose(frame, body, app),
    }
    let keys = match app.tab {
        Tab::Procs => KEYS_PROCS,
        Tab::Compose => KEYS_COMPOSE,
    };
    let foot_line = match (&app.pending, app.status.is_empty()) {
        (Some(p), _) => Line::raw(format!(" {}  y confirm · any key cancel ", p.label))
            .black()
            .on_yellow(),
        (None, false) => Line::raw(format!(" {} ", app.status)).yellow(),
        (None, true) => Line::raw(keys).dim(),
    };
    frame.render_widget(foot_line, foot);
    if let Some(logs) = &app.logs {
        let text: Vec<Line> = logs
            .lines
            .iter()
            .skip(logs.scroll)
            .map(|l| Line::raw(l.clone()))
            .collect();
        popup(
            frame,
            frame.area(),
            &format!(" logs {} · ↑↓ PgUp PgDn · any key closes ", logs.title),
            Paragraph::new(text),
        );
    }
}
