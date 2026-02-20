//! Interactive pushback loop for `spec plan`.
//!
//! When the planner encounters under-specified requirements, ambiguous
//! verification, or needs user decisions, it presents options and waits
//! for input. The loop continues until all task specs have verification
//! strategies or the user explicitly stops.

use std::fmt::Write as _;
use std::io::{BufRead, Write};

use serde::{Deserialize, Serialize};

use crate::context::ServiceContext;
use crate::ports::llm::{CompletionRequest, CompletionResponse};
use crate::spec::{SignalType, TaskSpec, VerificationStrategy};

/// A question the planner needs answered before it can proceed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PushbackQuestion {
    /// The task spec ID this question relates to.
    pub task_id: String,
    /// Human-readable description of the problem.
    pub description: String,
    /// Proposed options (labeled a, b, c, ...).
    pub options: Vec<String>,
}

/// An action the user can take during the conversation.
#[derive(Debug, Clone, PartialEq)]
pub enum UserAction {
    /// Pick one of the offered options (1-indexed).
    PickOption(usize),
    /// Free-form feedback text.
    Feedback(String),
    /// Add a new foundational task.
    AddTask {
        /// Title for the new task.
        title: String,
    },
    /// Accept current specs and stop the loop.
    Accept,
    /// Stop without finalizing.
    Stop,
}

/// Result of one conversation turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversationTurn {
    /// Summary shown to the user.
    pub message: String,
    /// Questions requiring user input (empty when all specs are resolved).
    pub questions: Vec<PushbackQuestion>,
    /// Current task specs (may be updated each turn).
    pub specs: Vec<TaskSpec>,
}

/// State for the interactive conversation loop.
pub struct ConversationLoop<R: BufRead, W: Write> {
    /// Current task specs being refined.
    specs: Vec<TaskSpec>,
    /// Reader for user input.
    reader: R,
    /// Writer for presenting output.
    writer: W,
}

impl<R: BufRead, W: Write> ConversationLoop<R, W> {
    /// Creates a new conversation loop with the given initial specs.
    pub fn new(specs: Vec<TaskSpec>, reader: R, writer: W) -> Self {
        Self { specs, reader, writer }
    }

    /// Runs the interactive loop until all specs are resolved or the user stops.
    ///
    /// Each iteration:
    /// 1. Analyzes specs via LLM to find unresolved questions
    /// 2. Presents findings and questions to the user
    /// 3. Reads user input and applies changes
    /// 4. Repeats until no questions remain or user says "stop"/"accept"
    ///
    /// # Errors
    ///
    /// Returns an error if LLM calls fail or I/O fails.
    pub async fn run(mut self, ctx: &ServiceContext) -> Result<Vec<TaskSpec>, String> {
        loop {
            let turn = self.analyze_specs(ctx).await?;

            self.present_turn(&turn)?;

            if turn.questions.is_empty() {
                writeln!(self.writer, "\nAll task specs have verification strategies. Done.")
                    .map_err(|e| format!("write error: {e}"))?;
                break;
            }

            let action = self.read_user_input()?;

            match action {
                UserAction::Accept => {
                    writeln!(self.writer, "\nAccepting current specs.")
                        .map_err(|e| format!("write error: {e}"))?;
                    break;
                }
                UserAction::Stop => {
                    writeln!(self.writer, "\nStopping. Specs are not finalized.")
                        .map_err(|e| format!("write error: {e}"))?;
                    break;
                }
                UserAction::PickOption(idx) => {
                    self.apply_option(ctx, &turn.questions, idx).await?;
                }
                UserAction::Feedback(text) => {
                    self.apply_feedback(ctx, &turn.questions, &text).await?;
                }
                UserAction::AddTask { title } => {
                    self.add_foundational_task(ctx, &title).await?;
                }
            }
        }

        Ok(self.specs)
    }

    /// Analyzes current specs via LLM to identify unresolved questions.
    async fn analyze_specs(&self, ctx: &ServiceContext) -> Result<ConversationTurn, String> {
        let prompt = build_analysis_prompt(&self.specs);
        let request = CompletionRequest {
            model: "claude-sonnet-4-20250514".into(),
            prompt,
            max_tokens: 4096,
        };

        let response: CompletionResponse =
            ctx.llm.complete(&request).await.map_err(|e| format!("LLM analysis failed: {e}"))?;

        parse_analysis_response(&response.text, &self.specs)
    }

