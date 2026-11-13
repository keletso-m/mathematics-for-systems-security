use anyhow::Result;
use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Paragraph, Wrap},
    DefaultTerminal, Frame,
};

use crate::{
    fhe::TokenStore,
    plan::{self, Plan},
    shares,
};

pub struct Session {
    pub redacted: String,
    pub plan: Plan,
    pub store: TokenStore,
    pub shares: Vec<Vec<u8>>,
    pub threshold: u8,
}

struct App {
    s: Session,
    approved: Vec<bool>,
    output: Vec<String>,
    status: String,
    done: bool,
}

impl App {
    fn execute(&mut self) {
        let picked: Vec<Vec<u8>> = self
            .s
            .shares
            .iter()
            .zip(&self.approved)
            .filter(|(_, a)| **a)
            .map(|(s, _)| s.clone())
            .collect();

        if picked.len() < self.s.threshold as usize {
            self.status = format!(
                "Refused: need {} approvals, have {}. Key cannot be rebuilt.",
                self.s.threshold,
                picked.len()
            );
            return;
        }

        // The key exists only inside this closure and is zeroized when it ends.
        let result = shares::combine(&picked, self.s.threshold)
            .and_then(|key| plan::execute(&self.s.plan, &self.s.store, &key));

        match result {
            Ok(lines) => {
                self.output = lines;
                self.done = true;
                self.status = "Executed locally. Key wiped from memory. Press q to quit.".into();
            }
            Err(e) => self.status = format!("Failed: {e}"),
        }
    }
}

pub fn run(session: Session) -> Result<()> {
    let n = session.shares.len();
    let mut app = App {
        approved: vec![false; n],
        s: session,
        output: vec![],
        status: "Approve share-holders, then press Enter.".into(),
        done: false,
    };
    let mut terminal = ratatui::init();
    let res = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    res
}

fn event_loop(t: &mut DefaultTerminal, app: &mut App) -> Result<()> {
    loop {
        t.draw(|f| draw(f, app))?;
        if let Event::Key(k) = event::read()? {
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match k.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char(c) if c.is_ascii_digit() && !app.done => {
                    let i = c.to_digit(10).unwrap() as usize;
                    if i >= 1 && i <= app.approved.len() {
                        app.approved[i - 1] = !app.approved[i - 1];
                    }
                }
                KeyCode::Enter if !app.done => app.execute(),
                _ => {}
            }
        }
    }
}

fn draw(f: &mut Frame, app: &App) {
    let [main, footer] = Layout::vertical([Constraint::Min(0), Constraint::Length(3)]).areas(f.area());
    let [left, mid, right] = Layout::horizontal([
        Constraint::Percentage(34),
        Constraint::Percentage(33),
        Constraint::Percentage(33),
    ])
    .areas(main);

    // LEFT: everything the provider can observe.
    let mut l = vec![
        Line::styled("Prompt sent:", Style::new().add_modifier(Modifier::BOLD)),
        Line::from(app.s.redacted.clone()),
        Line::from(""),
        Line::styled("Ciphertext held:", Style::new().add_modifier(Modifier::BOLD)),
    ];
    for (token, hex) in app.s.store.ciphertext_hex() {
        let short: String = hex.chars().take(28).collect();
        l.push(Line::from(format!("{token}")));
        l.push(Line::styled(format!("  {short}..."), Style::new().fg(Color::DarkGray)));
    }
    f.render_widget(
        Paragraph::new(l).wrap(Wrap { trim: false }).block(
            Block::bordered()
                .title(" PROVIDER SEES ")
                .border_style(Style::new().fg(Color::Red)),
        ),
        left,
    );

    // MIDDLE: the agent's plan (untrusted input).
    let mut m: Vec<Line> = vec![];
    for (i, step) in app.s.plan.steps.iter().enumerate() {
        m.push(Line::styled(
            format!("{}. {}", i + 1, step.tool),
            Style::new().add_modifier(Modifier::BOLD),
        ));
        for (k, v) in &step.args {
            m.push(Line::from(format!("   {k}: {v}")));
        }
    }
    f.render_widget(
        Paragraph::new(m).wrap(Wrap { trim: false }).block(
            Block::bordered()
                .title(" AGENT PLAN (untrusted) ")
                .border_style(Style::new().fg(Color::Yellow)),
        ),
        mid,
    );

    // RIGHT: local-only side.
    let have = app.approved.iter().filter(|a| **a).count();
    let mut r = vec![
        Line::styled(
            format!("Approvals: {have}/{} needed", app.s.threshold),
            Style::new().add_modifier(Modifier::BOLD),
        ),
    ];
    for (i, a) in app.approved.iter().enumerate() {
        let (mark, color) = if *a { ("x", Color::Green) } else { (" ", Color::Gray) };
        r.push(Line::styled(format!("[{mark}] holder {} (press {})", i + 1, i + 1), Style::new().fg(color)));
    }
    r.push(Line::from(""));
    if app.done {
        r.push(Line::styled("Local output:", Style::new().add_modifier(Modifier::BOLD)));
        for line in &app.output {
            r.push(Line::styled(line.clone(), Style::new().fg(Color::Green)));
        }
    } else {
        r.push(Line::styled("Master key: not in memory", Style::new().fg(Color::DarkGray)));
    }
    f.render_widget(
        Paragraph::new(r).wrap(Wrap { trim: false }).block(
            Block::bordered()
                .title(" LOCAL ONLY ")
                .border_style(Style::new().fg(Color::Green)),
        ),
        right,
    );

    f.render_widget(
        Paragraph::new(vec![
            Line::from(app.status.clone()),
            Line::styled("1-5 toggle approval | Enter execute | q quit", Style::new().fg(Color::DarkGray)),
        ])
        .block(Block::bordered()),
        footer,
    );
}