mod interactive;
mod commands;
pub mod context;
pub mod logger;

use clap::Parser;
use clap::Subcommand;
use athena_agent::AIAgent;
use athena_core::logging::{setup_logging, LoggingConfig, Mode};
use athena_tools::ToolRegistry;
use std::path::PathBuf;
use std::collections::HashSet;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Provide a custom workspace path
    #[arg(short = 'W', long)]
    workspace: Option<PathBuf>,

    /// Model to use (default: gpt-4o)
    #[arg(long, short = 'm')]
    model: Option<String>,

    /// API key for authentication
    #[arg(long)]
    api_key: Option<String>,

    /// Base URL for the API
    #[arg(long)]
    base_url: Option<String>,

    /// Maximum tool-calling iterations (default: 20)
    #[arg(long)]
    max_turns: Option<u32>,

    /// One-shot mode: send a single prompt and print ONLY the final response text to stdout
    #[arg(long, short = 'z')]
    oneshot: Option<String>,

    /// Comma-separated toolsets to enable for this invocation
    #[arg(long, short = 't')]
    toolsets: Option<String>,

    /// Preload one or more skills for the session
    #[arg(long, short = 's')]
    skills: Option<String>,

    /// Resume a previous session by ID or title
    #[arg(long, short = 'r')]
    resume: Option<String>,

    /// Resume a session by name, or the most recent if no name given
    #[arg(long, short = 'c')]
    continue_session: Option<Option<String>>,

    /// Run in an isolated git worktree (for parallel agents)
    #[arg(long, short = 'w')]
    worktree: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum GatewayCommands {
    /// Install gateway as a user-level systemd service
    Install,
    /// Start the gateway service
    Start,
    /// Stop the gateway service
    Stop,
    /// Check gateway service status
    Status,
    /// View gateway service logs
    Logs,
    /// Run the gateway service in the foreground
    Run,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Interactive chat with the agent
    Chat,

    /// Select default model and provider
    Model,

    /// Manage fallback providers (tried when the primary model fails)
    Fallback,

    /// Messaging gateway management
    Gateway {
        #[command(subcommand)]
        command: Option<GatewayCommands>,
    },

    /// Language Server Protocol management
    Lsp,

    /// Interactive setup wizard
    Setup,

    /// Set up WhatsApp integration
    Whatsapp,

    /// Slack integration helpers (manifest generation, etc.)
    Slack,

    /// Authenticate with an inference provider
    Login,

    /// Clear authentication for an inference provider
    Logout,

    /// Manage pooled provider credentials
    Auth,

    /// Show status of all components
    Status,

    /// Cron job management
    Cron,

    /// Manage dynamic webhook subscriptions
    Webhook,

    /// Multi-profile collaboration board (tasks, links, comments)
    Kanban,

    /// Inspect and manage shell-script hooks
    Hooks,

    /// Check configuration and dependencies
    Doctor,

    /// Dump setup summary for support/debugging
    Dump,

    /// Debug tools — upload logs and system info for support
    Debug,

    /// Back up Athena home directory to a zip file
    Backup,

    /// Inspect / prune / clear ~/.athena/checkpoints/
    Checkpoints,

    /// Restore a Athena backup from a zip file
    Import,

    /// View and edit configuration
    Config,

    /// Manage DM pairing codes for user authorization
    Pairing,

    /// Search, install, configure, and manage skills
    Skills,

    /// Manage plugins — install, update, remove, list
    Plugins,

    /// Background skill maintenance (curator) — status, run, pause, pin
    Curator,

    /// Configure external memory provider or edit memory file
    Memory {
        #[command(subcommand)]
        command: Option<MemoryCommands>,
    },

    /// Configure which tools are enabled per platform
    Tools,

    /// Manage the Computer Use (cua-driver) backend (macOS)
    ComputerUse,

    /// Manage MCP servers and run Athena as an MCP server
    Mcp {
        #[arg(long)]
        serve: bool,
    },

    /// Manage session history (list, rename, export, prune, delete)
    Sessions,

    /// Show usage insights and analytics
    Insights,

    /// OpenClaw migration tools
    Claw,

    /// Show version information
    Version,

    /// Update Athena to the latest version
    Update,

    /// Uninstall Athena
    Uninstall,

    /// Run Athena as an ACP (Agent Client Protocol) server
    Acp,

    /// Manage profiles — multiple isolated Athena instances
    Profile,

    /// Print shell completion script (bash, zsh, or fish)
    Completion,

    /// Start the web UI dashboard
    Dashboard,

    /// View and filter Athena log files
    Logs,

    /// Run a single query
    #[command(alias = "q")]
    Query {
        /// The query to run
        query: String,

        /// Comma-separated list of toolsets to enable
        #[arg(long, short = 't')]
        toolsets: Option<String>,

        /// Comma-separated list of skills to preload
        #[arg(long, short = 's')]
        skills: Option<String>,
    },

    /// List available tools
    ListTools,

    /// List available toolsets
    ListToolsets,

    /// Show configuration
    ConfigShow,

    /// Export a session trajectory to a JSON file
    Trajectory {
        #[command(subcommand)]
        command: Option<TrajectoryCommands>,
    },

    /// Run the agent headlessly through a list of prompts
    Batch {
        /// Run a batch configuration
        #[arg(long)]
        config: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum MemoryCommands {
    /// Edit the local MEMORY.md file
    Edit,
}

#[derive(Subcommand, Debug)]
pub enum TrajectoryCommands {
    /// Export a specific session trajectory
    Export {
        #[arg(long)]
        session: String,
        #[arg(long)]
        output: Option<String>,
    },
}

pub(crate) fn create_agent_builder(config: &athena_core::config::AthenaConfig, args: &Args) -> (athena_agent::AIAgentBuilder, std::sync::Arc<dyn athena_providers::LLMProvider + Send + Sync>) {
    let mut builder = AIAgent::builder();
    // Set model if provided globally
    if let Some(model) = &args.model {
        builder = builder.model(model);
    } else {
        if !config.model.default.is_empty() {
            builder = builder.model(&config.model.default);
        } else {
            builder = builder.model("gpt-4o");
        }
    }

    // Set max iterations if provided globally
    if let Some(max_turns) = args.max_turns {
        builder = builder.max_iterations(max_turns as usize);
    } else {
        builder = builder.max_iterations(20);
    }

    // Initialize provider registry
    athena_providers::registry::init_builtin_providers();
    let provider_slug = if config.model.provider.is_empty() { "openai" } else { &config.model.provider };
    
    // Resolve API Key and Base URL using the provider registry
    let mut resolved_api_key = args.api_key.clone().or_else(|| std::env::var("OPENAI_API_KEY").ok());
    let mut resolved_base_url = args.base_url.clone();
    
    if let Some(profile) = athena_providers::registry::get_provider_profile(provider_slug) {
        if resolved_base_url.is_none() {
            resolved_base_url = Some(profile.base_url.clone());
        }
        
        if resolved_api_key.is_none() {
            for env_var in &profile.env_vars {
                if let Some(val) = athena_core::config::get_env_value(env_var) {
                    resolved_api_key = Some(val);
                    break;
                }
            }
        }
    }

    if let Some(k) = resolved_api_key {
        builder = builder.api_key(k);
    }

    if let Some(url) = resolved_base_url {
        builder = builder.base_url(url);
    }

    let provider = athena_providers::registry::get_provider(provider_slug)
        .unwrap_or_else(|| athena_providers::registry::get_provider("openai").unwrap());

    if let Ok(db) = athena_state::db::SessionDB::new(None) {
        let logger = crate::logger::DbSessionLogger {
            db: std::sync::Arc::new(db),
            model: args.model.clone().unwrap_or_else(|| "gpt-4o".to_string()),
        };
        builder = builder.logger(std::sync::Arc::new(logger));
    }

    (builder, provider)
}

#[tokio::main]
async fn main() {
    // Load environment variables (.env file)
    let _ = dotenvy::dotenv();

    let args = Args::parse();

    // Setup logging
    let _log_dir = setup_logging(LoggingConfig {
        mode: Some(Mode::Cli),
        ..Default::default()
    });

    // Load default config
    let config = athena_core::config::load_config();

    if let Err(e) = execute_command(args, config).await {
        eprintln!("Fatal error: {}", e);
        std::process::exit(1);
    }
}

pub(crate) async fn execute_command(args: Args, config: athena_core::config::AthenaConfig) -> anyhow::Result<()> {
    let (builder, provider) = create_agent_builder(&config, &args);

    match &args.command {
        Some(Commands::Query { query, toolsets, skills }) => {
            let athena_home = athena_core::paths::get_athena_home();
            let db_path = athena_home.join("skills.db");
            
            let mut builder_with_skills = builder;
            if let (Ok(store), Ok(manager)) = (
                athena_skills::SkillStore::new(&db_path),
                athena_skills::SkillManager::new(&db_path)
            ) {
                builder_with_skills = builder_with_skills.skills(
                    std::sync::Arc::new(store),
                    std::sync::Arc::new(manager)
                );
            }
            
            let mut agent = builder_with_skills.build();
            let registry = ToolRegistry::new();
            commands::mcp::load_mcp_servers_into_registry(&registry).await;

            // Process toolsets if provided
            if let Some(toolsets_str) = toolsets {
                println!("Toolsets specified: {}", toolsets_str);
            } else if let Some(global_toolsets) = &args.toolsets {
                println!("Toolsets specified: {}", global_toolsets);
            }

            // Process skills if provided
            if let Some(skills_str) = skills {
                println!("Skills specified: {}", skills_str);
            } else if let Some(global_skills) = &args.skills {
                println!("Skills specified: {}", global_skills);
            }
            println!("Query: {}", query);
            
            let system_prompt = crate::context::build_system_prompt();
            match agent.run_conversation(query, Some(&system_prompt), &registry, provider).await {
                Ok(response) => {
                    println!("{}", response);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::ListTools) => {
            let registry = ToolRegistry::new();
            println!("\nAvailable Tools:");
            println!("═════════════════");
            let tools = registry.get_all_tools().await;
            for tool in tools {
                println!("- {:<20} [{}]", tool.name(), tool.toolset());
            }
        }
        Some(Commands::ListToolsets) => {
            let registry = ToolRegistry::new();
            let tools = registry.get_all_tools().await;
            let mut toolsets = HashSet::new();
            for tool in tools {
                toolsets.insert(tool.toolset());
            }

            println!("\nAvailable Toolsets:");
            println!("════════════════════");
            let mut ts_vec: Vec<_> = toolsets.into_iter().collect();
            ts_vec.sort();
            for ts in ts_vec {
                println!("- {}", ts);
            }
        }
        Some(Commands::ConfigShow) => {
            if let Err(e) = commands::config::run_config_show() {
                eprintln!("Error: {}", e);
            }
        }

        Some(Commands::Chat) => {
            // Start interactive chat session
            let athena_home = athena_core::paths::get_athena_home();
            let db_path = athena_home.join("skills.db");
            
            let mut builder_with_skills = builder;
            if let (Ok(store), Ok(manager)) = (
                athena_skills::SkillStore::new(&db_path),
                athena_skills::SkillManager::new(&db_path)
            ) {
                builder_with_skills = builder_with_skills.skills(
                    std::sync::Arc::new(store),
                    std::sync::Arc::new(manager)
                );
            }
            
            let agent = builder_with_skills.build();
            let registry = ToolRegistry::new();
            commands::mcp::load_mcp_servers_into_registry(&registry).await;
            interactive::run_interactive_loop(agent, &registry, provider).await;
        }
        Some(Commands::Model) => {
            if let Err(e) = commands::model::run_model() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Fallback) => {
            if let Err(e) = commands::fallback::run_fallback() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Gateway { command }) => {
            commands::gateway::run_gateway(command);
        }
        Some(Commands::Lsp) => {
            commands::lsp::run_lsp();
        }
        Some(Commands::Setup) => {
            if let Err(e) = commands::setup::run_setup() {
                eprintln!("Setup failed: {}", e);
            }
        }
        Some(Commands::Whatsapp) => {
            if let Err(e) = commands::whatsapp::run_whatsapp() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Slack) => {
            if let Err(e) = commands::slack::run_slack() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Login) => {
            if let Err(e) = commands::login::run_login() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Logout) => {
            if let Err(e) = commands::login::run_logout() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Auth) => {
            if let Err(e) = commands::auth::run_auth() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Status) => {
            commands::status::run_status();
        }
        Some(Commands::Cron) => {
            if let Err(e) = commands::cron::run_cron() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Webhook) => {
            if let Err(e) = commands::webhook::run_webhook() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Kanban) => {
            if let Err(e) = commands::kanban::run_kanban() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Hooks) => {
            if let Err(e) = commands::hooks::run_hooks() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Doctor) => {
            commands::doctor::run_doctor();
        }
        Some(Commands::Dump) => {
            commands::dump::run_dump();
        }
        Some(Commands::Debug) => {
            commands::debug::run_debug();
        }
        Some(Commands::Backup) => {
            if let Err(e) = commands::backup::run_backup() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Checkpoints) => {
            commands::checkpoints::run_checkpoints();
        }
        Some(Commands::Import) => {
            if let Err(e) = commands::import::run_import() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Config) => {
            if let Err(e) = commands::config::run_config_edit() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Pairing) => {
            if let Err(e) = commands::pairing::run_pairing() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Skills) => {
            if let Err(e) = commands::skills::run_skills() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Plugins) => {
            commands::plugins::run_plugins();
        }
        Some(Commands::Curator) => {
            commands::curator::run_curator();
        }
        Some(Commands::Memory { command }) => {
            if let Some(MemoryCommands::Edit) = command {
                if let Err(e) = commands::memory::run_memory_edit() {
                    eprintln!("Error: {}", e);
                }
            } else {
                if let Err(e) = commands::memory::run_memory() {
                    eprintln!("Error: {}", e);
                }
            }
        }
        Some(Commands::Tools) => {
            if let Err(e) = commands::tools::run_tools() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::ComputerUse) => {
            commands::computer_use::run_computer_use();
        }
        Some(Commands::Mcp { serve }) => {
            if *serve {
                let registry = std::sync::Arc::new(ToolRegistry::new());
                commands::mcp::serve_mcp(registry).await;
            } else {
                commands::mcp::run_mcp();
            }
        }
        Some(Commands::Sessions) => {
            if let Err(e) = commands::sessions::run_sessions() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Insights) => {
            commands::insights::run_insights();
        }
        Some(Commands::Claw) => {
            commands::claw::run_claw();
        }
        Some(Commands::Version) => {
            commands::version::run_version();
        }
        Some(Commands::Update) => {
            commands::update::run_update();
        }
        Some(Commands::Uninstall) => {
            if let Err(e) = commands::uninstall::run_uninstall() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Acp) => {
            commands::acp::run_acp();
        }
        Some(Commands::Profile) => {
            commands::profile::run_profile();
        }
        Some(Commands::Completion) => {
            commands::completion::run_completion();
        }
        Some(Commands::Dashboard) => {
            commands::dashboard::run_dashboard().await;
        }
        Some(Commands::Logs) => {
            if let Err(e) = commands::logs::run_logs() {
                eprintln!("Error: {}", e);
            }
        }
        Some(Commands::Trajectory { command }) => {
            if let Some(TrajectoryCommands::Export { session, output }) = command {
                if let Err(e) = commands::trajectory::run_trajectory_export(session.clone(), output.clone()).await {
                    eprintln!("Error exporting trajectory: {}", e);
                }
            }
        }
        Some(Commands::Batch { config: batch_config }) => {
            let agent = builder.build();
            let registry = ToolRegistry::new();
            commands::mcp::load_mcp_servers_into_registry(&registry).await;
            if let Err(e) = commands::batch::run_batch(batch_config.clone(), agent, registry, provider).await {
                eprintln!("Error running batch: {}", e);
            }
        }
        None => {
            // Run interactive mode
            if let Some(oneshot) = &args.oneshot {
                // One-shot mode
                let athena_home = athena_core::paths::get_athena_home();
                let db_path = athena_home.join("skills.db");
                
                let mut builder_with_skills = builder;
                if let (Ok(store), Ok(manager)) = (
                    athena_skills::SkillStore::new(&db_path),
                    athena_skills::SkillManager::new(&db_path)
                ) {
                    builder_with_skills = builder_with_skills.skills(
                        std::sync::Arc::new(store),
                        std::sync::Arc::new(manager)
                    );
                }
                
                let mut agent = builder_with_skills.build();
                let registry = ToolRegistry::new();
                commands::mcp::load_mcp_servers_into_registry(&registry).await;

                let system_prompt = crate::context::build_system_prompt();
                match agent.run_conversation(oneshot, Some(&system_prompt), &registry, provider).await {
                    Ok(response) => {
                        println!("{}", response);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                }
            } else if args.resume.is_some() || args.continue_session.is_some() {
                println!("Session resume is currently under development (Phase 5). To view sessions, use 'athena sessions'.");
            } else if args.worktree {
                println!("Worktree isolation mode is currently under development (Phase 7).");
            } else {
                // Regular interactive mode
                let athena_home = athena_core::paths::get_athena_home();
                let db_path = athena_home.join("skills.db");
                
                let mut builder_with_skills = builder;
                if let (Ok(store), Ok(manager)) = (
                    athena_skills::SkillStore::new(&db_path),
                    athena_skills::SkillManager::new(&db_path)
                ) {
                    builder_with_skills = builder_with_skills.skills(
                        std::sync::Arc::new(store),
                        std::sync::Arc::new(manager)
                    );
                }
                
                let agent = builder_with_skills.build();
                let registry = ToolRegistry::new();
                commands::mcp::load_mcp_servers_into_registry(&registry).await;
                interactive::run_interactive_loop(agent, &registry, provider).await;
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use athena_core::config::{AthenaConfig, ModelConfig};

    #[test]
    fn test_mistral_config_injects_correct_base_url() {
        let mut config = AthenaConfig::default();
        config.model = ModelConfig {
            default: "mistral-large-latest".to_string(),
            provider: "mistral".to_string(),
        };

        // Create empty args
        let args = Args {
            workspace: None,
            model: None,
            api_key: Some("dummy_mistral_key".to_string()),
            base_url: None,
            max_turns: None,
            oneshot: None,
            toolsets: None,
            skills: None,
            resume: None,
            continue_session: None,
            worktree: false,
            command: None,
        };

        let builder = create_agent_builder(&config, &args);
        let agent = builder.0.build();

        assert_eq!(agent.base_url(), Some("https://api.mistral.ai/v1"));
        assert_eq!(agent.api_key(), Some("dummy_mistral_key"));
    }

    #[tokio::test]
    async fn test_end_to_end_mocked_provider_test() {
        use wiremock::matchers::{method, path, header, body_partial_json};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use athena_tools::ToolRegistry;

        // 1. Start a local mock server
        let mock_server = MockServer::start().await;

        // 2. Set up the mock for non-streaming (compression)
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("Authorization", "Bearer dummy_mistral_key"))
            .and(body_partial_json(serde_json::json!({"stream": false})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "mock_id",
                "object": "chat.completion",
                "created": 12345,
                "model": "mistral-large-latest",
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Mock summary"
                    },
                    "finish_reason": "stop"
                }]
            })))
            .mount(&mock_server)
            .await;

        // 3. Set up the mock for streaming (main request)
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("Authorization", "Bearer dummy_mistral_key"))
            .respond_with(ResponseTemplate::new(200)
                .set_body_bytes(b"data: {\"id\":\"mock_id\",\"object\":\"chat.completion.chunk\",\"created\":12345,\"model\":\"mistral-large-latest\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Hello from mock Mistral!\"},\"finish_reason\":null}]}\n\ndata: {\"id\":\"mock_id\",\"object\":\"chat.completion.chunk\",\"created\":12345,\"model\":\"mistral-large-latest\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n".to_vec())
                .insert_header("Content-Type", "text/event-stream"))
            .mount(&mock_server)
            .await;

        // 3. Configure the CLI
        let mut config = AthenaConfig::default();
        config.model = ModelConfig {
            default: "mistral-large-latest".to_string(),
            provider: "mistral".to_string(),
        };

        // We use Args to override the base URL to our mock server
        let args = Args {
            workspace: None,
            model: None,
            api_key: Some("dummy_mistral_key".to_string()),
            base_url: Some(format!("{}/v1", mock_server.uri())),
            max_turns: None,
            oneshot: None,
            toolsets: None,
            skills: None,
            resume: None,
            continue_session: None,
            worktree: false,
            command: None,
        };

        // 4. Build the agent
        let (mut builder, provider) = create_agent_builder(&config, &args);
        // Force the max_iterations to 1 to prevent runaway
        builder = builder.max_iterations(1);
        let mut agent = builder.build();

        // 5. Run a conversation and verify the response!
        let registry = ToolRegistry::new();
        let response = agent.run_conversation("Say hi", None, &registry, provider).await.unwrap();

        assert_eq!(response, "Hello from mock Mistral!");
    }

    #[test]
    fn test_cli_parsing() {
        use clap::Parser;

        // Test normal chat
        let args = Args::parse_from(&["athena", "chat"]);
        assert!(matches!(args.command, Some(Commands::Chat)));

        // Test query with toolsets
        let args = Args::parse_from(&["athena", "query", "what is 2+2", "-t", "math,web"]);
        match args.command.unwrap() {
            Commands::Query { query, toolsets, skills } => {
                assert_eq!(query, "what is 2+2");
                assert_eq!(toolsets.unwrap(), "math,web");
                assert!(skills.is_none());
            }
            _ => panic!("Expected Query command"),
        }

        // Test gateway commands
        let args = Args::parse_from(&["athena", "gateway", "start"]);
        match args.command.unwrap() {
            Commands::Gateway { command: Some(GatewayCommands::Start) } => {}
            _ => panic!("Expected Gateway Start command"),
        }

        // Test global flags
        let args = Args::parse_from(&["athena", "--model", "gpt-4", "chat"]);
        assert_eq!(args.model.unwrap(), "gpt-4");
        assert!(matches!(args.command, Some(Commands::Chat)));
    }

    #[tokio::test]
    async fn test_execute_command_non_interactive() {
        use clap::Parser;
        use athena_core::config::AthenaConfig;

        let config = AthenaConfig::default();
        
        // Test ListTools
        let args = Args::parse_from(&["athena", "list-tools"]);
        let res = execute_command(args, config.clone()).await;
        assert!(res.is_ok());

        // Test ListToolsets
        let args = Args::parse_from(&["athena", "list-toolsets"]);
        let res = execute_command(args, config.clone()).await;
        assert!(res.is_ok());

        // Test ConfigShow
        let args = Args::parse_from(&["athena", "config-show"]);
        let res = execute_command(args, config.clone()).await;
        assert!(res.is_ok());

        // Test Version
        let args = Args::parse_from(&["athena", "version"]);
        let res = execute_command(args, config.clone()).await;
        assert!(res.is_ok());
    }
}

// Rust guideline compliant 2026-02-21
