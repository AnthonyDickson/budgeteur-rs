//! Defines the route handler for the page for creating an account balance.

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

/// Renders the create balance page.
#[derive(Template)]
#[template(path = "views/balance/create.html")]
struct CreateBalanceTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    create_balance_route: &'a str,
    max_date: Date,
}

/// The state needed for create balance page.
#[derive(Debug, Clone)]
pub struct CreateBalancePageState {
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
}

impl FromRef<AppState> for CreateBalancePageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone.clone(),
        }
    }
}

/// Renders the page for creating an account balance.
pub async fn get_create_balance_page(State(state): State<CreateBalancePageState>) -> Response {
    let nav_bar = get_nav_bar(endpoints::NEW_BALANCE_VIEW);

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
        CreateBalanceTemplate {
            nav_bar,
            create_balance_route: endpoints::BALANCES,
            max_date: OffsetDateTime::now_utc().to_offset(local_timezone).date(),
        },
    )
}
