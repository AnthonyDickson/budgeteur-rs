//! This file defines the `Tag` type, the types needed to create a tag and the API routes for the tag type.
//! A tag is used for categorising and grouping transactions.

use std::{
    fmt::Display,
    str::FromStr,
    sync::{Arc, Mutex},
};

use axum::{
    Form,
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use maud::{Markup, html};
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, Error,
    alert::Alert,
    endpoints,
    html::{
        BUTTON_PRIMARY_STYLE, FORM_CONTAINER_STYLE, FORM_LABEL_STYLE, FORM_TEXT_INPUT_STYLE, base,
    },
    navigation::NavBar,
};

/// The name of a tag.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TagName(String);

impl TagName {
    /// Create a tag name.
    ///
    /// # Errors
    ///
    /// This function will return an [Error::EmptyTagName] if `name` is an empty string.
    pub fn new(name: &str) -> Result<Self, Error> {
        let name = name.trim();

        if name.is_empty() {
            Err(Error::EmptyTagName)
        } else {
            Ok(Self(name.to_string()))
        }
    }

    /// Create a tag name without validation.
    ///
    /// The caller should ensure that the string is not empty.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if the non-empty invariant is violated it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(name: &str) -> Self {
        Self(name.to_string())
    }
}

impl AsRef<str> for TagName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl FromStr for TagName {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TagName::new(s)
    }
}

impl Display for TagName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type TagId = i64;

/// A tag for grouping expenses and income, e.g., 'Groceries', 'Eating Out', 'Wages'.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Tag {
    /// The ID of the tag.
    pub id: TagId,

    /// The name of the tag.
    pub name: TagName,
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

            button type="submit" class=(BUTTON_PRIMARY_STYLE) { "Update Tag" }
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

pub async fn get_new_tag_page() -> Response {
    new_tag_view().into_response()
}

/// The state needed for the tags listing page.
#[derive(Debug, Clone)]
pub struct TagsPageState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for TagsPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

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

#[derive(Debug, Serialize, Deserialize)]
pub struct TagFormData {
    pub name: String,
}

/// A route handler for creating a new tag.
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

/// Route handler for the edit tag page.
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

/// A route handler for updating a tag.
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

/// A route handler for deleting a tag.
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

/// Create a tag in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn create_tag(name: TagName, connection: &Connection) -> Result<Tag, Error> {
    connection.execute("INSERT INTO tag (name) VALUES (?1);", (name.as_ref(),))?;

    let id = connection.last_insert_rowid();

    Ok(Tag { id, name })
}

/// Retrieve tags in the database for the tag with `tag_id`.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn get_tag(tag_id: TagId, connection: &Connection) -> Result<Tag, Error> {
    connection
        .prepare("SELECT id, name FROM tag WHERE id = :id;")?
        .query_row(&[(":id", &tag_id)], map_row)
        .map_err(|error| error.into())
}

/// Update a tag's name in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error or if the tag doesn't exist.
pub fn update_tag(tag_id: TagId, new_name: TagName, connection: &Connection) -> Result<(), Error> {
    let rows_affected = connection.execute(
        "UPDATE tag SET name = ?1 WHERE id = ?2",
        (new_name.as_ref(), tag_id),
    )?;

    if rows_affected == 0 {
        return Err(Error::UpdateMissingTag);
    }

    Ok(())
}

/// Delete a tag from the database.
///
/// # Errors
/// This function will return an error if there is an SQL error or if the tag doesn't exist.
pub fn delete_tag(tag_id: TagId, connection: &Connection) -> Result<(), Error> {
    let rows_affected = connection.execute("DELETE FROM tag WHERE id = ?1", [tag_id])?;

    if rows_affected == 0 {
        return Err(Error::DeleteMissingTag);
    }

    Ok(())
}

/// Retrieve tags in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn get_all_tags(connection: &Connection) -> Result<Vec<Tag>, Error> {
    connection
        .prepare("SELECT id, name FROM tag ORDER BY name ASC;")?
        .query_map([], map_row)?
        .map(|maybe_tag| maybe_tag.map_err(|error| error.into()))
        .collect()
}

pub fn create_tag_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS tag (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );

        CREATE INDEX IF NOT EXISTS idx_tag_name ON tag(name);",
    )?;

    Ok(())
}