    /// Presents a conversation turn to the user via the writer.
    fn present_turn(&mut self, turn: &ConversationTurn) -> Result<(), String> {
        writeln!(self.writer, "\n{}", turn.message).map_err(|e| format!("write error: {e}"))?;

        for (i, q) in turn.questions.iter().enumerate() {
            writeln!(self.writer, "\n--- Question {} (task {}) ---", i + 1, q.task_id)
                .map_err(|e| format!("write error: {e}"))?;
            writeln!(self.writer, "{}", q.description).map_err(|e| format!("write error: {e}"))?;
            for (j, opt) in q.options.iter().enumerate() {
                #[allow(clippy::cast_possible_truncation)]
                let label = char::from(b'a' + j as u8);
                writeln!(self.writer, "  {label}) {opt}")
                    .map_err(|e| format!("write error: {e}"))?;
            }
        }

        write!(self.writer, "\n> ").map_err(|e| format!("write error: {e}"))?;
        self.writer.flush().map_err(|e| format!("flush error: {e}"))?;
        Ok(())
    }

    /// Reads and parses user input from the reader.
    fn read_user_input(&mut self) -> Result<UserAction, String> {
        let mut line = String::new();
        self.reader.read_line(&mut line).map_err(|e| format!("read error: {e}"))?;
        let trimmed = line.trim();
        Ok(parse_user_input(trimmed))
    }

    /// Applies a user's option selection by asking the LLM to refine specs.
    async fn apply_option(
        &mut self,
        ctx: &ServiceContext,
        questions: &[PushbackQuestion],
        option_idx: usize,
    ) -> Result<(), String> {
        // Build a prompt describing the chosen option
        let mut prompt = String::from("The user chose option ");
        #[allow(clippy::cast_possible_truncation)]
        let label = char::from(b'a' + (option_idx - 1) as u8);
        let _ = writeln!(prompt, "'{label}' for the following questions:\n");

        for q in questions {
            let _ = writeln!(prompt, "Task {}: {}", q.task_id, q.description);
            if option_idx <= q.options.len() {
                let _ = writeln!(prompt, "Chosen: {}", q.options[option_idx - 1]);
            }
        }

        let _ = writeln!(prompt, "\nCurrent specs:");
        for spec in &self.specs {
            let _ = writeln!(prompt, "- {} ({}): {:?}", spec.id, spec.title, spec.signal_type);
        }

        prompt.push_str(
            "\nUpdate the task specs based on the user's choice. \
             Respond with JSON: {\"updates\": [{\"task_id\": \"...\", \"title\": \"...\", \
             \"signal_type\": \"clear|fuzzy|internal_logic\", \
             \"verification\": \"resolved\"}], \
             \"new_tasks\": [{\"id\": \"...\", \"title\": \"...\"}]}",
        );

        let request = CompletionRequest {
            model: "claude-sonnet-4-20250514".into(),
            prompt,
            max_tokens: 2048,
        };

        let response: CompletionResponse =
            ctx.llm.complete(&request).await.map_err(|e| format!("LLM update failed: {e}"))?;

        self.apply_llm_updates(&response.text)
    }

