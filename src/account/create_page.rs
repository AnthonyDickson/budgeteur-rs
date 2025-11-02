//! Defines the route handler for the page for creating an account.

use askama::Template;
use axum::{
    extract::{FromRef, State},
    http::StatusCode,
    response::Response,
};
use time::{Date, OffsetDateTime};

use crate::{
    AppState, endpoints,
    internal_server_error::{InternalServerErrorPageTemplate, render_internal_server_error},
    navigation::{NavbarTemplate, get_nav_bar},
    shared_templates::render,
    timezone::get_local_offset,
};

/// Renders the create account page.
#[derive(Template)]
#[template(path = "views/account/create.html")]
struct CreateAccountTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    create_account_route: &'a str,
    max_date: Date,
}

/// The state needed for create page.
#[derive(Debug, Clone)]
pub struct CreateAccountPageState {
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
}

impl FromRef<AppState> for CreateAccountPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone.clone(),
        }
    }
}

/// Renders the page for creating an account.
pub async fn get_create_account_page(State(state): State<CreateAccountPageState>) -> Response {
    let nav_bar = get_nav_bar(endpoints::NEW_ACCOUNT_VIEW);

    let local_timezone = match get_local_offset(&state.local_timezone) {
        Some(offset) => offset,
        None => {
            tracing::error!(
                "Could not get local time offset from timezone {}",
                &state.local_timezone
            );
            return render_internal_server_error(InternalServerErrorPageTemplate {
                description: "Could not get local timezone",
                fix: "Check your server settings and ensure the timezone has \
                been set to valid, canonical timezone string",
            });
        }
    };

    render(
        StatusCode::OK,
        CreateAccountTemplate {
            nav_bar,
            create_account_route: endpoints::ACCOUNTS,
            max_date: OffsetDateTime::now_utc().to_offset(local_timezone).date(),
        },
    )
}
