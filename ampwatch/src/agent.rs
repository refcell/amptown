use std::process::Command;
use std::fs;

#[derive(Clone)]
pub enum AgentType {
    Reviewer,
    Implementer,
}

#[derive(Clone)]
pub struct Agent {
    pub name: String,
    pub agent_type: AgentType,
    pub is_running: bool,
    pub iterations: u32,
    pub last_activity: String,
}

impl Agent {
    pub fn new(name: &str, agent_type: AgentType) -> Self {
        Self {
            name: name.to_string(),
            agent_type,
            is_running: false,
            iterations: 0,
            last_activity: String::new(),
        }
    }
    
    pub fn refresh(&mut self, logs_dir: &Option<String>) {
        self.check_running();
        if let Some(dir) = logs_dir {
            self.read_log(dir);
        }
    }
    
    fn check_running(&mut self) {
        let session_name = format!("amptown-{}", self.name);
        
        // Check if tmux session exists
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
