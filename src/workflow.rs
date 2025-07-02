use crate::models::AgentCommand;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStep>,
    pub triggers: Vec<WorkflowTrigger>,
    pub variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub name: String,
    pub command: String,
    pub condition: Option<String>,
    pub retry_count: u32,
    pub timeout_secs: u64,
    pub continue_on_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowTrigger {
    Manual,
    FileChange(String),
    Schedule(String), // Cron format
    AfterCommand(String),
}

impl Workflow {
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            steps: Vec::new(),
            triggers: Vec::new(),
            variables: HashMap::new(),
        }
    }
    
    pub fn add_step(&mut self, step: WorkflowStep) {
        self.steps.push(step);
    }
    
    pub fn add_variable(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }
    
    pub fn substitute_variables(&self, command: &str) -> String {
        let mut result = command.to_string();
        for (key, value) in &self.variables {
            result = result.replace(&format!("${{{}}}", key), value);
        }
        result
    }
}

pub struct WorkflowManager {
    workflows: HashMap<String, Workflow>,
}

impl WorkflowManager {
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
        }
    }
    
    pub fn add_workflow(&mut self, workflow: Workflow) {
        self.workflows.insert(workflow.name.clone(), workflow);
    }
    
    pub fn get_workflow(&self, name: &str) -> Option<&Workflow> {
        self.workflows.get(name)
    }
    
    pub fn create_common_workflows(&mut self) {
        // Rust development workflow
        let mut rust_ci = Workflow::new(
            "rust-ci".to_string(),
            "Standard Rust CI pipeline".to_string(),
        );
        rust_ci.add_step(WorkflowStep {
            name: "Format Check".to_string(),
            command: "cargo fmt --check".to_string(),
            condition: None,
            retry_count: 0,
            timeout_secs: 30,
            continue_on_error: false,
        });
        rust_ci.add_step(WorkflowStep {
            name: "Clippy Lints".to_string(),
            command: "cargo clippy -- -D warnings".to_string(),
            condition: None,
            retry_count: 1,
            timeout_secs: 60,
            continue_on_error: false,
        });
        rust_ci.add_step(WorkflowStep {
            name: "Build".to_string(),
            command: "cargo build".to_string(),
            condition: None,
            retry_count: 1,
            timeout_secs: 300,
            continue_on_error: false,
        });
        rust_ci.add_step(WorkflowStep {
            name: "Test".to_string(),
            command: "cargo test".to_string(),
            condition: None,
            retry_count: 1,
            timeout_secs: 300,
            continue_on_error: false,
        });
        self.add_workflow(rust_ci);
        
        // Project backup workflow
        let mut backup = Workflow::new(
            "project-backup".to_string(),
            "Create project backup".to_string(),
        );
        backup.add_variable("backup_dir".to_string(), "backups".to_string());
        backup.add_variable("timestamp".to_string(), chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string());
        backup.add_step(WorkflowStep {
            name: "Create Backup Directory".to_string(),
            command: "mkdir -p ${backup_dir}/${timestamp}".to_string(),
            condition: None,
            retry_count: 0,
            timeout_secs: 10,
            continue_on_error: false,
        });
        backup.add_step(WorkflowStep {
            name: "Copy Source Files".to_string(),
            command: "cp -r src ${backup_dir}/${timestamp}/".to_string(),
            condition: Some("test -d src".to_string()),
            retry_count: 0,
            timeout_secs: 60,
            continue_on_error: true,
        });
        backup.add_step(WorkflowStep {
            name: "Copy Config Files".to_string(),
            command: "cp *.toml *.json *.yaml ${backup_dir}/${timestamp}/ 2>/dev/null || true".to_string(),
            condition: None,
            retry_count: 0,
            timeout_secs: 30,
            continue_on_error: true,
        });
        self.add_workflow(backup);
    }
    
    pub async fn execute_workflow(&self, name: &str) -> Result<Vec<AgentCommand>> {
        let workflow = self.workflows.get(name)
            .ok_or_else(|| anyhow::anyhow!("Workflow not found: {}", name))?;
        
        let mut commands = Vec::new();
        
        for step in &workflow.steps {
            let command = workflow.substitute_variables(&step.command);
            
            commands.push(AgentCommand {
                command,
                description: step.name.clone(),
                risk_level: crate::agent::Agent::assess_risk_level(&step.command),
                approved: false,
                executed: false,
                output: None,
                error: None,
            });
        }
        
        Ok(commands)
    }
    
    pub fn list_workflows(&self) -> Vec<&Workflow> {
        self.workflows.values().collect()
    }
}
