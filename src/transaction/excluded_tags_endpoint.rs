use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::Form;
use axum_htmx::HxRedirect;
use rusqlite::Connection;

use crate::{
    AppState, Error, endpoints,
    tag::{ExcludedTagsForm, save_excluded_tags},
};

/// State needed for updating excluded tags on the transactions page.
#[derive(Debug, Clone)]
pub struct TransactionsExcludedTagsState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for TransactionsExcludedTagsState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// API endpoint to update excluded tags and return to the transactions page.
pub async fn update_transactions_excluded_tags(
    State(state): State<TransactionsExcludedTagsState>,
    Form(form): Form<ExcludedTagsForm>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    if let Err(error) = save_excluded_tags(&form.excluded_tags, &connection) {
        tracing::error!("Failed to save transaction preferences: {error}");
        return Error::DashboardPreferencesSaveError.into_alert_response();
    }

    let redirect_url = form
        .redirect_url
        .unwrap_or_else(|| endpoints::TRANSACTIONS_VIEW.to_owned());

    (HxRedirect(redirect_url), StatusCode::SEE_OTHER).into_response()
}
