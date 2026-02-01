use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use std::{
    io,
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

mod agent;
mod instance;
mod pr;

use agent::AgentType;
use instance::{discover_instances, Instance};
use pr::PullRequest;

struct App {
    instances: Vec<Instance>,
    selected_instance: usize,

    // UI state
    selected_tab: usize,  // 0: Agents, 1: Open PRs, 2: Merged PRs
    pr_list_state: ListState,
    agent_list_state: ListState,
    instance_list_state: ListState,

    // Modal state
    show_modal: bool,
    modal_content: Arc<Mutex<String>>,
    modal_loading: Arc<Mutex<bool>>,

    // Refresh
    last_refresh: Instant,

    // Live indicator
    tick: usize,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            instances: Vec::new(),
            selected_instance: 0,
            selected_tab: 0,
            pr_list_state: ListState::default(),
            agent_list_state: ListState::default(),
            instance_list_state: ListState::default(),
            show_modal: false,
            modal_content: Arc::new(Mutex::new(String::new())),
            modal_loading: Arc::new(Mutex::new(false)),
            last_refresh: Instant::now(),
            tick: 0,
        };
        app.instance_list_state.select(Some(0));
        app.agent_list_state.select(Some(0));
        app
    }

    fn refresh(&mut self) {
        // Discover all running instances
        let discovered = discover_instances();
        
        // Convert to vec and sort by repo name for stable ordering
        let mut instances: Vec<Instance> = discovered.into_values().collect();
        instances.sort_by_key(|a| a.repo_name());
        
        // Refresh each instance's data
        for instance in &mut instances {
            instance.refresh();
        }
        
        self.instances = instances;
        
        // Ensure selected instance is valid
        if self.selected_instance >= self.instances.len() {
            self.selected_instance = self.instances.len().saturating_sub(1);
        }
        
        self.last_refresh = Instant::now();
    }

    fn current_instance(&self) -> Option<&Instance> {
        self.instances.get(self.selected_instance)
    }

    fn selected_pr(&self) -> Option<&PullRequest> {
        let instance = self.current_instance()?;
        let idx = self.pr_list_state.selected()?;
        if self.selected_tab == 1 {
            instance.open_prs.get(idx)
        } else if self.selected_tab == 2 {
            instance.closed_prs.get(idx)
        } else {
            None
        }
    }

    fn summarize_pr(&mut self) {
        let pr_number = match self.selected_pr() {
            Some(pr) => pr.number,
            None => return,
        };
        let repo_path = match self.current_instance().and_then(|i| i.repo_path.clone()) {
            Some(p) => p,
            None => return,
        };

        self.show_modal = true;
        *self.modal_loading.lock().unwrap() = true;
        *self.modal_content.lock().unwrap() = format!(
            "Loading summary for PR #{}...\n\nPlease wait, amp is analyzing the PR.",
            pr_number
        );

        let repo = repo_path;
        let content = Arc::clone(&self.modal_content);
        let loading = Arc::clone(&self.modal_loading);

        thread::spawn(move || {
            let output = Command::new("amp")
                .args([
                    "--dangerously-allow-all",
                    "--no-ide",
                    "-x",
                    &format!(
                        "Summarize PR #{} in this repository. Include: what changed, why, and any concerns. Be concise.",
                        pr_number
                    ),
                ])
                .current_dir(&repo)
                .output();

            let result = match output {
                Ok(out) if out.status.success() => {
                    String::from_utf8_lossy(&out.stdout).to_string()
                }
                Ok(out) => {
                    format!(
                        "Error summarizing PR:\n{}",
                        String::from_utf8_lossy(&out.stderr)
                    )
                }
                Err(e) => {
                    format!("Failed to run amp: {}", e)
                }
            };

            *content.lock().unwrap() = result;
            *loading.lock().unwrap() = false;
        });
    }

    fn next_tab(&mut self) {
        self.selected_tab = (self.selected_tab + 1) % 3;
        self.pr_list_state.select(Some(0));
    }

    fn prev_tab(&mut self) {
        self.selected_tab = if self.selected_tab == 0 {
            2
        } else {
            self.selected_tab - 1
        };
        self.pr_list_state.select(Some(0));
    }

    fn next_instance(&mut self) {
        if !self.instances.is_empty() {
            self.selected_instance = (self.selected_instance + 1) % self.instances.len();
            self.instance_list_state.select(Some(self.selected_instance));
            self.pr_list_state.select(Some(0));
        }
    }

    fn prev_instance(&mut self) {
        if !self.instances.is_empty() {
            self.selected_instance = if self.selected_instance == 0 {
                self.instances.len() - 1
            } else {
                self.selected_instance - 1
            };
            self.instance_list_state.select(Some(self.selected_instance));
            self.pr_list_state.select(Some(0));
        }
    }

    fn next_item(&mut self) {
        let len = match self.selected_tab {
            0 => self.current_instance().map(|i| i.agents.len()).unwrap_or(0),
            1 => self.current_instance().map(|i| i.open_prs.len()).unwrap_or(0),
            2 => self.current_instance().map(|i| i.closed_prs.len()).unwrap_or(0),
            _ => 0,
        };
        if len > 0 {
            let state = if self.selected_tab == 0 {
                &mut self.agent_list_state
            } else {
                &mut self.pr_list_state
            };
            let i = state.selected().unwrap_or(0);
            state.select(Some((i + 1) % len));
        }
    }

    fn prev_item(&mut self) {
        let len = match self.selected_tab {
            0 => self.current_instance().map(|i| i.agents.len()).unwrap_or(0),
            1 => self.current_instance().map(|i| i.open_prs.len()).unwrap_or(0),
            2 => self.current_instance().map(|i| i.closed_prs.len()).unwrap_or(0),
            _ => 0,
        };
        if len > 0 {
            let state = if self.selected_tab == 0 {
                &mut self.agent_list_state
            } else {
                &mut self.pr_list_state
            };
            let i = state.selected().unwrap_or(0);
            state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
        }
    }
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.refresh();

    let tick_rate = Duration::from_millis(200);
    let refresh_rate = Duration::from_secs(5);
    let mut last_tick = Instant::now();
    let mut last_refresh = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if app.show_modal {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                                app.show_modal = false;
                            }
                            _ => {}
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') => break,
                            KeyCode::Tab => app.next_tab(),
                            KeyCode::BackTab => app.prev_tab(),
                            KeyCode::Down | KeyCode::Char('j') => app.next_item(),
                            KeyCode::Up | KeyCode::Char('k') => app.prev_item(),
                            KeyCode::Right | KeyCode::Char('l') => app.next_instance(),
                            KeyCode::Left | KeyCode::Char('h') => app.prev_instance(),
                            KeyCode::Enter => {
                                if app.selected_tab > 0 {
                                    app.summarize_pr();
                                }
                            }
                            KeyCode::Char('r') => app.refresh(),
                            _ => {}
                        }
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick = app.tick.wrapping_add(1);
            last_tick = Instant::now();
        }

        if last_refresh.elapsed() >= refresh_rate {
            app.refresh();
            last_refresh = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Instance selector
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Animated spinner frames
    const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = SPINNER_FRAMES[app.tick % SPINNER_FRAMES.len()];

    // Get current instance info for header
    let (open_count, merged_count) = app
        .current_instance()
        .map(|i| (i.open_prs.len(), i.closed_prs.len()))
        .unwrap_or((0, 0));

    // Header
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled(format!(" {} ", spinner), Style::default().fg(Color::Green)),
        Span::styled(
            "AMPWATCH ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "LIVE",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!(
                " Agents {} ",
                if app.selected_tab == 0 { "●" } else { "○" }
            ),
            if app.selected_tab == 0 {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            },
        ),
        Span::styled(
            format!(
                " Open PRs ({}) {} ",
                open_count,
                if app.selected_tab == 1 { "●" } else { "○" }
            ),
            if app.selected_tab == 1 {
                Style::default().fg(Color::Green)
            } else {
                Style::default()
            },
        ),
        Span::styled(
            format!(
                " Merged PRs ({}) {} ",
                merged_count,
                if app.selected_tab == 2 { "●" } else { "○" }
            ),
            if app.selected_tab == 2 {
                Style::default().fg(Color::Magenta)
            } else {
                Style::default()
            },
        ),
    ])])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // Instance selector
    render_instance_selector(f, app, chunks[1]);

    // Content
    if app.instances.is_empty() {
        let empty = Paragraph::new("No amptown instances found. Start one with: amptown <repo-path>")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" No Instances "));
        f.render_widget(empty, chunks[2]);
    } else if let Some(instance) = app.instances.get(app.selected_instance) {
        match app.selected_tab {
            0 => render_agents(f, instance, &mut app.agent_list_state, chunks[2]),
            1 => {
                let prs = instance.open_prs.clone();
                render_prs(f, &prs, &mut app.pr_list_state, chunks[2], "Open Pull Requests");
            }
            2 => {
                let prs = instance.closed_prs.clone();
                render_prs(
                    f,
                    &prs,
                    &mut app.pr_list_state,
                    chunks[2],
                    "Merged Pull Requests",
                );
            }
            _ => {}
        }
    }

    // Footer
    let footer_text = if app.instances.len() > 1 {
        "q: Quit │ Tab: View │ ←→: Instance │ ↑↓: Navigate │ Enter: Summarize │ r: Refresh"
    } else if app.selected_tab == 0 {
        "q: Quit │ Tab: Switch view │ ↑↓: Navigate │ r: Refresh"
    } else {
        "q: Quit │ Tab: Switch view │ ↑↓: Navigate │ Enter: Summarize PR │ r: Refresh"
    };
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[3]);

    // Modal
    if app.show_modal {
        render_modal(f, app);
    }
}

