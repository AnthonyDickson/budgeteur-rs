//! Tag editing page and endpoint.

use std::sync::{Arc, Mutex};

use axum::{
    Form,
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use maud::{Markup, html};
use rusqlite::Connection;

use crate::{
    AppState, Error, endpoints,
    html::{
        BUTTON_PRIMARY_STYLE, FORM_CONTAINER_STYLE, FORM_LABEL_STYLE, FORM_TEXT_INPUT_STYLE, base,
    },
    navigation::NavBar,
    tag::{TagId, TagName, domain::TagFormData, get_tag, update_tag},
};

/// The state needed for the edit tag page.
#[derive(Debug, Clone)]
pub struct EditTagPageState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for EditTagPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// The state needed for updating a tag.
#[derive(Debug, Clone)]
pub struct UpdateTagEndpointState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for UpdateTagEndpointState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Render the tag editing page.
pub async fn get_edit_tag_page(
    Path(tag_id): Path<TagId>,
    State(state): State<EditTagPageState>,
) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let edit_endpoint = endpoints::format_endpoint(endpoints::EDIT_TAG_VIEW, tag_id);
    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_TAG, tag_id);

    match get_tag(tag_id, &connection) {
        Ok(tag) => Ok(
            edit_tag_view(&edit_endpoint, &update_endpoint, tag.name.as_ref(), "").into_response(),
        ),
        Err(error) => {
            let error_message = match error {
                Error::NotFound => "Tag not found",
                _ => {
                    tracing::error!("Failed to retrieve tag {tag_id}: {error}");
                    "Failed to load tag"
                }
            };

            Ok(edit_tag_view(&edit_endpoint, &update_endpoint, "", error_message).into_response())
        }
    }
}

/// Handle tag update form submission.
pub async fn update_tag_endpoint(
    Path(tag_id): Path<TagId>,
    State(state): State<UpdateTagEndpointState>,
    Form(form_data): Form<TagFormData>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_TAG, tag_id);

    let name = match TagName::new(&form_data.name) {
        Ok(name) => name,
        Err(error) => {
            return edit_tag_form_view(
                &update_endpoint,
                &form_data.name,
                &format!("Error: {error}"),
            )
            .into_response();
        }
    };

    match update_tag(tag_id, name, &connection) {
        Ok(_) => (
            HxRedirect(endpoints::TAGS_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response(),
        Err(Error::UpdateMissingTag) => Error::UpdateMissingTag.into_alert_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while updating tag {tag_id}: {error}");
            error.into_alert_response()
        }
    }
}

fn edit_tag_view(
    edit_endpoint: &str,
    update_endpoint: &str,
    tag_name: &str,
    error_message: &str,
) -> Markup {
    let nav_bar = NavBar::new(edit_endpoint).into_html();
    let form = edit_tag_form_view(update_endpoint, tag_name, error_message);

    let content = html! {
        (nav_bar)
        div class=(FORM_CONTAINER_STYLE) { (form) }
    };

    base("Edit Tag", &[], &content)
}

fn edit_tag_form_view(update_tag_endpoint: &str, tag_name: &str, error_message: &str) -> Markup {
    html! {
        form
            hx-put=(update_tag_endpoint)
            class="w-full space-y-4 md:space-y-6"
        {
            div
            {
                label
                    for="name"
                    class=(FORM_LABEL_STYLE)
                {
                    "Tag Name"
                }

                input
                    id="name"
                    type="text"
                    name="name"
                    placeholder="Tag Name"
                    value=(tag_name)
                    required
                    autofocus
                    class=(FORM_TEXT_INPUT_STYLE);
            }

            @if !error_message.is_empty() {
                p
                {
                    (error_message)
                }
            }

            button type="submit" class=(BUTTON_PRIMARY_STYLE) { "Update Tag" }
        }
    }
}

#[cfg(test)]
mod edit_tag_endpoint_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        Form,
        extract::{Path, State},
        http::StatusCode,
        response::IntoResponse,
    };
    use rusqlite::Connection;

    use crate::{
        endpoints,
        tag::{
            TagName, create_tag, create_tag_table,
            domain::TagFormData,
            edit::{EditTagPageState, UpdateTagEndpointState},
            get_edit_tag_page, update_tag_endpoint,
        },
        test_utils::{
            assert_content_type, assert_form_error_message, assert_form_input_with_value,
            assert_form_submit_button_with_text, assert_hx_endpoint, assert_hx_redirect,
            assert_valid_html, must_get_form, parse_html_document, parse_html_fragment,
        },
    };

    fn get_edit_tag_state() -> EditTagPageState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_tag_table(&connection).expect("Could not create tag table");

        EditTagPageState {
            db_connection: Arc::new(Mutex::new(connection)),
        }
    }

    fn get_update_tag_state() -> UpdateTagEndpointState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_tag_table(&connection).expect("Could not create tag table");

        UpdateTagEndpointState {
            db_connection: Arc::new(Mutex::new(connection)),
        }
    }

    #[tokio::test]
    async fn get_edit_tag_page_succeeds() {
        let state = get_edit_tag_state();
        let tag_name = TagName::new_unchecked("Test Tag");
        let tag = create_tag(tag_name.clone(), &state.db_connection.lock().unwrap())
            .expect("Could not create test tag");

        let response = get_edit_tag_page(Path(tag.id), State(state)).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");

        let html = parse_html_document(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_hx_endpoint(
            &form,
            &endpoints::format_endpoint(endpoints::PUT_TAG, tag.id),
            "hx-put",
        );
        assert_form_input_with_value(&form, "name", "text", tag_name.as_ref());
        assert_form_submit_button_with_text(&form, "Update Tag");
    }

    #[tokio::test]
    async fn get_edit_tag_page_with_invalid_id_shows_error() {
        let state = get_edit_tag_state();
        let invalid_id = 999999;

        let response = get_edit_tag_page(Path(invalid_id), State(state))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html_document(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_form_error_message(&form, "Tag not found");
    }

    #[tokio::test]
    async fn update_tag_endpoint_succeeds() {
        let state = get_update_tag_state();
        let original_name = TagName::new_unchecked("Original");
        let tag = create_tag(original_name, &state.db_connection.lock().unwrap())
            .expect("Could not create test tag");

        let form = TagFormData {
            name: "Updated".to_string(),
        };

        let response = update_tag_endpoint(Path(tag.id), State(state), Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_hx_redirect(&response, endpoints::TAGS_VIEW);
    }

    #[tokio::test]
    async fn update_tag_endpoint_with_invalid_id_returns_not_found() {
        let state = get_update_tag_state();
        let invalid_id = 999999;
        let form = TagFormData {
            name: "Updated".to_string(),
        };

        let response = update_tag_endpoint(Path(invalid_id), State(state), Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn update_tag_endpoint_with_empty_name_returns_error() {
        let state = get_update_tag_state();
        let tag_name = TagName::new_unchecked("Test Tag");
        let tag = create_tag(tag_name, &state.db_connection.lock().unwrap())
            .expect("Could not create test tag");

        let form = TagFormData {
            name: "".to_string(),
        };

        let response = update_tag_endpoint(Path(tag.id), State(state), Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html_fragment(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_form_error_message(&form, "Error: Tag name cannot be empty");
    }
}
