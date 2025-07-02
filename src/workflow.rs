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
    /// Creates a new workflow with the specified name and description, initializing empty steps, triggers, and variables.
    ///
    /// # Examples
    ///
    /// ```
    /// let workflow = Workflow::new("build".to_string(), "Builds the project".to_string());
    /// assert_eq!(workflow.name, "build");
    /// assert_eq!(workflow.description, "Builds the project");
    /// assert!(workflow.steps.is_empty());
    /// assert!(workflow.triggers.is_empty());
    /// assert!(workflow.variables.is_empty());
    /// ```
    pub fn new(name: String, description: String) -> Self {
        Self {
            name,
            description,
            steps: Vec::new(),
            triggers: Vec::new(),
            variables: HashMap::new(),
        }
    }
    
    /// Adds a step to the workflow's sequence of steps.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut workflow = Workflow::new("example".to_string(), "Demo workflow".to_string());
    /// let step = WorkflowStep {
    ///     name: "build".to_string(),
    ///     command: "cargo build".to_string(),
    ///     condition: None,
    ///     retry_count: 0,
    ///     timeout_secs: 60,
    ///     continue_on_error: false,
    /// };
    /// workflow.add_step(step);
    /// assert_eq!(workflow.steps.len(), 1);
    /// ```
    pub fn add_step(&mut self, step: WorkflowStep) {
        self.steps.push(step);
    }
    
    /// Adds a variable to the workflow's variable map, associating the given key with the specified value.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut workflow = Workflow::new("example".to_string(), "desc".to_string());
    /// workflow.add_variable("ENV".to_string(), "production".to_string());
    /// assert_eq!(workflow.variables.get("ENV"), Some(&"production".to_string()));
    /// ```
    pub fn add_variable(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }
    
    /// Replaces variable placeholders in a command string with their corresponding values from the workflow's variables map.
    ///
    /// Variable placeholders are expected in the format `${VAR_NAME}` and are substituted with the value associated with `VAR_NAME`.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut workflow = Workflow::new("example".to_string(), "desc".to_string());
    /// workflow.add_variable("USER".to_string(), "alice".to_string());
    /// let cmd = "echo Hello, ${USER}!";
    /// let substituted = workflow.substitute_variables(cmd);
    /// assert_eq!(substituted, "echo Hello, alice!");
    /// ```
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
    /// Creates a new, empty `WorkflowManager` with no workflows.
    ///
    /// # Examples
    ///
    /// ```
    /// let manager = WorkflowManager::new();
    /// assert_eq!(manager.list_workflows().len(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            workflows: HashMap::new(),
        }
    }
    
    /// Adds a workflow to the manager, keyed by its name.
    ///
    /// If a workflow with the same name already exists, it will be replaced.
    pub fn add_workflow(&mut self, workflow: Workflow) {
        self.workflows.insert(workflow.name.clone(), workflow);
    }
    
    /// Retrieves a reference to a workflow by its name, if it exists.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut manager = WorkflowManager::new();
    /// let workflow = Workflow::new("build".to_string(), "Build project".to_string());
    /// manager.add_workflow(workflow);
    /// let retrieved = manager.get_workflow("build");
    /// assert!(retrieved.is_some());
    /// ```
    pub fn get_workflow(&self, name: &str) -> Option<&Workflow> {
        self.workflows.get(name)
    }
    
    /// Adds predefined common workflows to the manager.
    ///
    /// This method creates and registers two standard workflows:
    /// - "rust-ci": A Rust continuous integration pipeline with steps for formatting, linting, building, and testing.
    /// - "project-backup": A backup workflow that creates a timestamped backup directory and copies source and configuration files, using variable substitution for directory names and supporting conditional and error-tolerant steps.
    ///
    /// These workflows are added to the manager and can be retrieved or executed later.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut manager = WorkflowManager::new();
    /// manager.create_common_workflows();
    /// assert!(manager.get_workflow("rust-ci").is_some());
    /// assert!(manager.get_workflow("project-backup").is_some());
    /// ```
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
    
    /// Generates a list of agent commands for a workflow by name, substituting variables and assessing risk for each step.
    ///
    /// Returns an error if the workflow does not exist. Each `AgentCommand` contains the substituted command, step description, risk level, and default execution metadata.
    ///
    /// # Returns
    /// A vector of `AgentCommand` objects representing the workflow steps with variables substituted.
    ///
    /// # Examples
    ///
    /// ```
    /// let manager = WorkflowManager::new();
    /// // ... add workflows ...
    /// let commands = tokio_test::block_on(manager.execute_workflow("rust-ci")).unwrap();
    /// assert!(!commands.is_empty());
    /// ```
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
    
    /// Returns a vector of references to all workflows managed by this instance.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut manager = WorkflowManager::new();
    /// manager.create_common_workflows();
    /// let workflows = manager.list_workflows();
    /// assert!(!workflows.is_empty());
    /// ```
    pub fn list_workflows(&self) -> Vec<&Workflow> {
        self.workflows.values().collect()
    }
}
