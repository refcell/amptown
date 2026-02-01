use std::fs;
use std::process::Command;

#[derive(Clone)]
pub enum AgentType {
    Reviewer,
    Implementer,
}

#[derive(Clone)]
pub struct Agent {
    pub name: String,
    pub agent_type: AgentType,
    pub instance_id: String,
    pub is_running: bool,
    pub iterations: u32,
    pub last_activity: String,
}

impl Agent {
    pub fn new(name: &str, agent_type: AgentType, instance_id: String) -> Self {
        Self {
            name: name.to_string(),
            agent_type,
            instance_id,
            is_running: false,
            iterations: 0,
            last_activity: String::new(),
        }
    }

    pub fn session_name(&self) -> String {
        format!("amptown-{}-{}", self.instance_id, self.name)
    }

    pub fn refresh(&mut self, logs_dir: &Option<String>) {
        self.check_running();
        if let Some(dir) = logs_dir {
            self.read_log(dir);
        }
    }

    fn check_running(&mut self) {
        let session_name = self.session_name();

        let output = Command::new("tmux")
            .args(["has-session", "-t", &session_name])
            .output();

        self.is_running = output.map(|o| o.status.success()).unwrap_or(false);
    }

    fn read_log(&mut self, logs_dir: &str) {
        let log_path = format!("{}/{}.log", logs_dir, self.name);

        if let Ok(content) = fs::read_to_string(&log_path) {
            // Count iterations
            self.iterations = content.matches("Starting").count() as u32;

            // Get last meaningful line
            let lines: Vec<&str> = content.lines().collect();
            for line in lines.iter().rev() {
                if !line.starts_with('[') && !line.trim().is_empty() {
                    self.last_activity = line.chars().take(80).collect();
                    break;
                }
            }
        }
    }
}