fn map_row(row: &Row) -> Result<Tag, rusqlite::Error> {
    let id = row.get(0)?;
    let raw_name: String = row.get(1)?;
    let name = TagName::new_unchecked(&raw_name);

    Ok(Tag { id, name })
}

#[cfg(test)]
mod tag_name_tests {
    use crate::{Error, tag::TagName};

    #[test]
    fn new_fails_on_empty_string() {
        let tag_name = TagName::new("");

        assert_eq!(tag_name, Err(Error::EmptyTagName));
    }

    #[test]
    fn new_fails_on_just_whitespace() {
        let tag_name = TagName::new("\n\t \r");

        assert_eq!(tag_name, Err(Error::EmptyTagName));
    }

    #[test]
    fn new_succeeds_on_non_empty_string() {
        let tag_name = TagName::new("🔥");

        assert!(tag_name.is_ok())
    }
}

#[cfg(test)]
mod tag_query_tests {
    use std::collections::HashSet;

    use rusqlite::Connection;

    use crate::{
        Error,
        tag::{TagName, create_tag, get_all_tags, get_tag, update_tag},
    };

    use super::{create_tag_table, delete_tag};

    fn get_test_db_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        create_tag_table(&connection).expect("Could not create tag table");
        connection
    }

    #[test]
    fn create_tag_succeeds() {
        let connection = get_test_db_connection();
        let name = TagName::new("Terrifically a tag").unwrap();

        let tag = create_tag(name.clone(), &connection);

        let got_tag = tag.expect("Could not create tag");
        assert!(got_tag.id > 0);
        assert_eq!(got_tag.name, name);
    }

    #[test]
    fn get_tag_succeeds() {
        let connection = get_test_db_connection();
        let name = TagName::new_unchecked("Foo");
        let inserted_tag = create_tag(name, &connection).expect("Could not create test tag");

        let selected_tag = get_tag(inserted_tag.id, &connection);

        assert_eq!(Ok(inserted_tag), selected_tag);
    }

    #[test]
    fn get_tag_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let inserted_tag = create_tag(TagName::new_unchecked("Foo"), &connection)
            .expect("Could not create test tag");

        let selected_tag = get_tag(inserted_tag.id + 123, &connection);

        assert_eq!(selected_tag, Err(Error::NotFound));
    }

    #[test]
    fn test_get_all_tag() {
        let store = get_test_db_connection();

        let inserted_tags = HashSet::from([
            create_tag(TagName::new_unchecked("Foo"), &store).expect("Could not create test tag"),
            create_tag(TagName::new_unchecked("Bar"), &store).expect("Could not create test tag"),
        ]);

        let selected_tags = get_all_tags(&store).expect("Could not get all tags");
        let selected_tags = HashSet::from_iter(selected_tags);

        assert_eq!(inserted_tags, selected_tags);
    }

    #[test]
    fn update_tag_succeeds() {
        let connection = get_test_db_connection();
        let original_name = TagName::new_unchecked("Original");
        let tag = create_tag(original_name, &connection).expect("Could not create test tag");

        let new_name = TagName::new_unchecked("Updated");
        let result = update_tag(tag.id, new_name.clone(), &connection);

        assert!(result.is_ok());

        let updated_tag = get_tag(tag.id, &connection).expect("Could not get updated tag");
        assert_eq!(updated_tag.name, new_name);
        assert_eq!(updated_tag.id, tag.id);
    }

    #[test]
    fn update_tag_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let invalid_id = 999999;
        let new_name = TagName::new_unchecked("Updated");

        let result = update_tag(invalid_id, new_name, &connection);

        assert_eq!(result, Err(Error::UpdateMissingTag));
    }

    #[test]
    fn delete_tag_succeeds() {
        let connection = get_test_db_connection();
        let name = TagName::new_unchecked("ToDelete");
        let tag = create_tag(name, &connection).expect("Could not create test tag");

        let result = delete_tag(tag.id, &connection);

        assert!(result.is_ok());

        let get_result = get_tag(tag.id, &connection);
        assert_eq!(get_result, Err(Error::NotFound));
    }

    #[test]
    fn delete_tag_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let invalid_id = 999999;

        let result = delete_tag(invalid_id, &connection);

        assert_eq!(result, Err(Error::DeleteMissingTag));
    }
}

#[cfg(test)]
mod new_tag_page_tests {
    use axum::{http::StatusCode, response::Response};
    use scraper::{ElementRef, Html};

