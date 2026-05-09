mod app;
mod runtime;

use std::{io, time::Duration};

use app::{Message, init, update, view};
use clap::Parser;
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use runtime::Runtime;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "budgeteur-tui", about = "TUI client for Budgeteur")]
struct Cli {
    #[arg(long, default_value = "http://localhost:8080")]
    url: String,
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let (runtime, mut rx) = Runtime::<Message>::new();

    let (mut model, initial_cmd) = init(cli.url);
    runtime.spawn(initial_cmd);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    loop {
        terminal.draw(|f| view(&model, f))?;

        if model.should_quit {
            break;
        }

        // Crossterm events
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            runtime.spawn(update(&mut model, Message::Key(key.code)));
        }

        // Completed async commands
        while let Ok(msg) = rx.try_recv() {
            runtime.spawn(update(&mut model, msg));
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