    /// Applies free-form user feedback by asking the LLM to refine specs.
    async fn apply_feedback(
        &mut self,
        ctx: &ServiceContext,
        questions: &[PushbackQuestion],
        feedback: &str,
    ) -> Result<(), String> {
        let mut prompt = String::from("The user provided feedback:\n");
        let _ = writeln!(prompt, "\"{feedback}\"\n");
        let _ = writeln!(prompt, "Open questions:");
        for q in questions {
            let _ = writeln!(prompt, "- Task {}: {}", q.task_id, q.description);
        }

        let _ = writeln!(prompt, "\nCurrent specs:");
        for spec in &self.specs {
            let _ = writeln!(prompt, "- {} ({}): {:?}", spec.id, spec.title, spec.signal_type);
        }

        prompt.push_str(
            "\nUpdate task specs based on the feedback. \
             Respond with JSON: {\"updates\": [{\"task_id\": \"...\", \"title\": \"...\", \
             \"signal_type\": \"clear|fuzzy|internal_logic\", \
             \"verification\": \"resolved\"}], \
             \"new_tasks\": [{\"id\": \"...\", \"title\": \"...\"}]}",
        );

        let request = CompletionRequest {
            model: "claude-sonnet-4-20250514".into(),
            prompt,
            max_tokens: 2048,
        };

        let response: CompletionResponse =
            ctx.llm.complete(&request).await.map_err(|e| format!("LLM feedback failed: {e}"))?;

        self.apply_llm_updates(&response.text)
    }

    /// Adds a new foundational task via LLM.
    async fn add_foundational_task(
        &mut self,
        ctx: &ServiceContext,
        title: &str,
    ) -> Result<(), String> {
        let mut prompt = String::new();
        let _ = writeln!(prompt, "Create a new foundational task spec for: \"{title}\"");
        let _ = writeln!(prompt, "\nExisting tasks:");
        for spec in &self.specs {
            let _ = writeln!(prompt, "- {} ({})", spec.id, spec.title);
        }

        prompt.push_str(
            "\nRespond with JSON: {\"id\": \"TASK-N\", \"title\": \"...\", \
             \"signal_type\": \"clear\", \"acceptance_criteria\": [\"...\"], \
             \"verification\": \"resolved\"}",
        );

        let request = CompletionRequest {
            model: "claude-sonnet-4-20250514".into(),
            prompt,
            max_tokens: 1024,
        };

        let response: CompletionResponse =
            ctx.llm.complete(&request).await.map_err(|e| format!("LLM add-task failed: {e}"))?;

        self.apply_new_task(&response.text)
    }

    /// Applies LLM-proposed updates to current specs.
    fn apply_llm_updates(&mut self, response: &str) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Updates {
            #[serde(default)]
            updates: Vec<TaskUpdate>,
            #[serde(default)]
            new_tasks: Vec<NewTask>,
        }

        #[derive(Deserialize)]
        struct TaskUpdate {
            task_id: String,
            #[serde(default)]
            title: Option<String>,
            #[serde(default)]
            signal_type: Option<String>,
        }

        #[derive(Deserialize)]
        struct NewTask {
            id: String,
            title: String,
        }

        let parsed: Updates =
            serde_json::from_str(response).map_err(|e| format!("parse LLM updates: {e}"))?;

        for update in &parsed.updates {
            if let Some(spec) = self.specs.iter_mut().find(|s| s.id == update.task_id) {
                if let Some(title) = &update.title {
                    spec.title.clone_from(title);
                }
                if let Some(st) = &update.signal_type {
                    if let Some(parsed_st) = parse_signal_type(st) {
                        spec.signal_type = parsed_st;
                    }
                }
            }
        }

        for new_task in &parsed.new_tasks {
            let spec = TaskSpec {
                id: new_task.id.clone(),
                title: new_task.title.clone(),
                requirement: None,
                context: None,
                acceptance_criteria: vec![],
                signal_type: SignalType::Clear,
                verification: VerificationStrategy::DirectAssertion { checks: vec![] },
            };
            self.specs.push(spec);
        }

        Ok(())
    }

    /// Applies a single new task from LLM response.
    fn apply_new_task(&mut self, response: &str) -> Result<(), String> {
        #[derive(Deserialize)]
        struct NewTaskResponse {
            id: String,
            title: String,
            #[serde(default)]
            signal_type: Option<String>,
            #[serde(default)]
            acceptance_criteria: Vec<String>,
        }

        let parsed: NewTaskResponse =
            serde_json::from_str(response).map_err(|e| format!("parse new task: {e}"))?;

        let signal_type =
            parsed.signal_type.as_deref().and_then(parse_signal_type).unwrap_or(SignalType::Clear);

        let spec = TaskSpec {
            id: parsed.id,
            title: parsed.title,
            requirement: None,
            context: None,
            acceptance_criteria: parsed.acceptance_criteria,
            signal_type,
            verification: VerificationStrategy::DirectAssertion { checks: vec![] },
        };
        self.specs.push(spec);

        Ok(())
    }
}

