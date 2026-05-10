//! Tag creation page and endpoint.

use std::sync::{Arc, Mutex};

use axum::{
    Form,
    extract::{FromRef, State},
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
    tag::{TagName, create_tag, domain::TagFormData},
};

/// The state needed for creating a tag.
#[derive(Debug, Clone)]
pub struct CreateTagEndpointState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for CreateTagEndpointState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Render the tag creation page.
pub async fn get_new_tag_page() -> Response {
    new_tag_view().into_response()
}

/// Handle tag creation form submission.
pub async fn create_tag_endpoint(
    State(state): State<CreateTagEndpointState>,
    Form(new_tag): Form<TagFormData>,
) -> Response {
    let name = match TagName::new(&new_tag.name) {
        Ok(name) => name,
        Err(error) => {
            return new_tag_form_view(&format!("Error: {error}")).into_response();
        }
    };

    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match create_tag(name, &connection) {
        Ok(_) => (
            HxRedirect(endpoints::TAGS_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while creating a tag: {error}");

            error.into_alert_response()
        }
    }
}
fn new_tag_view() -> Markup {
    let nav_bar = NavBar::new(endpoints::NEW_TAG_VIEW).into_html();
    let form = new_tag_form_view("");

    let content = html! {
        (nav_bar)
        div class=(FORM_CONTAINER_STYLE) { (form) }
    };

    base("Create Tag", &[], &content)
}
fn new_tag_form_view(error_message: &str) -> Markup {
    let create_tag_endpoint = endpoints::POST_TAG;

    html! {
        form
            hx-post=(create_tag_endpoint)
            hx-target-error="#alert-container"
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
                    required
                    autofocus
                    class=(FORM_TEXT_INPUT_STYLE);
            }

            @if !error_message.is_empty() {
                p class="text-red-600 dark:text-red-400"
                {
                    (error_message)
                }
            }

            button type="submit" class=(BUTTON_PRIMARY_STYLE) { "Create Tag" }
        }
    }
}

#[cfg(test)]
mod new_tag_page_tests {
    use axum::http::StatusCode;

    use crate::{
        endpoints,
        tag::get_new_tag_page,
        test_utils::{
            assert_form_input, assert_form_submit_button, assert_hx_endpoint, assert_valid_html,
            must_get_form, parse_html_document,
        },
    };

    #[tokio::test]
    async fn render_page() {
        let response = get_new_tag_page().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .expect("content-type header missing"),
            "text/html; charset=utf-8"
        );

        let html = parse_html_document(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_hx_endpoint(&form, endpoints::POST_TAG, "hx-post");
        assert_form_input(&form, "name", "text");
        assert_form_submit_button(&form);
    }
}

#[cfg(test)]
mod create_tag_endpoint_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        Form,
        extract::State,
        http::{StatusCode, header::CONTENT_TYPE},
        response::IntoResponse,
    };
    use rusqlite::Connection;

    use crate::{
        endpoints,
        tag::{
            Tag, TagName, create::CreateTagEndpointState, create_tag_endpoint, create_tag_table,
            domain::TagFormData, get_tag,
        },
        test_utils::{
            assert_form_error_message, assert_hx_redirect, assert_valid_html, get_header,
            must_get_form, parse_html_fragment,
        },
    };

    fn get_tag_state() -> CreateTagEndpointState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_tag_table(&connection).expect("Could not create tag table");

        CreateTagEndpointState {
            db_connection: Arc::new(Mutex::new(connection)),
        }
    }

    #[tokio::test]
    async fn can_create_tag() {
        let state = get_tag_state();
        let name = TagName::new_unchecked("Foo");
        let want = Tag {
            id: 1,
            name: name.clone(),
        };
        let form = TagFormData {
            name: name.to_string(),
        };

        let response = create_tag_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_hx_redirect(&response, endpoints::TAGS_VIEW);
        assert_eq!(Ok(want), get_tag(1, &state.db_connection.lock().unwrap()));
    }

    #[tokio::test]
    async fn create_tag_fails_on_empty_name() {
        let state = get_tag_state();
        let form = TagFormData {
            name: "".to_string(),
        };

        let response = create_tag_endpoint(State(state), Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            get_header(&response, CONTENT_TYPE.as_str()),
            "text/html; charset=utf-8"
        );
        let html = parse_html_fragment(response).await;
        assert_valid_html(&html);
        let form = must_get_form(&html);
        assert_form_error_message(&form, "Error: Tag name cannot be empty");
    }
}
