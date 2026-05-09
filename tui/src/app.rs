use std::time::Duration;

use crate::runtime::Cmd;
use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
};
use reqwest::Client;

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected(String),
}

pub struct Model {
    pub url: String,
    pub connection_status: ConnectionStatus,
    pub should_quit: bool,
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

pub enum Message {
    Key(KeyCode),
    Tick,
    ConnectionResult(Result<(), String>),
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub fn init(url: String) -> (Model, Cmd<Message>) {
    let model = Model {
        connection_status: ConnectionStatus::Connecting,
        should_quit: false,
        url: url.clone(),
    };
    let cmd = Cmd::batch([check_connection(url), tick_after()]);
    (model, cmd)
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

pub fn update(model: &mut Model, msg: Message) -> Cmd<Message> {
    match msg {
        Message::Key(KeyCode::Char('q')) => {
            model.should_quit = true;
            Cmd::none()
        }
        Message::Key(_) => Cmd::none(),
        Message::Tick => Cmd::batch([check_connection(model.url.clone()), tick_after()]),
        Message::ConnectionResult(Ok(())) => {
            model.connection_status = ConnectionStatus::Connected;
            Cmd::none()
        }
        Message::ConnectionResult(Err(e)) => {
            model.connection_status = ConnectionStatus::Disconnected(e);
            Cmd::none()
        }
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(model: &Model, f: &mut Frame) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    let text = match &model.connection_status {
        ConnectionStatus::Connecting => "● Connecting…".to_string(),
        ConnectionStatus::Connected => "● Connected".to_string(),
        ConnectionStatus::Disconnected(msg) => format!("● Disconnected: {msg}"),
    };

    let color = match &model.connection_status {
        ConnectionStatus::Connecting => Color::Yellow,
        ConnectionStatus::Connected => Color::Green,
        ConnectionStatus::Disconnected(_) => Color::Red,
    };

    let status = Paragraph::new(Text::from(text).style(Style::default().fg(color))).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Budgeteur TUI"),
    );

    f.render_widget(status, chunks[0]);
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

const TICK_INTERVAL: Duration = Duration::from_secs(30);

fn check_connection(url: String) -> Cmd<Message> {
    Cmd::from(async move {
        let client = Client::new();
        let result = match client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(format!("server returned {}", resp.status())),
            Err(e) => Err(e.to_string()),
        };
        Message::ConnectionResult(result)
    })
}

fn tick_after() -> Cmd<Message> {
    Cmd::from(async {
        tokio::time::sleep(TICK_INTERVAL).await;
        Message::Tick
    })
}