fn render_instance_selector(f: &mut Frame, app: &App, area: Rect) {
    if app.instances.is_empty() {
        let empty = Paragraph::new("No instances running")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title(" Instances "));
        f.render_widget(empty, area);
        return;
    }

    let titles: Vec<Line> = app
        .instances
        .iter()
        .enumerate()
        .map(|(i, inst)| {
            let running = inst.running_agent_count();
            let style = if i == app.selected_instance {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::styled(format!(" {} ({}/6) ", inst.repo_name(), running), style)
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Instances ({}) ", app.instances.len())),
        )
        .select(app.selected_instance)
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

fn render_agents(f: &mut Frame, instance: &Instance, _list_state: &mut ListState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Reviewers
    let reviewers: Vec<ListItem> = instance
        .agents
        .iter()
        .filter(|a| matches!(a.agent_type, AgentType::Reviewer))
        .map(|a| {
            let status_color = if a.is_running {
                Color::Green
            } else {
                Color::Red
            };
            let status_icon = if a.is_running { "●" } else { "○" };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(&a.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" (iter: {})", a.iterations)),
            ]))
        })
        .collect();

    let reviewers_list = List::new(reviewers).block(
        Block::default()
            .title(" Reviewers ")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Blue)),
    );
    f.render_widget(reviewers_list, chunks[0]);

    // Implementers
    let implementers: Vec<ListItem> = instance
        .agents
        .iter()
        .filter(|a| matches!(a.agent_type, AgentType::Implementer))
        .map(|a| {
            let status_color = if a.is_running {
                Color::Green
            } else {
                Color::Red
            };
            let status_icon = if a.is_running { "●" } else { "○" };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(&a.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" (iter: {})", a.iterations)),
            ]))
        })
        .collect();

    let implementers_list = List::new(implementers).block(
        Block::default()
            .title(" Implementers ")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Magenta)),
    );
    f.render_widget(implementers_list, chunks[1]);
}

fn render_prs(
    f: &mut Frame,
    prs: &[PullRequest],
    list_state: &mut ListState,
    area: Rect,
    title: &str,
) {
    let items: Vec<ListItem> = prs
        .iter()
        .map(|pr| {
            let state_color = match pr.state.as_str() {
                "OPEN" => Color::Green,
                "MERGED" => Color::Magenta,
                "CLOSED" => Color::Red,
                _ => Color::White,
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("#{:<4} ", pr.number),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(format!("{:<8} ", pr.state), Style::default().fg(state_color)),
                Span::raw(&pr.title),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" {} ", title))
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .fg(Color::Yellow),
        );

    f.render_stateful_widget(list, area, list_state);
}

fn render_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(80, 60, f.area());

    f.render_widget(Clear, area);

    let is_loading = *app.modal_loading.lock().unwrap();
    let content = app.modal_content.lock().unwrap().clone();

    let title = if is_loading {
        " Loading... (Press Esc to cancel) "
    } else {
        " PR Summary (Press Esc to close) "
    };

    let modal = Paragraph::new(content).wrap(Wrap { trim: true }).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::DarkGray)),
    );

    f.render_widget(modal, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
