use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Cell, Clear, Paragraph, Row, Sparkline, Table, Wrap};

use crate::model::{Kind, Port, Proc, Proto};
use crate::tui::app::{App, Tab};

const SELECTED: Style = Style::new().bg(Color::DarkGray).add_modifier(Modifier::BOLD);
const MARK: &str = "▶ ";
/// Compose is not a kind of process but sits in the same menu: its own colour sets it apart.
const COMPOSE: Color = Color::LightRed;
/// Under this width the kinds pane gives its columns to the table and folds into the table title.
const NARROW: u16 = 100;
const KEYS_PROCS: &str = " ↑↓ move · 1-8 kind · 9 compose · a all · m mine · p ports · / filter · esc clear · s sort · ⏎ detail · t term · K kill · ⇥ compose · q quit ";
const KEYS_COMPOSE: &str =
    " ↑↓ move · 9 processes · 1-8 a kind · x stop · u start · r restart · l logs · ⇥ procs · q quit ";

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
    let containers = app
        .compose
        .as_ref()
        .map_or_else(|_| "—".to_owned(), |all| all.len().to_string());
    lines.push(Line::from(vec![
        Span::raw(format!("9 {} ", if app.tab == Tab::Compose { "[x]" } else { "[ ]" })),
        Span::styled(format!("{:<9}", "compose"), Style::new().fg(COMPOSE).bold()),
        Span::raw(format!("{containers:>4}")).dim(),
    ]));
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

fn procs(frame: &mut Frame, area: Rect, app: &mut App, narrow: bool) {
    // VRAM earns its column only on a machine where some process holds GPU memory.
    let gpu = app.procs.iter().any(|p| p.vram.is_some());
    let rows = app.rows.iter().map(|&i| {
        let p = &app.procs[i];
        let mut cells = vec![
            Cell::from(p.pid.to_string()),
            Cell::from(p.kind.label()).style(Style::new().fg(tint(p.kind))),
            Cell::from(p.name.clone()),
            Cell::from(format!("{:5.1}", p.cpu)),
            Cell::from(bytes(p.rss)),
            Cell::from(ports(&p.ports)).style(Style::new().fg(Color::LightGreen)),
            Cell::from(age(p.uptime)),
            Cell::from(origin(p)).dim(),
        ];
        gpu.then(|| cells.insert(5, Cell::from(p.vram.map(|m| bytes(m << 20)).unwrap_or_default())));
        Row::new(cells)
    });
    let mut widths = vec![
        Constraint::Length(8),
        Constraint::Length(9),
        Constraint::Length(16),
        Constraint::Length(6),
        Constraint::Length(7),
        Constraint::Length(14),
        Constraint::Length(5),
        Constraint::Min(10),
    ];
    let mut header = vec!["PID", "KIND", "NAME", "CPU%", "MEM", "PORTS", "UP", "ORIGIN"];
    if gpu {
        widths.insert(5, Constraint::Length(7));
        header.insert(5, "VRAM");
    }
    let shown: Vec<&str> = Kind::ALL
        .iter()
        .zip(app.kinds)
        .filter(|&(_, on)| on)
        .map(|(k, _)| k.label())
        .collect();
    let kinds = if narrow {
        format!("· {} ", shown.join(" "))
    } else {
        String::new()
    };
    let title = format!(" processes {}/{} {kinds}", app.rows.len(), app.procs.len());
    let table = Table::new(rows, widths)
        .header(Row::new(header).bold())
        .row_highlight_style(SELECTED)
        .highlight_symbol(MARK)
        .block(Block::bordered().title(title));
    app.areas.table = area;
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
    let block = Block::bordered().border_style(Style::new().fg(COMPOSE));
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
            Cell::from(c.project.clone().unwrap_or_else(|| "—".to_owned()))
                .bold()
                .fg(COMPOSE),
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
        .highlight_symbol(MARK)
        .block(block.title(format!(" compose {} ", all.len())));
    app.areas.table = area;
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
    let tab = |t: Tab, label: &'static str| match (app.tab == t, t) {
        (true, Tab::Compose) => Span::raw(label).reversed().bold().fg(COMPOSE),
        (true, Tab::Procs) => Span::raw(label).reversed().bold(),
        (false, _) => Span::raw(label).dim(),
    };
    let labels = [
        Span::raw(" procs ").bold(),
        tab(Tab::Procs, " processes "),
        Span::raw(" "),
        tab(Tab::Compose, " compose "),
    ];
    // Each tab's clickable span, left to right after the title.
    let spans = labels.each_ref().map(|s| u16::try_from(s.width()).unwrap_or(0));
    let at = |i: usize| Rect::new(head.x + spans[..i].iter().sum::<u16>(), head.y, spans[i], 1);
    app.areas.procs = at(1);
    app.areas.compose = at(3);
    frame.render_widget(Line::from(labels.to_vec()), head);
    app.areas.kinds = Rect::default();
    // One menu for both views: the kinds pane stays beside compose too.
    let narrow = body.width < NARROW;
    let main = if narrow {
        body
    } else {
        let [side, main] = Layout::horizontal([Constraint::Length(24), Constraint::Min(0)]).areas(body);
        app.areas.kinds = side;
        kinds(frame, side, app);
        main
    };
    match app.tab {
        Tab::Procs => {
            if app.open {
                let [top, bottom] =
                    Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(main);
                procs(frame, top, app, narrow);
                detail(frame, bottom, app);
            } else {
                procs(frame, main, app, narrow);
            }
        }
        Tab::Compose => compose(frame, main, app),
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
