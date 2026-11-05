use aether_agent::{Agent, Event};
use aether_core::{
    Mode,
    config::{Config, config_dir, data_dir},
};
use aether_providers::OpenAiCompatible;
use aether_storage::Store;
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use std::{
    io::{self, IsTerminal, Write},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(
    name = "aether",
    version,
    about = "Local-first terminal coding agent (early development)"
)]
struct Cli {
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(long, global = true)]
    session: Option<String>,
    #[arg(long, global = true, value_enum)]
    mode: Option<CliMode>,
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Clone, ValueEnum)]
enum CliMode {
    Ask,
    Plan,
    Build,
    Debug,
    Review,
}
impl From<CliMode> for Mode {
    fn from(m: CliMode) -> Self {
        match m {
            CliMode::Ask => Self::Ask,
            CliMode::Plan => Self::Plan,
            CliMode::Build => Self::Build,
            CliMode::Debug => Self::Debug,
            CliMode::Review => Self::Review,
        }
    }
}
#[derive(Subcommand)]
enum Commands {
    Chat,
    Run {
        prompt: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        allow_read: bool,
    },
    Sessions {
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        json: bool,
    },
    Export {
        id: String,
    },
    Fork {
        id: String,
        title: String,
    },
    Models,
    Doctor,
    Config {
        #[arg(long)]
        init: bool,
    },
}
#[tokio::main]
async fn main() {
    if let Err(e) = entry().await {
        eprintln!("aether: {e:#}");
        std::process::exit(1);
    }
}
async fn entry() -> Result<()> {
    let cli = Cli::parse();
    let workspace = cli
        .workspace
        .unwrap_or(std::env::current_dir()?)
        .canonicalize()
        .context("workspace does not exist")?;
    if !workspace.is_dir() {
        bail!("workspace must be a directory");
    }
    let mut config = Config::load(&workspace, cli.config.as_deref())?;
    if let Some(mode) = cli.mode {
        config.agent.mode = mode.into();
    }
    if let Some(Commands::Config { init }) = &cli.command {
        if *init {
            let p = cli.config.unwrap_or(config_dir().join("config.toml"));
            config.save_new(&p)?;
            println!("{}", p.display());
        } else {
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
        return Ok(());
    }
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("aether.log"))?;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "aether=info".into()),
        )
        .with_writer(log)
        .try_init()
        .map_err(|e| anyhow::anyhow!("initialize logging: {e}"))?;
    let store = Store::open(dir.join("sessions.db")).await?;
    let provider = Arc::new(OpenAiCompatible::new(config.provider.clone())?);
    match cli.command {
        Some(Commands::Sessions { search, json }) => {
            let sessions = store.list(search).await?;
            if json {
                println!("{}", serde_json::to_string(&sessions)?);
            } else {
                for s in sessions {
                    println!("{}  {}  {}", s.id, s.title, s.workspace);
                }
            }
        }
        Some(Commands::Export { id }) => println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"version":1,"session":store.get(&id).await?,"events":store.timeline(&id).await?})
            )?
        ),
        Some(Commands::Fork { id, title }) => println!("{}", store.fork(&id, &title).await?),
        Some(Commands::Models) => {
            for model in provider.models().await? {
                println!("{model}");
            }
        }
        Some(Commands::Doctor) => {
            println!(
                "workspace: {}\ndatabase: {}\nmodel: {}\nmode: {:?}\nremote authorized: {}",
                workspace.display(),
                dir.join("sessions.db").display(),
                config.provider.model,
                config.agent.mode,
                config.provider.allow_remote
            );
            let models = provider.models().await.context(
                "provider unavailable; start LM Studio/Ollama/llama.cpp and configure the model",
            )?;
            println!("provider reachable: {} model(s)", models.len());
            if !models.contains(&config.provider.model) {
                bail!("configured model not advertised; choose one from `aether models`");
            }
        }
        Some(Commands::Run {
            prompt,
            json,
            allow_read,
        }) => {
            if allow_read {
                config.permissions.read = aether_core::Rule::Allow;
            }
            let s = get_session(&store, cli.session, &workspace, &config, &prompt).await?;
            let agent = Agent {
                config,
                workspace,
                store,
                provider,
            };
            headless(agent, s.id, prompt, json).await?;
        }
        None | Some(Commands::Chat) => {
            let s = get_session(&store, cli.session, &workspace, &config, "New session").await?;
            aether_tui::run(
                Agent {
                    config,
                    workspace,
                    store,
                    provider,
                },
                s,
            )
            .await?;
        }
        Some(Commands::Config { .. }) => unreachable!(),
    }
    Ok(())
}
async fn get_session(
    store: &Store,
    id: Option<String>,
    workspace: &std::path::Path,
    config: &Config,
    title: &str,
) -> Result<aether_core::Session> {
    if let Some(id) = id {
        let s = store.get(&id).await?;
        if std::path::Path::new(&s.workspace) != workspace {
            bail!("session workspace mismatch: reopen in {}", s.workspace);
        }
        if s.model != config.provider.model {
            bail!(
                "session model is {}; configure that model before resuming (model switch migration not implemented)",
                s.model
            );
        }
        Ok(s)
    } else {
        store.create(workspace, &config.provider.model, title).await
    }
}
async fn headless(agent: Agent, id: String, prompt: String, json_output: bool) -> Result<()> {
    let cancel = CancellationToken::new();
    let (tx, mut rx) = mpsc::channel(64);
    let c = cancel.clone();
    let task = tokio::spawn(async move { agent.run(id, prompt, tx, c).await });
    let mut interrupt = false;
    loop {
        tokio::select! {
            _=tokio::signal::ctrl_c(),if !interrupt=>{cancel.cancel();interrupt=true;},
            event=rx.recv()=>{let Some(event)=event else{break};match event {
                Event::Text(t)=>if json_output{println!("{}",serde_json::json!({"type":"text","text":t}));}else{print!("{t}");io::stdout().flush()?;},
                Event::Permission{preview,reply}=>{
                    // JSON/noninteractive mode never reads stdin or assumes consent.
                    if json_output||!io::stdin().is_terminal(){eprintln!("permission denied (noninteractive): {preview}");let _=reply.send(false);}
                    else{eprintln!("\n{preview}\nApprove once? [y/N]");let approved=approval(&cancel).await?;let _=reply.send(approved);}
                },
                Event::Tool{name,detail}=>if json_output{println!("{}",serde_json::json!({"type":"tool","name":name,"detail":detail}));}else{eprintln!("\n[{name}] {detail}");},
                Event::State(s)=>if json_output{println!("{}",serde_json::json!({"type":"state","state":s}));},
                Event::Usage(u)=>if json_output{println!("{}",serde_json::json!({"type":"usage","usage":u}));},
                Event::Finished{error}=>if json_output{println!("{}",serde_json::json!({"type":"finished","error":error}));},
            }}
        }
    }
    task.await.context("agent task panicked")??;
    if !json_output {
        println!();
    }
    Ok(())
}

async fn approval(cancel: &CancellationToken) -> Result<bool> {
    use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
    use futures_util::StreamExt;
    crossterm::terminal::enable_raw_mode()?;
    struct RawGuard;
    impl Drop for RawGuard {
        fn drop(&mut self) {
            let _ = crossterm::terminal::disable_raw_mode();
        }
    }
    let _guard = RawGuard;
    let mut input = EventStream::new();
    loop {
        tokio::select! {_=cancel.cancelled()=>return Ok(false),_=tokio::signal::ctrl_c()=>{cancel.cancel();return Ok(false);},event=input.next()=>{
            match event {Some(Ok(Event::Key(key)))=>{if key.code==KeyCode::Char('c')&&key.modifiers.contains(KeyModifiers::CONTROL){cancel.cancel();return Ok(false);}return Ok(key.code==KeyCode::Char('y'));},Some(Err(e))=>return Err(e.into()),None=>return Ok(false),_=>{}}
        }}
    }
}
