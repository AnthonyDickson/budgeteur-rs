mod app;
mod config;
mod dashboard;
mod key_binding;
mod request;
mod runtime;

use std::{io, time::Duration};

use app::Message;
use clap::Parser;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ed25519_dalek::SigningKey;
use rand::Rng;
use ratatui::{Terminal, backend::CrosstermBackend};
use runtime::Runtime;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "budgeteur-tui", about = "TUI client for Budgeteur", version)]
struct Cli {
    /// Server URL (overrides config file).
    #[arg(long)]
    url: Option<String>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Generate an Ed25519 keypair for passwordless auth.
    Init,
    /// Print the config path
    ConfigPath,
}

/// Load the Ed25519 signing key from the XDG data directory.
fn load_signing_key() -> io::Result<SigningKey> {
    let path = config::private_key_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not determine XDG data directory",
        )
    })?;

    let hex_key = match std::fs::read_to_string(&path) {
        Ok(key) => key,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "no private key found at {}. run `budgeteur-tui init` first",
                    path.display()
                ),
            ));
        }
        Err(e) => {
            return Err(io::Error::other(format!(
                "could not read private key from {}: {e}",
                path.display()
            )));
        }
    };

    let raw_key: [u8; 32] = hex::decode(hex_key.trim())
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid hex in private key: {e}"),
            )
        })?
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "private key must be 32 bytes"))?;

    Ok(SigningKey::from_bytes(&raw_key))
}

// ---------------------------------------------------------------------------
// Init: key generation
// ---------------------------------------------------------------------------

fn run_init() -> Result<(), Box<dyn std::error::Error>> {
    let path = config::private_key_path().ok_or("could not determine XDG data directory")?;
    config::ensure_parent_dir(&path)?;

    // Generate fresh Ed25519 keypair.
    let mut seed = [0u8; 32];
    rand::rng().fill_bytes(&mut seed);
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();

    // Write the private key as hex to the data directory.
    std::fs::write(&path, hex::encode(signing_key.to_bytes()))?;

    // Print the public key for the user to copy to the server.
    let pub_key_hex = hex::encode(verifying_key.to_bytes());
    println!("Keypair generated successfully.");
    println!();
    println!("Private key written to: {}", path.display());
    println!();
    println!("Add the following to the server's tui_public_keys.toml:");
    println!();
    println!("[[keys]]");
    println!("label = \"<choose-a-label>\"");
    println!("public_key = \"{pub_key_hex}\"");
    println!();

    Ok(())
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Init) => {
            if let Err(e) = run_init() {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
            Ok(())
        }
        Some(Command::ConfigPath) => {
            config::config_path().inspect(|path| println!("{}", path.to_string_lossy()));

            Ok(())
        }
        None => {
            let cfg = config::Config::load();
            let server_url = cli.url.unwrap_or(cfg.server_url);

            run(server_url).await
        }
    }
}

/// Initialise the Elm-style runtime and run the TUI event loop.
async fn run(server_url: String) -> io::Result<()> {
    let signing_key = load_signing_key()?;

    let (runtime, mut rx) = Runtime::<Message>::new();

    let (mut model, initial_cmd, mut view_state) = app::init(server_url, signing_key);
    runtime.spawn(initial_cmd);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|f| app::view(&model, &mut view_state, f))?;

        if model.should_quit {
            break;
        }

        // Crossterm events
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && let Some(msg) = app::handle_key_event(&mut view_state, key.code)
        {
            let cmd;
            (model, cmd) = app::update(model, msg);
            runtime.spawn(cmd);
        }

        // Completed async commands
        while let Ok(msg) = rx.try_recv() {
            let cmd;
            (model, cmd) = app::update(model, msg);
            runtime.spawn(cmd);
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
