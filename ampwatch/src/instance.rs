use std::collections::HashMap;
use std::process::Command;

use crate::agent::{Agent, AgentType};
use crate::pr::PullRequest;

/// An amptown instance (one per repository)
#[derive(Clone)]
pub struct Instance {
    pub id: String,
    pub repo_path: Option<String>,
    pub logs_dir: Option<String>,
    pub agents: Vec<Agent>,
    pub open_prs: Vec<PullRequest>,
    pub closed_prs: Vec<PullRequest>,
}

impl Instance {
    pub fn new(id: String) -> Self {
        Self {
            id: id.clone(),
            repo_path: None,
            logs_dir: None,
            agents: vec![
                Agent::new("reviewer-alpha", AgentType::Reviewer, id.clone()),
                Agent::new("reviewer-beta", AgentType::Reviewer, id.clone()),
                Agent::new("reviewer-gamma", AgentType::Reviewer, id.clone()),
                Agent::new("impl-alpha", AgentType::Implementer, id.clone()),
                Agent::new("impl-beta", AgentType::Implementer, id.clone()),
                Agent::new("impl-gamma", AgentType::Implementer, id.clone()),
            ],
            open_prs: Vec::new(),
            closed_prs: Vec::new(),
        }
    }

    pub fn refresh(&mut self) {
        self.find_repo_path();
        self.refresh_agents();
        self.refresh_prs();
    }

    fn find_repo_path(&mut self) {
        // Get repo path from any running agent's tmux session
        for agent in &self.agents {
            let session_name = format!("amptown-{}-{}", self.id, agent.name);
            let output = Command::new("tmux")
                .args([
                    "display-message",
                    "-t",
                    &session_name,
                    "-p",
                    "#{pane_current_path}",
                ])
                .output();

            if let Ok(output) = output {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        self.repo_path = Some(path);
                        return;
                    }
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
        let Some(repo_path) = &self.repo_path else {
            return;
        };

        // Get open PRs - force fresh data with --no-cache if available
        if let Ok(output) = Command::new("gh")
            .args([
                "pr",
                "list",
                "--json",
                "number,title,state,author,createdAt,headRefName",
            ])
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
            .args([
                "pr",
                "list",
                "--state",
                "merged",
                "--limit",
                "10",
                "--json",
                "number,title,state,author,createdAt,headRefName",
            ])
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

    pub fn running_agent_count(&self) -> usize {
        self.agents.iter().filter(|a| a.is_running).count()
    }

    pub fn repo_name(&self) -> String {
        self.repo_path
            .as_ref()
            .and_then(|p| p.rsplit('/').next())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("instance-{}", self.id))
    }
}

/// Discover all running amptown instances by scanning tmux sessions
pub fn discover_instances() -> HashMap<String, Instance> {
    let mut instances: HashMap<String, Instance> = HashMap::new();

    // List all tmux sessions
    let output = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let sessions = String::from_utf8_lossy(&output.stdout);
            for session in sessions.lines() {
                // Match pattern: amptown-{instance_id}-{agent_name}
                if let Some(rest) = session.strip_prefix("amptown-") {
                    // Extract instance ID (8 hex chars)
                    if rest.len() > 9 && rest.chars().nth(8) == Some('-') {
                        let instance_id = &rest[..8];
                        if instance_id.chars().all(|c| c.is_ascii_hexdigit()) {
                            instances
                                .entry(instance_id.to_string())
                                .or_insert_with(|| Instance::new(instance_id.to_string()));
                        }
                    }
                }
            }
        }
    }

    // Also check for log directories to find instances that might have stopped
    discover_from_logs(&mut instances);

    instances
}

fn discover_from_logs(instances: &mut HashMap<String, Instance>) {
    let mut patterns: Vec<String> = vec!["/tmp/amptown-*/logs".to_string()];

    if let Ok(tmpdir) = std::env::var("TMPDIR") {
        let tmpdir = tmpdir.trim_end_matches('/');
        patterns.insert(0, format!("{}/amptown-*/logs", tmpdir));
    }

    patterns.push("/var/folders/*/*/*/*/amptown-*/logs".to_string());

    for pattern in &patterns {
        if let Ok(paths) = glob::glob(pattern) {
            for path in paths.flatten() {
                if path.is_dir() {
                    // Extract instance ID from path like /tmp/amptown-abc12345/logs
                    if let Some(parent) = path.parent() {
                        if let Some(dir_name) = parent.file_name() {
                            let dir_str = dir_name.to_string_lossy();
                            if let Some(id) = dir_str.strip_prefix("amptown-") {
                                if id.len() >= 6 {
                                    let instance = instances
                                        .entry(id.to_string())
                                        .or_insert_with(|| Instance::new(id.to_string()));
                                    instance.logs_dir = Some(path.to_string_lossy().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
