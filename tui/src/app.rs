use crossterm::event::KeyCode;
use ed25519_dalek::{SigningKey, pkcs8::EncodePrivateKey};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Wrap},
};

use crate::{dashboard, key_binding::KeyBinding, request::RequestContext, runtime::Cmd};

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

pub enum PageModel {
    Dashboard(dashboard::Model),
}

pub struct Model {
    pub should_quit: bool,
    #[expect(dead_code)]
    request_ctx: RequestContext,
    page: PageModel,
}

/// Ratatui widget state for the active page, kept separate from [`Model`]
/// to preserve TEA purity.
pub enum ViewState {
    Dashboard(dashboard::ViewState),
}

// ---------------------------------------------------------------------------
// Message
// ---------------------------------------------------------------------------

#[allow(clippy::large_enum_variant)]
pub enum Message {
    Quit,
    DashboardMsg(dashboard::Message),
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

pub fn init(server_url: String, signing_key: SigningKey) -> (Model, Cmd<Message>, ViewState) {
    let der = signing_key
        .to_pkcs8_der()
        .expect("could not encode signing key to PKCS#8 DER")
        .as_bytes()
        .to_vec();

    let request_ctx = RequestContext {
        base_url: server_url,
        signing_key_der: der,
    };
    let (dashboard_model, dashboard_cmd, dashboard_view_state) = dashboard::init(&request_ctx);

    let model = Model {
        should_quit: false,
        request_ctx,
        page: PageModel::Dashboard(dashboard_model),
    };

    let cmd = Cmd::map(dashboard_cmd, Message::DashboardMsg);
    (model, cmd, ViewState::Dashboard(dashboard_view_state))
}

// ---------------------------------------------------------------------------
// Update
// ---------------------------------------------------------------------------

pub fn update(mut model: Model, msg: Message) -> (Model, Cmd<Message>) {
    match msg {
        Message::Quit => {
            model.should_quit = true;
            (model, Cmd::none())
        }
        Message::DashboardMsg(inner_msg) => match model.page {
            PageModel::Dashboard(inner_model) => {
                // TODO: pass request ctx
                let (updated_model, cmd) = dashboard::update(inner_model, inner_msg);
                model.page = PageModel::Dashboard(updated_model);
                (model, cmd.map(Message::DashboardMsg))
            }
        },
    }
}

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

pub fn view(model: &Model, view_state: &mut ViewState, f: &mut Frame) {
    let area = f.area();
    let [page_area, status_bar_area] =
        Layout::vertical([Constraint::Min(20), Constraint::Length(2)]).areas(area);

    match (&model.page, &mut *view_state) {
        (PageModel::Dashboard(dashboard_model), ViewState::Dashboard(inner_view_state)) => {
            dashboard::view(dashboard_model, inner_view_state, page_area, f)
        }
    }

    let mut bindings = match (&model.page, &view_state) {
        (PageModel::Dashboard(_), ViewState::Dashboard(inner_view_state)) => {
            dashboard::key_bindings(inner_view_state)
        }
    };
    bindings.extend(global_key_bindings());
    draw_status_bar(bindings, status_bar_area, f);
}

fn draw_status_bar(bindings: Vec<KeyBinding>, area: Rect, f: &mut Frame) {
    let mut spans: Vec<Span> = bindings
        .into_iter()
        .flat_map(|b| {
            [
                b.key.gray(),
                " ".into(),
                b.description.dark_gray(),
                " • ".dark_gray(),
            ]
        })
        .collect();

    // Remove trailing separator
    spans.pop();

    let paragraph = Paragraph::new(Text::from(Line::from(spans)))
        .block(Block::default())
        .left_aligned()
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Key event routing
// ---------------------------------------------------------------------------

/// Route a raw key event to an optional message. Mutates [`ViewState`]
/// directly for navigation (j/k); returns a message for actions that need
/// the async command pipeline.
pub fn handle_key_event(view_state: &mut ViewState, key: KeyCode) -> Option<Message> {
    // Global bindings
    if let KeyCode::Char('q') = key {
        return Some(Message::Quit);
    }

    // Page-level bindings
    match view_state {
        ViewState::Dashboard(view_state) => {
            dashboard::handle_key(key, view_state).map(Message::DashboardMsg)
        }
    }
}

fn global_key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding {
            key: "r".to_owned(),
            description: "refresh".to_owned(),
        },
        KeyBinding {
            key: "q".to_owned(),
            description: "quit".to_owned(),
        },
    ]
}
