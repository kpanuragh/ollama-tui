use crate::agent::{Agent, SystemContext};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Represents the state of the autonomous agent
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    /// Agent is idle, waiting for a goal
    Idle,
    /// Agent is analyzing the goal and planning next steps
    Reasoning,
    /// Agent is executing a command
    Executing,
    /// Agent is analyzing command output
    AnalyzingOutput,
    /// Goal has been achieved
    GoalAchieved,
    /// Agent encountered an error or cannot proceed
    Failed,
}

/// Represents a step in the autonomous workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub step_number: usize,
    pub reasoning: String,
    pub command: Option<String>,
    pub output: Option<String>,
    pub exit_code: Option<i32>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Memory entry for tracking agent actions and learnings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub step: AgentStep,
    pub analysis: String,
}

/// The autonomous agent that can reason, plan, and execute commands in a loop
pub struct AutonomousAgent {
    /// Current state of the agent
    pub state: AgentState,

    /// The goal the agent is working toward
    pub goal: Option<String>,

    /// System context (OS, shell, current directory, etc.)
    pub context: SystemContext,

    /// Memory of all steps taken (for learning and context)
    pub memory: VecDeque<MemoryEntry>,

    /// Maximum number of steps before forced stop
    pub max_steps: usize,

    /// Current step number
    pub current_step: usize,

    /// Maximum memory entries to keep
    pub max_memory_size: usize,
}

impl AutonomousAgent {
    /// Creates a new autonomous agent
    pub fn new() -> Self {
        Self {
            state: AgentState::Idle,
            goal: None,
            context: Agent::gather_system_context(),
            memory: VecDeque::new(),
            max_steps: 50,  // Safety limit
            current_step: 0,
            max_memory_size: 100,
        }
    }

    /// Start working on a new goal
    pub fn set_goal(&mut self, goal: String) {
        self.goal = Some(goal);
        self.state = AgentState::Reasoning;
        self.current_step = 0;
        // Keep memory from previous sessions for learning, but mark new goal
    }

