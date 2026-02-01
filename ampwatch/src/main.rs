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
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
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
mod pr;

use agent::{Agent, AgentType};
use pr::PullRequest;

struct App {
    agents: Vec<Agent>,
    open_prs: Vec<PullRequest>,
    closed_prs: Vec<PullRequest>,
    logs_dir: Option<String>,
    repo_path: Option<String>,
    
    // UI state
    selected_tab: usize,
    pr_list_state: ListState,
    agent_list_state: ListState,
    
    // Modal state
    show_modal: bool,
    modal_content: Arc<Mutex<String>>,
    modal_loading: Arc<Mutex<bool>>,
    
    // Refresh
    last_refresh: Instant,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            agents: vec![
                Agent::new("reviewer-alpha", AgentType::Reviewer),
                Agent::new("reviewer-beta", AgentType::Reviewer),
                Agent::new("reviewer-gamma", AgentType::Reviewer),
                Agent::new("impl-alpha", AgentType::Implementer),
                Agent::new("impl-beta", AgentType::Implementer),
                Agent::new("impl-gamma", AgentType::Implementer),
            ],
            open_prs: Vec::new(),
            closed_prs: Vec::new(),
            logs_dir: None,
            repo_path: None,
            selected_tab: 0,
            pr_list_state: ListState::default(),
            agent_list_state: ListState::default(),
            show_modal: false,
            modal_content: Arc::new(Mutex::new(String::new())),
            modal_loading: Arc::new(Mutex::new(false)),
            last_refresh: Instant::now(),
        };
        app.agent_list_state.select(Some(0));
        app
    }
    
    fn refresh(&mut self) {
        self.find_logs_dir();
        self.find_repo_path();
        self.refresh_agents();
        self.refresh_prs();
        self.last_refresh = Instant::now();
    }
    
    fn find_logs_dir(&mut self) {
        // Search for amptown logs directories
        let patterns = [
            "/var/folders/**/amptown-*/logs",
            "/tmp/amptown-*/logs",
        ];
        
        for pattern in patterns {
            if let Ok(paths) = glob::glob(pattern) {
                for path in paths.flatten() {
                    if path.is_dir() {
                        self.logs_dir = Some(path.to_string_lossy().to_string());
                        return;
                    }
                }
            }
        }
    }
    
    fn find_repo_path(&mut self) {
        // Get repo path from tmux session
        let output = Command::new("tmux")
            .args(["display-message", "-t", "amptown-impl-alpha", "-p", "#{pane_current_path}"])
            .output();
        
        if let Ok(output) = output {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    self.repo_path = Some(path);
                }
            }
        }
    }
    
    fn refresh_agents(&mut self) {
        for agent in &mut self.agents {
            agent.refresh(&self.logs_dir);
        }
    }
    
    fn refresh_prs(&mut self) {
        let Some(repo_path) = &self.repo_path else { return };
        
        // Get open PRs
        if let Ok(output) = Command::new("gh")
            .args(["pr", "list", "--json", "number,title,state,author,createdAt,headRefName"])
            .current_dir(repo_path)
            .output()
        {
            if output.status.success() {
                if let Ok(prs) = serde_json::from_slice::<Vec<PullRequest>>(&output.stdout) {
                    self.open_prs = prs;
                }
            }
        }
        
        // Get closed/merged PRs
        if let Ok(output) = Command::new("gh")
            .args(["pr", "list", "--state", "merged", "--limit", "10", "--json", "number,title,state,author,createdAt,headRefName"])
            .current_dir(repo_path)
            .output()
        {
            if output.status.success() {
                if let Ok(prs) = serde_json::from_slice::<Vec<PullRequest>>(&output.stdout) {
                    self.closed_prs = prs;
                }
            }
        }
    }
    
    fn selected_pr(&self) -> Option<&PullRequest> {
        let idx = self.pr_list_state.selected()?;
        if self.selected_tab == 1 {
            self.open_prs.get(idx)
        } else if self.selected_tab == 2 {
            self.closed_prs.get(idx)
        } else {
            None
        }
    }
    
    fn summarize_pr(&mut self) {
        let pr_number = match self.selected_pr() {
            Some(pr) => pr.number,
            None => return,
        };
        let Some(repo_path) = &self.repo_path else { return };
        
        self.show_modal = true;
        *self.modal_loading.lock().unwrap() = true;
        *self.modal_content.lock().unwrap() = format!("Loading summary for PR #{}...\n\nPlease wait, amp is analyzing the PR.", pr_number);
        
        let repo = repo_path.clone();
        let content = Arc::clone(&self.modal_content);
        let loading = Arc::clone(&self.modal_loading);
        
        // Run amp in a background thread
        thread::spawn(move || {
            let output = Command::new("amp")
                .args([
                    "--dangerously-allow-all",
                    "--no-ide",
                    "-x",
                    &format!("Summarize PR #{} in this repository. Include: what changed, why, and any concerns. Be concise.", pr_number),
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
        self.selected_tab = if self.selected_tab == 0 { 2 } else { self.selected_tab - 1 };
        self.pr_list_state.select(Some(0));
    }
    
    fn next_item(&mut self) {
        let len = match self.selected_tab {
            0 => self.agents.len(),
            1 => self.open_prs.len(),
            2 => self.closed_prs.len(),
            _ => 0,
        };
        
        if len == 0 { return; }
        
        let state = if self.selected_tab == 0 {
            &mut self.agent_list_state
        } else {
            &mut self.pr_list_state
        };
        
        let i = state.selected().unwrap_or(0);
        state.select(Some((i + 1) % len));
    }
    
    fn prev_item(&mut self) {
        let len = match self.selected_tab {
            0 => self.agents.len(),
            1 => self.open_prs.len(),
            2 => self.closed_prs.len(),
            _ => 0,
        };
        
        if len == 0 { return; }
        
        let state = if self.selected_tab == 0 {
            &mut self.agent_list_state
        } else {
            &mut self.pr_list_state
        };
        
        let i = state.selected().unwrap_or(0);
        state.select(Some(if i == 0 { len - 1 } else { i - 1 }));
    }
}

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    let mut app = App::new();
    app.refresh();
    
    let tick_rate = Duration::from_secs(5);
    let mut last_tick = Instant::now();
    
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
            app.refresh();
            last_tick = Instant::now();
        }
    }
    
    // Restore terminal
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
            Constraint::Length(3),  // Header
            Constraint::Min(0),      // Content
            Constraint::Length(3),  // Footer
        ])
        .split(f.area());
    
    // Header
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" AMPWATCH ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" │ "),
            Span::styled(
                format!(" Agents {} ", if app.selected_tab == 0 { "●" } else { "○" }),
                if app.selected_tab == 0 { Style::default().fg(Color::Yellow) } else { Style::default() }
            ),
            Span::styled(
                format!(" Open PRs ({}) {} ", app.open_prs.len(), if app.selected_tab == 1 { "●" } else { "○" }),
                if app.selected_tab == 1 { Style::default().fg(Color::Green) } else { Style::default() }
            ),
            Span::styled(
                format!(" Merged PRs ({}) {} ", app.closed_prs.len(), if app.selected_tab == 2 { "●" } else { "○" }),
                if app.selected_tab == 2 { Style::default().fg(Color::Magenta) } else { Style::default() }
            ),
        ]),
    ])
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);
    
    // Content
    match app.selected_tab {
        0 => render_agents(f, app, chunks[1]),
        1 => {
            let prs = app.open_prs.clone();
            render_prs(f, &prs, &mut app.pr_list_state, chunks[1], "Open Pull Requests");
        }
        2 => {
            let prs = app.closed_prs.clone();
            render_prs(f, &prs, &mut app.pr_list_state, chunks[1], "Merged Pull Requests");
        }
        _ => {}
    }
    
    // Footer
    let footer_text = if app.selected_tab == 0 {
        "q: Quit │ Tab: Switch view │ ↑↓: Navigate │ r: Refresh"
    } else {
        "q: Quit │ Tab: Switch view │ ↑↓: Navigate │ Enter: Summarize PR │ r: Refresh"
    };
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
    
    // Modal
    if app.show_modal {
        render_modal(f, app);
    }
}

