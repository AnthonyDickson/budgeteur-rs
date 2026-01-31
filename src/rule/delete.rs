use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    Error,
    alert::Alert,
    rule::{
        db::delete_rule,
        models::{RuleId, RuleState},
    },
};

/// A route handler for deleting a rule.
pub async fn delete_rule_endpoint(
    Path(rule_id): Path<RuleId>,
    State(state): State<RuleState>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match delete_rule(rule_id, &connection) {
        Ok(_) => (
            StatusCode::OK,
            Alert::SuccessSimple {
                message: "Rule deleted successfully".to_owned(),
            }
            .into_html(),
        )
            .into_response(),
        Err(Error::DeleteMissingRule) => Error::DeleteMissingRule.into_alert_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while deleting rule {rule_id}: {error}");
            error.into_alert_response()
        }
    }
}
