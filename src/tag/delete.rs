//! Tag deletion endpoint.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Path, State},
    response::{IntoResponse, Response},
};
use rusqlite::Connection;

use crate::{
    AppState, Error,
    alert::Alert,
    tag::{TagId, db::delete_tag},
};

/// The state needed for deleting a tag.
#[derive(Debug, Clone)]
pub struct DeleteTagEndpointState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for DeleteTagEndpointState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Handle tag deletion. Returns success alert or error.
pub async fn delete_tag_endpoint(
    Path(tag_id): Path<TagId>,
    State(state): State<DeleteTagEndpointState>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match delete_tag(tag_id, &connection) {
        Ok(_) => Alert::SuccessSimple {
            message: "Tag deleted successfully".to_owned(),
        }
        .into_response(),
        Err(Error::DeleteMissingTag) => Error::DeleteMissingTag.into_alert_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while deleting tag {tag_id}: {error}");
            error.into_alert_response()
        }
    }
}

#[cfg(test)]
mod delete_tag_endpoint_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::{Path, State},
        http::StatusCode,
        response::IntoResponse,
    };
    use rusqlite::Connection;
    use scraper::Html;

    use crate::{
        tag::{TagName, create_tag, create_tag_table, delete_tag_endpoint},
        test_utils::{assert_valid_html, get_header, parse_html_fragment},
    };

    use super::DeleteTagEndpointState;

    fn get_delete_tag_state() -> DeleteTagEndpointState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_tag_table(&connection).expect("Could not create tag table");

        DeleteTagEndpointState {
            db_connection: Arc::new(Mutex::new(connection)),
        }
    }

    #[tokio::test]
    async fn delete_tag_endpoint_succeeds() {
        let state = get_delete_tag_state();
        let tag_name = TagName::new_unchecked("Test Tag");
        let tag = create_tag(tag_name, &state.db_connection.lock().unwrap())
            .expect("Could not create test tag");

        let response = delete_tag_endpoint(Path(tag.id), State(state))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn delete_tag_endpoint_with_invalid_id_returns_error_html() {
        let state = get_delete_tag_state();
        let invalid_id = 999999;

        let response = delete_tag_endpoint(Path(invalid_id), State(state))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            get_header(&response, "content-type"),
            "text/html; charset=utf-8"
        );

        let html = parse_html_fragment(response).await;
        assert_valid_html(&html);
        assert_error_content(&html, "Could not delete tag");
    }

    #[track_caller]
    fn assert_error_content(html: &Html, want_error_message: &str) {
        let p = scraper::Selector::parse("p").unwrap();
        let error_message = html
            .select(&p)
            .next()
            .expect("No error message found")
            .text()
            .collect::<Vec<_>>()
            .join("");
        let got_error_message = error_message.trim();

        assert_eq!(want_error_message, got_error_message);
    }
}