    use crate::{endpoints, tag::get_new_tag_page};

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

        let html = parse_html(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_hx_endpoint(&form, endpoints::POST_TAG);
        assert_form_input(&form, "name", "text");
        assert_form_submit_button(&form);
    }

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_document(&text)
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }

    #[track_caller]
    fn must_get_form(html: &Html) -> ElementRef<'_> {
        html.select(&scraper::Selector::parse("form").unwrap())
            .next()
            .expect("No form found")
    }

    #[track_caller]
    fn assert_hx_endpoint(form: &ElementRef, endpoint: &str) {
        let hx_post = form
            .value()
            .attr("hx-post")
            .expect("hx-post attribute missing");

        assert_eq!(
            hx_post, endpoint,
            "want form with attribute hx-post=\"{endpoint}\", got {hx_post:?}"
        );
        assert_eq!(hx_post, endpoint);
    }

    #[track_caller]
    fn assert_form_input(form: &ElementRef, name: &str, type_: &str) {
        for input in form.select(&scraper::Selector::parse("input").unwrap()) {
            let input_name = input.value().attr("name").unwrap_or_default();

            if input_name == name {
                let input_type = input.value().attr("type").unwrap_or_default();
                let input_required = input.value().attr("required");

                assert_eq!(
                    input_type, type_,
                    "want input with type \"{type_}\", got {input_type:?}"
                );

                assert!(
                    input_required.is_some(),
                    "want input with name {name} to have the required attribute but got none"
                );

                return;
            }
        }

        panic!("No input found with name \"{name}\" and type \"{type_}\"");
    }

    #[track_caller]
    fn assert_form_submit_button(form: &ElementRef) {
        let submit_button = form
            .select(&scraper::Selector::parse("button").unwrap())
            .next()
            .expect("No button found");

        assert_eq!(
            submit_button.value().attr("type").unwrap_or_default(),
            "submit",
            "want submit button with type=\"submit\""
        );
    }
}