/// Parses a signal type string into a `SignalType`.
fn parse_signal_type(s: &str) -> Option<SignalType> {
    match s {
        "clear" => Some(SignalType::Clear),
        "fuzzy" => Some(SignalType::Fuzzy),
        "internal_logic" => Some(SignalType::InternalLogic),
        _ => None,
    }
}

/// Parses user input into a `UserAction`.
fn parse_user_input(input: &str) -> UserAction {
    let lower = input.to_lowercase();
    match lower.as_str() {
        "accept" | "done" | "yes" => UserAction::Accept,
        "stop" | "quit" | "exit" | "no" => UserAction::Stop,
        _ => {
            // Check for option letter (a, b, c, ...) or "option a"
            let cleaned = lower.trim_start_matches("option ").trim();
            if cleaned.len() == 1 {
                let ch = cleaned.as_bytes()[0];
                if ch.is_ascii_lowercase() {
                    #[allow(clippy::cast_possible_truncation)]
                    return UserAction::PickOption((ch - b'a' + 1) as usize);
                }
            }

            // Check for "add task: <title>"
            if let Some(title) = lower.strip_prefix("add task:") {
                let title = title.trim();
                if !title.is_empty() {
                    return UserAction::AddTask { title: title.to_string() };
                }
            }
            if let Some(title) = input.strip_prefix("add task:") {
                let title = title.trim();
                if !title.is_empty() {
                    return UserAction::AddTask { title: title.to_string() };
                }
            }

            // Everything else is free-form feedback
            UserAction::Feedback(input.to_string())
        }
    }
}

/// Builds the LLM prompt for analyzing current specs.
fn build_analysis_prompt(specs: &[TaskSpec]) -> String {
    let mut prompt = String::new();
    prompt.push_str(
        "Analyze these task specs and identify any that lack proper verification strategies \
         or have ambiguous requirements.\n\n",
    );

    prompt.push_str("## Task Specs\n\n");
    for spec in specs {
        let _ = writeln!(prompt, "### {} — {}", spec.id, spec.title);
        if let Some(req) = &spec.requirement {
            let _ = writeln!(prompt, "Requirement: {req}");
        }
        let _ = writeln!(prompt, "Signal type: {:?}", spec.signal_type);
        let _ = writeln!(prompt, "Acceptance criteria:");
        for ac in &spec.acceptance_criteria {
            let _ = writeln!(prompt, "  - {ac}");
        }
        let _ = writeln!(prompt, "Verification: {:?}\n", spec.verification);
    }

    prompt.push_str(
        "## Instructions\n\n\
         Respond with JSON (no markdown fences):\n\
         {\n  \
           \"summary\": \"Brief overview of findings\",\n  \
           \"questions\": [\n    \
             {\n      \
               \"task_id\": \"TASK-ID\",\n      \
               \"description\": \"What's unclear or unverifiable\",\n      \
               \"options\": [\"option a description\", \"option b description\"]\n    \
             }\n  \
           ]\n\
         }\n\n\
         - If all specs have clear verification strategies, return an empty questions array.\n\
         - Each question should offer 2-3 concrete options.\n\
         - Focus on verification strategy gaps and ambiguous acceptance criteria.\n",
    );

    prompt
}