    /// Creates the reasoning prompt for the LLM
    pub fn create_reasoning_prompt(&self) -> String {
        let goal_text = self.goal.as_deref().unwrap_or("No goal set");

        let memory_context = if self.memory.is_empty() {
            "No previous steps taken yet.".to_string()
        } else {
            let mut context = String::from("Previous steps:\n");
            for (idx, entry) in self.memory.iter().rev().take(10).enumerate() {
                context.push_str(&format!(
                    "\n{}. {}\n   Command: {}\n   Output: {}\n   Analysis: {}\n",
                    self.memory.len() - idx,
                    entry.step.reasoning,
                    entry.step.command.as_deref().unwrap_or("none"),
                    entry.step.output.as_deref()
                        .map(|o| if o.len() > 200 { format!("{}...", &o[..200]) } else { o.to_string() })
                        .unwrap_or_else(|| "no output".to_string()),
                    if entry.analysis.len() > 100 { format!("{}...", &entry.analysis[..100]) } else { entry.analysis.clone() }
                ));
            }
            context
        };

        format!(r#"You are an AUTONOMOUS AGENT working to achieve a goal. You operate in a continuous loop:
1. Analyze the current situation
2. Decide the next command to run
3. Execute it (I will do this for you)
4. Analyze the output
5. Repeat until goal is achieved

SYSTEM CONTEXT:
- Operating System: {}
- Shell: {}
- Current Directory: {}
- Git Repository: {}{}
- Home Directory: {}

CURRENT GOAL:
{}

{}

CURRENT STEP: {}/{}

YOUR TASK:
Analyze the situation and decide the NEXT action. Respond in this EXACT JSON format:

```json
{{
  "reasoning": "Your analysis of the current situation and why you're taking this action",
  "action": "execute" or "goal_achieved" or "need_info",
  "command": "the shell command to run (if action is 'execute', otherwise null)",
  "expected_outcome": "what you expect this command to produce"
}}
```

RULES:
1. ONLY suggest ONE command at a time
2. Use the output from previous commands to inform your next step
3. If the goal is achieved, set action to "goal_achieved"
4. If you need more information from the user, set action to "need_info"
5. Keep commands safe and read-only when possible
6. Use the system context (you're in directory: {})
7. Build upon previous steps - don't repeat unnecessarily
8. If you're stuck after 3 similar commands, try a different approach

Respond with ONLY the JSON object, no other text."#,
            self.context.os,
            self.context.shell,
            self.context.current_dir,
            if self.context.is_git_repo { "Yes" } else { "No" },
            if let Some(ref branch) = self.context.git_branch {
                format!(" (branch: {})", branch)
            } else {
                String::new()
            },
            self.context.home_dir.as_deref().unwrap_or("unknown"),
            goal_text,
            memory_context,
            self.current_step + 1,
            self.max_steps,
            self.context.current_dir
        )
    }

    /// Creates the analysis prompt after command execution
    pub fn create_analysis_prompt(&self, command: &str, output: &str, exit_code: i32) -> String {
        let goal_text = self.goal.as_deref().unwrap_or("No goal set");

        format!(r#"You are analyzing the result of a command execution in an autonomous agent loop.

GOAL: {}

COMMAND EXECUTED:
{}

EXIT CODE: {}

OUTPUT:
{}

YOUR TASK:
Analyze this output and provide insights in this EXACT JSON format:

```json
{{
  "success": true or false,
  "analysis": "Your detailed analysis of what happened and what it means",
  "progress": "What progress was made toward the goal",
  "learned": "Any important information learned from this output",
  "next_suggestion": "Brief suggestion for what to do next"
}}
```

Respond with ONLY the JSON object, no other text."#,
            goal_text,
            command,
            exit_code,
            if output.len() > 1000 {
                format!("{}... (truncated)", &output[..1000])
            } else {
                output.to_string()
            }
        )
    }

    /// Add a step to memory
    pub fn add_to_memory(&mut self, step: AgentStep, analysis: String) {
        self.memory.push_back(MemoryEntry { step, analysis });

        // Limit memory size
        while self.memory.len() > self.max_memory_size {
            self.memory.pop_front();
        }
    }

    /// Get a summary of current progress
    pub fn get_progress_summary(&self) -> String {
        format!(
            "🎯 Goal: {}\n📊 Progress: Step {}/{}\n💾 Memory: {} entries\n🤖 State: {:?}",
            self.goal.as_deref().unwrap_or("None"),
            self.current_step,
            self.max_steps,
            self.memory.len(),
            self.state
        )
    }

    /// Check if agent should stop (safety limit reached)
    pub fn should_stop(&self) -> bool {
        self.current_step >= self.max_steps
    }

    /// Reset the agent to idle state
    pub fn reset(&mut self) {
        self.state = AgentState::Idle;
        self.goal = None;
        self.current_step = 0;
        // Keep memory for learning across sessions
    }
}

impl Default for AutonomousAgent {
    fn default() -> Self {
        Self::new()
    }
}

/// Response from the reasoning module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningResponse {
    pub reasoning: String,
    pub action: String,  // "execute", "goal_achieved", "need_info"
    pub command: Option<String>,
    pub expected_outcome: Option<String>,
}

/// Response from the analysis module
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResponse {
    pub success: bool,
    pub analysis: String,
    pub progress: String,
    pub learned: String,
    pub next_suggestion: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_creation() {
        let agent = AutonomousAgent::new();
        assert_eq!(agent.state, AgentState::Idle);
        assert!(agent.goal.is_none());
        assert_eq!(agent.current_step, 0);
    }

    #[test]
    fn test_set_goal() {
        let mut agent = AutonomousAgent::new();
        agent.set_goal("Find all Rust files".to_string());
        assert_eq!(agent.state, AgentState::Reasoning);
        assert_eq!(agent.goal, Some("Find all Rust files".to_string()));
    }

    #[test]
    fn test_memory_limit() {
        let mut agent = AutonomousAgent::new();
        agent.max_memory_size = 5;

        for i in 0..10 {
            let step = AgentStep {
                step_number: i,
                reasoning: format!("Step {}", i),
                command: Some("ls".to_string()),
                output: Some("output".to_string()),
                exit_code: Some(0),
                timestamp: chrono::Utc::now(),
            };
            agent.add_to_memory(step, format!("Analysis {}", i));
        }

        assert_eq!(agent.memory.len(), 5);
    }

    #[test]
    fn test_should_stop() {
        let mut agent = AutonomousAgent::new();
        agent.max_steps = 10;
        agent.current_step = 11;
        assert!(agent.should_stop());

        agent.current_step = 5;
        assert!(!agent.should_stop());
    }
}