#[cfg(test)]
mod create_tag_endpoint_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        Form,
        extract::State,
        http::{StatusCode, header::CONTENT_TYPE},
        response::{IntoResponse, Response},
    };
    use rusqlite::Connection;
    use scraper::{ElementRef, Html};

    use crate::{
        endpoints,
        tag::{Tag, TagName, create_tag_endpoint, get_tag},
    };

    use super::{CreateTagEndpointState, TagFormData, create_tag_table};

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
        let html = parse_html(response).await;
        assert_valid_html(&html);
        let form = must_get_form(&html);
        assert_error_message(&form, "Error: Tag name cannot be empty");
    }

    #[track_caller]
    fn assert_hx_redirect(response: &Response, endpoint: &str) {
        assert_eq!(get_header(response, "hx-redirect"), endpoint,);
    }

    #[track_caller]
    fn get_header(response: &Response, header_name: &str) -> String {
        let header_error_message = format!("Headers missing {header_name}");

        response
            .headers()
            .get(header_name)
            .expect(&header_error_message)
            .to_str()
            .expect("Could not convert to str")
            .to_string()
    }

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_fragment(&text)
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors {:?} for HTML {}",
            html.errors,
            html.html()
        );
    }

    #[track_caller]
    fn must_get_form(html: &Html) -> ElementRef<'_> {
        html.select(&scraper::Selector::parse("form").unwrap())
            .next()
            .expect("No form found")
    }

    #[track_caller]
    fn assert_error_message(form: &ElementRef, want_error_message: &str) {
        let p = scraper::Selector::parse("p").unwrap();
        let error_message = form
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

#[cfg(test)]
mod edit_tag_endpoint_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        Form,
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Response},
    };
    use rusqlite::Connection;
    use scraper::{ElementRef, Html};

    use crate::{
        endpoints,
        tag::{TagName, create_tag, get_edit_tag_page, update_tag_endpoint},
    };

    use super::{EditTagPageState, TagFormData, UpdateTagEndpointState, create_tag_table};

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
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .expect("content-type header missing"),
            "text/html; charset=utf-8"
        );

        let html = parse_html(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_hx_endpoint(
            &form,
            &endpoints::format_endpoint(endpoints::PUT_TAG, tag.id),
            "hx-put",
        );
        assert_form_input_with_value(&form, "name", "text", tag_name.as_ref());
        assert_form_submit_button(&form, "Update Tag");
    }

    #[tokio::test]
    async fn get_edit_tag_page_with_invalid_id_shows_error() {
        let state = get_edit_tag_state();
        let invalid_id = 999999;

        let response = get_edit_tag_page(Path(invalid_id), State(state))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let html = parse_html(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_error_message(&form, "Tag not found");
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

        let html = parse_fragment_html(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_error_message(&form, "Error: Tag name cannot be empty");
    }

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_document(&text)
    }

    async fn parse_fragment_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_fragment(&text)
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors {:?} for HTML {}",
            html.errors,
            html.html()
        );
    }

    #[track_caller]
    fn must_get_form(html: &Html) -> ElementRef<'_> {
        html.select(&scraper::Selector::parse("form").unwrap())
            .next()
            .expect("No form found")
    }

    #[track_caller]
    fn assert_hx_endpoint(form: &ElementRef, endpoint: &str, attribute: &str) {
        let hx_attr = form
            .value()
            .attr(attribute)
            .unwrap_or_else(|| panic!("{attribute} attribute missing"));

        assert_eq!(
            hx_attr, endpoint,
            "want form with attribute {attribute}=\"{endpoint}\", got {hx_attr:?}"
        );
    }

    #[track_caller]
    fn assert_form_input_with_value(form: &ElementRef, name: &str, type_: &str, value: &str) {
        for input in form.select(&scraper::Selector::parse("input").unwrap()) {
            let input_name = input.value().attr("name").unwrap_or_default();

            if input_name == name {
                let input_type = input.value().attr("type").unwrap_or_default();
                let input_value = input.value().attr("value").unwrap_or_default();
                let input_required = input.value().attr("required");

                assert_eq!(
                    input_type, type_,
                    "want input with type \"{type_}\", got {input_type:?}"
                );

                assert_eq!(
                    input_value, value,
                    "want input with value \"{value}\", got {input_value:?}"
                );

                assert!(
                    input_required.is_some(),
                    "want input with name {name} to have the required attribute but got none"
                );

                return;
            }
        }

        panic!("No input found with name \"{name}\", type \"{type_}\", and value \"{value}\"");
    }

    #[track_caller]
    fn assert_form_submit_button(form: &ElementRef, text: &str) {
        let submit_button = form
            .select(&scraper::Selector::parse("button").unwrap())
            .next()
            .expect("No button found");

        assert_eq!(
            submit_button.value().attr("type").unwrap_or_default(),
            "submit",
            "want submit button with type=\"submit\""
        );

        let button_text = submit_button
            .text()
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string();
        assert_eq!(
            button_text, text,
            "want button text \"{text}\", got \"{button_text}\""
        );
    }

    #[track_caller]
    fn assert_error_message(form: &ElementRef, want_error_message: &str) {
        let p = scraper::Selector::parse("p").unwrap();
        let error_message = form
            .select(&p)
            .next()
            .expect("No error message found")
            .text()
            .collect::<Vec<_>>()
            .join("");
        let got_error_message = error_message.trim();

        assert_eq!(want_error_message, got_error_message);
    }

    #[track_caller]
    fn assert_hx_redirect(response: &Response, endpoint: &str) {
        assert_eq!(get_header(response, "hx-redirect"), endpoint);
    }

    #[track_caller]
    fn get_header(response: &Response, header_name: &str) -> String {
        let header_error_message = format!("Headers missing {header_name}");

        response
            .headers()
            .get(header_name)
            .expect(&header_error_message)
            .to_str()
            .expect("Could not convert to str")
            .to_string()
    }
}

#[cfg(test)]
mod delete_tag_endpoint_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Response},
    };
    use rusqlite::Connection;
    use scraper::Html;

    use crate::tag::{TagName, create_tag, delete_tag_endpoint};

    use super::{DeleteTagEndpointState, create_tag_table};

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

        let html = parse_fragment_html(response).await;
        assert_valid_html(&html);
        assert_error_content(&html, "Could not delete tag");
    }

    #[track_caller]
    fn get_header(response: &Response, header_name: &str) -> String {
        let header_error_message = format!("Headers missing {header_name}");

        response
            .headers()
            .get(header_name)
            .expect(&header_error_message)
            .to_str()
            .expect("Could not convert to str")
            .to_string()
    }

    async fn parse_fragment_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_fragment(&text)
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors {:?} for HTML {}",
            html.errors,
            html.html()
        );
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