fn render_agents(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    
    // Reviewers
    let reviewers: Vec<ListItem> = app.agents.iter()
        .filter(|a| matches!(a.agent_type, AgentType::Reviewer))
        .map(|a| {
            let status_color = if a.is_running { Color::Green } else { Color::Red };
            let status_icon = if a.is_running { "●" } else { "○" };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(&a.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" (iter: {})", a.iterations)),
            ]))
        })
        .collect();
    
    let reviewers_list = List::new(reviewers)
        .block(Block::default().title(" Reviewers ").borders(Borders::ALL).style(Style::default().fg(Color::Blue)))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(reviewers_list, chunks[0]);
    
    // Implementers
    let implementers: Vec<ListItem> = app.agents.iter()
        .filter(|a| matches!(a.agent_type, AgentType::Implementer))
        .map(|a| {
            let status_color = if a.is_running { Color::Green } else { Color::Red };
            let status_icon = if a.is_running { "●" } else { "○" };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
                Span::styled(&a.name, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" (iter: {})", a.iterations)),
            ]))
        })
        .collect();
    
    let implementers_list = List::new(implementers)
        .block(Block::default().title(" Implementers ").borders(Borders::ALL).style(Style::default().fg(Color::Magenta)))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_widget(implementers_list, chunks[1]);
}

fn render_prs(f: &mut Frame, prs: &[PullRequest], list_state: &mut ListState, area: Rect, title: &str) {
    let items: Vec<ListItem> = prs.iter()
        .map(|pr| {
            let state_color = match pr.state.as_str() {
                "OPEN" => Color::Green,
                "MERGED" => Color::Magenta,
                "CLOSED" => Color::Red,
                _ => Color::White,
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("#{:<4} ", pr.number), Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:<8} ", pr.state), Style::default().fg(state_color)),
                Span::raw(&pr.title),
            ]))
        })
        .collect();
    
    let list = List::new(items)
        .block(Block::default().title(format!(" {} ", title)).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED).fg(Color::Yellow));
    
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
    
    let modal = Paragraph::new(content)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .style(Style::default().bg(Color::DarkGray))
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
