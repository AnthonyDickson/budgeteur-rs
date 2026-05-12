use budgeteur_shared::auth::{TUI_CLIENT_SUB, TuiClaims};
use budgeteur_shared::dashboard::DashboardSummary;

use crate::runtime::Cmd;
use crossterm::event::KeyCode;
use ed25519_dalek::SigningKey;
use ed25519_dalek::pkcs8::EncodePrivateKey;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
};

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

pub enum Status {
    Loading,
    Error(String),
    Ready(DashboardSummary),
}

pub struct Model {
    status: Status,
    pub should_quit: bool,
    #[expect(dead_code)]
    signing_key_der: Vec<u8>,
    #[expect(dead_code)]
    server_url: String,
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

pub enum Message {
    Key(KeyCode),
    DashboardResult(Result<DashboardSummary, String>),
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub fn init(server_url: String, signing_key: SigningKey) -> (Model, Cmd<Message>) {
    let der = signing_key
        .to_pkcs8_der()
        .expect("could not encode signing key to PKCS#8 DER")
        .as_bytes()
        .to_vec();

    let model = Model {
        status: Status::Loading,
        should_quit: false,
        server_url: server_url.clone(),
        signing_key_der: der.clone(),
    };

    let cmd = fetch_dashboard(server_url, der);
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

        Message::DashboardResult(Ok(data)) => {
            model.status = Status::Ready(data);
            Cmd::none()
        }
        Message::DashboardResult(Err(e)) => {
            model.status = Status::Error(e);
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

    let (text, color) = match &model.status {
        Status::Loading => ("● Loading…".to_string(), Color::Yellow),
        Status::Ready(data) => (
            format!(
                "● Balance: ${:.2}  |  Net: ${:.2}/mo",
                data.total_balance, data.monthly_net
            ),
            Color::Green,
        ),
        Status::Error(msg) => (format!("● {msg}"), Color::Red),
    };

    let status = Paragraph::new(Text::from(text).style(Style::default().fg(color))).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Budgeteur TUI"),
    );

    f.render_widget(status, chunks[0]);

    if let Status::Ready(data) = &model.status {
        let details = format!(
            "Total Balance: ${:.2}\n\nLast Month:\n  Income:   ${:.2}\n  Expenses: ${:.2}\n  Net:      ${:.2}",
            data.total_balance, data.monthly_income, data.monthly_expenses, data.monthly_net
        );
        let detail_widget = Paragraph::new(details)
            .block(Block::default().borders(Borders::ALL).title("Dashboard"));
        f.render_widget(detail_widget, chunks[1]);
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

const TOKEN_EXPIRY_SECONDS: usize = 300;

fn fetch_dashboard(server_url: String, signing_key_der: Vec<u8>) -> Cmd<Message> {
    Cmd::from(async move {
        let header = match sign_auth_header(&signing_key_der) {
            Ok(h) => h,
            Err(e) => return Message::DashboardResult(Err(e)),
        };

        let client = reqwest::Client::new();
        let url = format!("{server_url}{}", budgeteur_shared::routes::DASHBOARD);
        match client
            .get(&url)
            .header("Authorization", &header)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => match resp.json::<DashboardSummary>().await {
                Ok(data) => Message::DashboardResult(Ok(data)),
                Err(e) => Message::DashboardResult(Err(format!("could not parse dashboard: {e}"))),
            },
            Ok(resp) if resp.status().as_u16() == 401 => Message::DashboardResult(Err(
                "authentication failed — is your public key registered on the server?".into(),
            )),
            Ok(resp) => Message::DashboardResult(Err(format!("server returned {}", resp.status()))),
            Err(e) => Message::DashboardResult(Err(format!("connection error: {e}"))),
        }
    })
}

fn sign_auth_header(signing_key_der: &[u8]) -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock error: {e}"))?
        .as_secs() as usize;

    let claims = TuiClaims {
        sub: TUI_CLIENT_SUB.into(),
        iat: now,
        exp: now + TOKEN_EXPIRY_SECONDS,
    };

    let encoding_key = jsonwebtoken::EncodingKey::from_ed_der(signing_key_der);

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::EdDSA),
        &claims,
        &encoding_key,
    )
    .map_err(|e| format!("could not sign JWT: {e}"))?;

    Ok(format!("Bearer {token}"))
}
