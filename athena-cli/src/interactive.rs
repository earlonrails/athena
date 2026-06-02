use athena_agent::AIAgent;
use athena_tools::ToolRegistry;
use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType, Context};
use rustyline::completion::{Completer, Pair};
use rustyline::hint::Hinter;
use rustyline::highlight::Highlighter;
use rustyline::validate::Validator;
use rustyline::Helper;
use rustyline::history::DefaultHistory;
use tracing::error;

#[derive(Default)]
struct AthenaHelper {
    commands: Vec<String>,
}

impl Completer for AthenaHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> Result<(usize, Vec<Pair>), ReadlineError> {
        if line.starts_with('/') {
            let word = &line[..pos];
            let mut matches = Vec::new();
            for cmd in &self.commands {
                if cmd.starts_with(word) {
                    matches.push(Pair {
                        display: cmd.clone(),
                        replacement: cmd.clone(),
                    });
                }
            }
            Ok((0, matches))
        } else {
            Ok((0, Vec::new()))
        }
    }
}

impl Hinter for AthenaHelper {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for AthenaHelper {}
impl Validator for AthenaHelper {}
impl Helper for AthenaHelper {}


pub async fn run_interactive_loop(mut agent: AIAgent, registry: &ToolRegistry, provider: std::sync::Arc<dyn athena_providers::LLMProvider + Send + Sync>) {
    let config = Config::builder()
        .completion_type(CompletionType::List)
        .build();

    let mut rl = match Editor::<AthenaHelper, DefaultHistory>::with_config(config) {
        Ok(rl) => rl,
        Err(e) => {
            error!("Failed to initialize readline: {}", e);
            return;
        }
    };

    rl.set_helper(Some(AthenaHelper {
        commands: vec![
            "/help".into(),
            "/model".into(),
            "/tools".into(),
            "/skills browse".into(),
            "/background ".into(),
            "/skin".into(),
            "/voice on".into(),
            "/voice tts".into(),
            "/reasoning high".into(),
            "/title ".into(),
            "/status".into(),
            "/sessions".into(),
        ]
    }));

    let raw_model = agent.model();
    let provider_str = if raw_model.contains("claude") || raw_model.contains("opus") || raw_model.contains("sonnet") {
        " (Anthropic)"
    } else if raw_model.contains("gpt") {
        " (OpenAI)"
    } else {
        ""
    };
    let model_display = format!("{}{}", raw_model, provider_str);

    let mut state = SessionState {
        title: String::from("Untitled Session"),
        turn_count: 0,
        voice_mode: false,
        tts_mode: false,
        active_skin: String::from("default"),
    };
    
    println!("🦉 Athena Interactive Agent Session (v{})", env!("CARGO_PKG_VERSION"));
    println!("Active Model: {}", model_display);
    println!("Sandbox Target: Docker (Local container active)");
    println!("Press Ctrl+D or type 'exit' to quit.");
    println!();

    loop {
        let readline = rl.readline("athena> ");
        match readline {
            Ok(line) => {
                let input = line.trim();
                if input.is_empty() {
                    continue;
                }

                if input == "/quit" || input == "/exit" || input == "exit" || input == "quit" {
                    println!("Goodbye!");
                    break;
                }

                if input.starts_with('/') {
                    let _ = rl.add_history_entry(input);
                    let parts: Vec<&str> = input.split_whitespace().collect();
                    if parts.is_empty() { continue; }
                    let cmd = parts[0];

                    println!();
                    process_slash_command(cmd, &parts, &mut state, &mut agent, provider.clone(), &model_display).await;
                    println!();
                    continue;
                }

                state.turn_count += 1;
                let _ = rl.add_history_entry(input);

                println!();
                
                // For now, no persistent history passed in, just a stateless run.
                // In a future PR we will track history.
                let system_prompt = "You are Athena, a powerful AI assistant running locally on the user's system via an interactive terminal. You have full access to execute terminal commands, read files, and automate tasks through your tools. Do not decline requests to run commands on the user's system. Use your provided tools to accomplish the user's goals. \
                \
                IMPORTANT: You have an internal cron engine that runs jobs automatically. You can configure this by editing ~/.athena/config.yaml under the `cron_jobs` block. Cron jobs execute your conversations internally. You can configure them to report back to Telegram by setting the `channel` (integer) and `thread` (integer) fields on the cron job. \
                You can also manage skills (in ~/.athena/skills) and plugins (in ~/.athena/plugins) and MCP endpoints. \
                You have hot-reloading enabled, meaning if you edit config.yaml to add a cron job, the gateway will detect it within 30 seconds and start running it automatically.";
                match agent.run_conversation(input, Some(system_prompt), registry, provider.clone()).await {
                    Ok(response) => {
                        println!("\n{}\n", response);
                    }
                    Err(e) => {
                        error!("Agent error: {}", e);
                        println!("Error: {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                error!("Error reading line: {:?}", err);
                break;
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SessionState {
    pub title: String,
    pub turn_count: usize,
    pub voice_mode: bool,
    pub tts_mode: bool,
    pub active_skin: String,
}

pub async fn process_slash_command(
    cmd: &str,
    parts: &[&str],
    state: &mut SessionState,
    agent: &mut AIAgent,
    provider: std::sync::Arc<dyn athena_providers::LLMProvider + Send + Sync>,
    model_display: &str,
) {
    match cmd {
        "/help" => {
            println!("Available commands:");
            println!("  /help           Show command help");
            println!("  /model          Show or change the current model");
            println!("  /tools          List currently available tools");
            println!("  /skills browse  Browse the skills hub and official optional skills");
            println!("  /background     Run a prompt in a separate background session");
            println!("  /skin           Show or switch the active CLI skin");
            println!("  /voice on       Enable CLI voice mode");
            println!("  /voice tts      Toggle spoken playback for replies");
            println!("  /reasoning      Set reasoning effort (low, medium, high)");
            println!("  /title          Name the current session");
            println!("  /status         Show session info");
            println!("  /sessions       Open an interactive session picker");
        }
        "/model" => {
            if let Err(e) = crate::commands::model::run_model() {
                println!("Error: {}", e);
            }
        }
        "/tools" => {
            if let Err(e) = crate::commands::tools::run_tools() {
                println!("Error: {}", e);
            }
        }
        "/skills" => {
            if let Err(e) = crate::commands::skills::run_skills() {
                println!("Error: {}", e);
            }
        }
        "/sessions" => {
            if let Err(e) = crate::commands::sessions::run_sessions() {
                println!("Error: {}", e);
            }
        }
        "/reasoning" => {
            if parts.len() > 1 {
                let mut config = athena_core::config::load_config();
                match parts[1].to_lowercase().as_str() {
                    "low" => config.agent.max_iterations = 5,
                    "medium" => config.agent.max_iterations = 20,
                    "high" => config.agent.max_iterations = 50,
                    _ => println!("Invalid level. Use low, medium, or high."),
                }
                if config.agent.max_iterations == 5 || config.agent.max_iterations == 20 || config.agent.max_iterations == 50 {
                    if athena_core::config::save_config(&config).is_ok() {
                        cliclack::note("Reasoning Effort Updated", format!("max_iterations set to {}", config.agent.max_iterations)).unwrap_or(());
                    }
                }
            } else {
                println!("Usage: /reasoning [low|medium|high]");
            }
        }
        "/title" => {
            if parts.len() > 1 {
                state.title = parts[1..].join(" ");
                cliclack::note("Session Renamed", &state.title).unwrap_or(());
            } else {
                println!("Usage: /title <name>");
            }
        }
        "/status" => {
            let status = format!(
                "Title: {}\nModel: {}\nTurns: {}\nVoice: {}\nTTS: {}\nSkin: {}",
                state.title, model_display, state.turn_count, state.voice_mode, state.tts_mode, state.active_skin
            );
            cliclack::note("Session Status", status).unwrap_or(());
        }
        "/voice" => {
            if parts.len() > 1 {
                match parts[1] {
                    "on" => {
                        state.voice_mode = true;
                        cliclack::note("Voice Mode", "Enabled. Press Ctrl+B to record (Requires external bridge)").unwrap_or(());
                    }
                    "off" => {
                        state.voice_mode = false;
                        cliclack::note("Voice Mode", "Disabled").unwrap_or(());
                    }
                    "tts" => {
                        state.tts_mode = !state.tts_mode;
                        cliclack::note("TTS Mode", if state.tts_mode { "Enabled" } else { "Disabled" }).unwrap_or(());
                    }
                    _ => println!("Usage: /voice [on|off|tts]"),
                }
            } else {
                println!("Usage: /voice [on|off|tts]");
            }
        }
        "/skin" => {
            if parts.len() > 1 {
                state.active_skin = parts[1].to_string();
                cliclack::note("Skin Updated", format!("Active skin is now '{}'", state.active_skin)).unwrap_or(());
            } else {
                println!("Usage: /skin <name>");
            }
        }
        "/background" => {
            if parts.len() > 1 {
                let prompt = parts[1..].join(" ");
                let bg_provider = provider.clone();
                let bg_model = agent.model().to_string();
                
                println!("Starting background task...");
                
                tokio::spawn(async move {
                    let mut bg_agent = athena_agent::AIAgent::builder().model(bg_model).build();
                    let empty_registry = athena_tools::ToolRegistry::new();
                    match bg_agent.run_conversation(&prompt, Some("You are a helpful assistant."), &empty_registry, bg_provider).await {
                        Ok(response) => {
                            println!("\n\n[Background Task Completed]\n{}\n", response);
                        }
                        Err(e) => {
                            println!("\n\n[Background Task Error]: {}\n", e);
                        }
                    }
                });
            } else {
                println!("Usage: /background <prompt>");
            }
        }
        _ => {
            println!("Unknown command: {}. Type /help for a list of commands.", cmd);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use athena_providers::LLMProvider;
    use std::sync::Arc;

    struct DummyProvider {
        profile: athena_providers::ProviderProfile,
    }

    #[async_trait::async_trait]
    impl LLMProvider for DummyProvider {
        fn profile(&self) -> &athena_providers::ProviderProfile {
            &self.profile
        }
        
        async fn fetch_models(
            &self,
            _api_key: Option<&str>,
            _timeout: f64,
        ) -> Result<Vec<String>, athena_providers::ProviderError> {
            Ok(vec![])
        }
        
        async fn create_chat_completion(
            &self,
            _request: athena_providers::ChatCompletionRequest,
        ) -> Result<athena_providers::ChatCompletionResponse, athena_providers::ProviderError> {
            Ok(athena_providers::ChatCompletionResponse {
                id: "1".into(),
                model: "dummy".into(),
                choices: vec![],
                usage: None,
                created: 0,
            })
        }
        
        async fn create_chat_completion_stream(
            &self,
            _request: athena_providers::ChatCompletionRequest,
        ) -> Result<athena_providers::ChatCompletionStream, athena_providers::ProviderError> {
            Err(athena_providers::ProviderError::ApiRequestFailed("Not implemented".into()))
        }
    }

    #[tokio::test]
    async fn test_process_slash_command_state() {
        let mut state = SessionState::default();
        let mut agent = athena_agent::AIAgent::builder().model("dummy-model".to_string()).build();
        let provider: Arc<dyn LLMProvider + Send + Sync> = Arc::new(DummyProvider {
            profile: athena_providers::ProviderProfile::new("dummy"),
        });
        
        // Test /title
        process_slash_command("/title", &["/title", "My", "Cool", "Session"], &mut state, &mut agent, provider.clone(), "dummy-display").await;
        assert_eq!(state.title, "My Cool Session");
        
        // Test /voice on
        process_slash_command("/voice", &["/voice", "on"], &mut state, &mut agent, provider.clone(), "dummy-display").await;
        assert!(state.voice_mode);
        
        // Test /voice off
        process_slash_command("/voice", &["/voice", "off"], &mut state, &mut agent, provider.clone(), "dummy-display").await;
        assert!(!state.voice_mode);
        
        // Test /voice tts
        assert!(!state.tts_mode);
        process_slash_command("/voice", &["/voice", "tts"], &mut state, &mut agent, provider.clone(), "dummy-display").await;
        assert!(state.tts_mode);
        process_slash_command("/voice", &["/voice", "tts"], &mut state, &mut agent, provider.clone(), "dummy-display").await;
        assert!(!state.tts_mode);
        
        // Test /skin
        process_slash_command("/skin", &["/skin", "cyberpunk"], &mut state, &mut agent, provider.clone(), "dummy-display").await;
        assert_eq!(state.active_skin, "cyberpunk");
        
        // Run some help/status commands to ensure they don't panic
        process_slash_command("/help", &["/help"], &mut state, &mut agent, provider.clone(), "dummy-display").await;
        process_slash_command("/status", &["/status"], &mut state, &mut agent, provider.clone(), "dummy-display").await;
    }
}

// Rust guideline compliant 2026-02-21