/// Parses the LLM analysis response into a `ConversationTurn`.
fn parse_analysis_response(response: &str, specs: &[TaskSpec]) -> Result<ConversationTurn, String> {
    #[derive(Deserialize)]
    struct AnalysisResponse {
        summary: String,
        #[serde(default)]
        questions: Vec<QuestionResponse>,
    }

    #[derive(Deserialize)]
    struct QuestionResponse {
        task_id: String,
        description: String,
        #[serde(default)]
        options: Vec<String>,
    }

    let parsed: AnalysisResponse = serde_json::from_str(response)
        .map_err(|e| format!("failed to parse LLM analysis response: {e}"))?;

    let questions = parsed
        .questions
        .into_iter()
        .map(|q| PushbackQuestion {
            task_id: q.task_id,
            description: q.description,
            options: q.options,
        })
        .collect();

    Ok(ConversationTurn { message: parsed.summary, questions, specs: specs.to_vec() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cassette::format::{Cassette, Interaction};
    use crate::context::ServiceContext;
    use crate::spec::{VerificationCheck, VerificationStrategy};
    use chrono::Utc;
    use serde_json::json;
    use std::path::Path;

    /// Helper to write a cassette file and return its path.
    fn write_cassette(
        dir: &Path,
        name: &str,
        interactions: Vec<Interaction>,
    ) -> std::path::PathBuf {
        let cassette = Cassette {
            name: name.into(),
            recorded_at: Utc::now(),
            commit: "abc".into(),
            interactions,
        };
        let yaml = serde_yaml::to_string(&cassette).unwrap();
        let path = dir.join(format!("{name}.cassette.yaml"));
        std::fs::write(&path, yaml).unwrap();
        path
    }

    fn sample_spec(id: &str, title: &str, has_verification: bool) -> TaskSpec {
        let verification = if has_verification {
            VerificationStrategy::DirectAssertion {
                checks: vec![VerificationCheck::TestSuite {
                    command: "cargo test".into(),
                    expected: "all pass".into(),
                }],
            }
        } else {
            VerificationStrategy::DirectAssertion { checks: vec![] }
        };

        TaskSpec {
            id: id.into(),
            title: title.into(),
            requirement: Some("req-1".into()),
            context: None,
            acceptance_criteria: vec!["it works".into()],
            signal_type: SignalType::Clear,
            verification,
        }
    }

    // --- parse_user_input tests ---

    #[test]
    fn parse_accept() {
        assert_eq!(parse_user_input("accept"), UserAction::Accept);
        assert_eq!(parse_user_input("done"), UserAction::Accept);
        assert_eq!(parse_user_input("yes"), UserAction::Accept);
    }

    #[test]
    fn parse_stop() {
        assert_eq!(parse_user_input("stop"), UserAction::Stop);
        assert_eq!(parse_user_input("quit"), UserAction::Stop);
        assert_eq!(parse_user_input("exit"), UserAction::Stop);
    }

    #[test]
    fn parse_option_letter() {
        assert_eq!(parse_user_input("a"), UserAction::PickOption(1));
        assert_eq!(parse_user_input("b"), UserAction::PickOption(2));
        assert_eq!(parse_user_input("c"), UserAction::PickOption(3));
        assert_eq!(parse_user_input("option a"), UserAction::PickOption(1));
    }

    #[test]
    fn parse_add_task() {
        assert_eq!(
            parse_user_input("add task: Component test infrastructure"),
            UserAction::AddTask { title: "component test infrastructure".into() }
        );
    }

    #[test]
    fn parse_feedback() {
        assert_eq!(
            parse_user_input("The timeline should also support filtering"),
            UserAction::Feedback("The timeline should also support filtering".into())
        );
    }

    // --- parse_analysis_response tests ---

    #[test]
    fn parse_analysis_with_questions() {
        let specs = vec![sample_spec("TASK-1", "Build UI", false)];
        let response = serde_json::to_string(&json!({
            "summary": "Task 1 has no verification strategy",
            "questions": [{
                "task_id": "TASK-1",
                "description": "No component test infrastructure exists",
                "options": [
                    "Add foundational task for component tests",
                    "Use structural assertions only"
                ]
            }]
        }))
        .unwrap();

        let turn = parse_analysis_response(&response, &specs).unwrap();
        assert_eq!(turn.message, "Task 1 has no verification strategy");
        assert_eq!(turn.questions.len(), 1);
        assert_eq!(turn.questions[0].task_id, "TASK-1");
        assert_eq!(turn.questions[0].options.len(), 2);
    }

    #[test]
    fn parse_analysis_all_resolved() {
        let specs = vec![sample_spec("TASK-1", "Build UI", true)];
        let response = serde_json::to_string(&json!({
            "summary": "All specs have verification strategies",
            "questions": []
        }))
        .unwrap();

        let turn = parse_analysis_response(&response, &specs).unwrap();
        assert!(turn.questions.is_empty());
    }

    #[test]
    fn parse_analysis_rejects_invalid_json() {
        let specs = vec![];
        let result = parse_analysis_response("not json", &specs);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("failed to parse"));
    }

    // --- build_analysis_prompt tests ---

    #[test]
    fn analysis_prompt_includes_spec_details() {
        let specs = vec![sample_spec("TASK-1", "Build UI", false)];
        let prompt = build_analysis_prompt(&specs);
        assert!(prompt.contains("TASK-1"));
        assert!(prompt.contains("Build UI"));
        assert!(prompt.contains("Clear"));
        assert!(prompt.contains("it works"));
    }

    // --- ConversationLoop integration tests ---

    #[tokio::test]
    async fn conversation_loop_all_resolved() {
        let dir = std::env::temp_dir().join("speck_conv_test_resolved");
        std::fs::create_dir_all(&dir).unwrap();

        let analysis_response = serde_json::to_string(&json!({
            "summary": "All task specs have clear verification strategies.",
            "questions": []
        }))
        .unwrap();

        let interactions = vec![Interaction {
            seq: 0,
            port: "llm".into(),
            method: "complete".into(),
            input: json!({}),
            output: json!({
                "ok": {
                    "text": analysis_response,
                    "prompt_tokens": 200,
                    "completion_tokens": 50
                }
            }),
        }];

        let cassette_path = write_cassette(&dir, "conv_resolved", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![sample_spec("TASK-1", "Build UI", true)];
        let reader = std::io::Cursor::new(b"" as &[u8]);
        let mut output = Vec::new();

        let conv = ConversationLoop::new(specs.clone(), reader, &mut output);
        let result = conv.run(&ctx).await.unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "TASK-1");

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("All task specs have verification strategies. Done."));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn conversation_loop_user_accepts() {
        let dir = std::env::temp_dir().join("speck_conv_test_accept");
        std::fs::create_dir_all(&dir).unwrap();

        let analysis_response = serde_json::to_string(&json!({
            "summary": "Task 1 needs verification",
            "questions": [{
                "task_id": "TASK-1",
                "description": "No test infrastructure",
                "options": ["Add tests", "Skip tests"]
            }]
        }))
        .unwrap();

        let interactions = vec![Interaction {
            seq: 0,
            port: "llm".into(),
            method: "complete".into(),
            input: json!({}),
            output: json!({
                "ok": {
                    "text": analysis_response,
                    "prompt_tokens": 200,
                    "completion_tokens": 50
                }
            }),
        }];

        let cassette_path = write_cassette(&dir, "conv_accept", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![sample_spec("TASK-1", "Build UI", false)];
        let reader = std::io::Cursor::new(b"accept\n");
        let mut output = Vec::new();

        let conv = ConversationLoop::new(specs, reader, &mut output);
        let result = conv.run(&ctx).await.unwrap();

        assert_eq!(result.len(), 1);

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Task 1 needs verification"));
        assert!(output_str.contains("Accepting current specs"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn conversation_loop_user_stops() {
        let dir = std::env::temp_dir().join("speck_conv_test_stop");
        std::fs::create_dir_all(&dir).unwrap();

        let analysis_response = serde_json::to_string(&json!({
            "summary": "Task needs work",
            "questions": [{
                "task_id": "TASK-1",
                "description": "Unclear requirement",
                "options": ["Clarify", "Skip"]
            }]
        }))
        .unwrap();

        let interactions = vec![Interaction {
            seq: 0,
            port: "llm".into(),
            method: "complete".into(),
            input: json!({}),
            output: json!({
                "ok": {
                    "text": analysis_response,
                    "prompt_tokens": 200,
                    "completion_tokens": 50
                }
            }),
        }];

        let cassette_path = write_cassette(&dir, "conv_stop", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![sample_spec("TASK-1", "Build UI", false)];
        let reader = std::io::Cursor::new(b"stop\n");
        let mut output = Vec::new();

        let conv = ConversationLoop::new(specs, reader, &mut output);
        let result = conv.run(&ctx).await.unwrap();

        assert_eq!(result.len(), 1);

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("Stopping. Specs are not finalized."));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn conversation_loop_option_selection() {
        let dir = std::env::temp_dir().join("speck_conv_test_option");
        std::fs::create_dir_all(&dir).unwrap();

        let analysis_response = serde_json::to_string(&json!({
            "summary": "Task 1 needs verification",
            "questions": [{
                "task_id": "TASK-1",
                "description": "No test infrastructure",
                "options": ["Add foundational task", "Use structural assertions"]
            }]
        }))
        .unwrap();

        // Second LLM call: apply option
        let update_response = serde_json::to_string(&json!({
            "updates": [{"task_id": "TASK-1", "title": "Build UI with tests"}],
            "new_tasks": [{"id": "TASK-2", "title": "Component test infrastructure"}]
        }))
        .unwrap();

        // Third LLM call: re-analysis showing resolved
        let resolved_response = serde_json::to_string(&json!({
            "summary": "All specs resolved",
            "questions": []
        }))
        .unwrap();

        let interactions = vec![
            Interaction {
                seq: 0,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": analysis_response,
                        "prompt_tokens": 200,
                        "completion_tokens": 50
                    }
                }),
            },
            Interaction {
                seq: 1,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": update_response,
                        "prompt_tokens": 300,
                        "completion_tokens": 100
                    }
                }),
            },
            Interaction {
                seq: 2,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": resolved_response,
                        "prompt_tokens": 200,
                        "completion_tokens": 50
                    }
                }),
            },
        ];

        let cassette_path = write_cassette(&dir, "conv_option", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![sample_spec("TASK-1", "Build UI", false)];
        let reader = std::io::Cursor::new(b"a\n");
        let mut output = Vec::new();

        let conv = ConversationLoop::new(specs, reader, &mut output);
        let result = conv.run(&ctx).await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title, "Build UI with tests");
        assert_eq!(result[1].id, "TASK-2");
        assert_eq!(result[1].title, "Component test infrastructure");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn conversation_loop_add_task() {
        let dir = std::env::temp_dir().join("speck_conv_test_add");
        std::fs::create_dir_all(&dir).unwrap();

        let analysis_response = serde_json::to_string(&json!({
            "summary": "Task 1 needs infrastructure",
            "questions": [{
                "task_id": "TASK-1",
                "description": "Missing test setup",
                "options": ["Add tests"]
            }]
        }))
        .unwrap();

        // LLM call for adding task
        let new_task_response = serde_json::to_string(&json!({
            "id": "TASK-99",
            "title": "E2E test infrastructure",
            "signal_type": "clear",
            "acceptance_criteria": ["Playwright configured"]
        }))
        .unwrap();

        // Re-analysis showing resolved
        let resolved_response = serde_json::to_string(&json!({
            "summary": "All resolved",
            "questions": []
        }))
        .unwrap();

        let interactions = vec![
            Interaction {
                seq: 0,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": analysis_response,
                        "prompt_tokens": 200,
                        "completion_tokens": 50
                    }
                }),
            },
            Interaction {
                seq: 1,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": new_task_response,
                        "prompt_tokens": 200,
                        "completion_tokens": 50
                    }
                }),
            },
            Interaction {
                seq: 2,
                port: "llm".into(),
                method: "complete".into(),
                input: json!({}),
                output: json!({
                    "ok": {
                        "text": resolved_response,
                        "prompt_tokens": 200,
                        "completion_tokens": 50
                    }
                }),
            },
        ];

        let cassette_path = write_cassette(&dir, "conv_add_task", interactions);
        let ctx = ServiceContext::replaying(&cassette_path).unwrap();

        let specs = vec![sample_spec("TASK-1", "Build UI", false)];
        let reader = std::io::Cursor::new(b"add task: E2E test infrastructure\n");
        let mut output = Vec::new();

        let conv = ConversationLoop::new(specs, reader, &mut output);
        let result = conv.run(&ctx).await.unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[1].id, "TASK-99");
        assert_eq!(result[1].title, "E2E test infrastructure");
        assert_eq!(result[1].acceptance_criteria, vec!["Playwright configured"]);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
