//! This file defines the `Tag` type, the types needed to create a tag and the API routes for the tag type.
//! A tag is used for categorising and grouping transactions.

use std::fmt::Display;
use std::sync::{Arc, Mutex};

use askama_axum::Template;
use axum::{
    Form,
    extract::{FromRef, Path, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, Error,
    database_id::DatabaseID,
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    shared_templates::NewTagFormTemplate,
};

/// The name of a tag.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TagName(String);

impl TagName {
    /// Create a tag name.
    ///
    /// # Errors
    ///
    /// This function will return an error if `name` is an empty string.
    pub fn new(name: &str) -> Result<Self, Error> {
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

impl Display for TagName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A tag for grouping expenses and income, e.g., 'Groceries', 'Eating Out', 'Wages'.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Tag {
    /// The ID of the tag.
    pub id: DatabaseID,

    /// The name of the tag.
    pub name: TagName,
}

/// Renders the new tag page.
#[derive(Template)]
#[template(path = "views/new_tag.html")]
struct NewTagTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    form: NewTagFormTemplate<'a>,
}

/// Renders the form for editing a tag.
#[derive(Template)]
#[template(path = "partials/edit_tag_form.html")]
struct EditTagFormTemplate<'a> {
    update_tag_endpoint: &'a str,
    tag_name: &'a str,
    error_message: &'a str,
}

/// Renders the edit tag page.
#[derive(Template)]
#[template(path = "views/edit_tag.html")]
struct EditTagTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    form: EditTagFormTemplate<'a>,
}

/// Renders an error message for tag operations.
#[derive(Template)]
#[template(path = "partials/tag_error.html")]
struct TagErrorTemplate<'a> {
    error_message: &'a str,
}

pub async fn get_new_tag_page() -> Response {
    NewTagTemplate {
        nav_bar: get_nav_bar(endpoints::NEW_TAG_VIEW),
        form: NewTagFormTemplate {
            create_tag_endpoint: endpoints::POST_TAG,
            error_message: "",
        },
    }
    .into_response()
}

/// A tag with its formatted edit URL for template rendering.
#[derive(Debug, Clone)]
struct TagWithEditUrl {
    pub tag: Tag,
    pub edit_url: String,
    pub transaction_count: i64,
}

/// Renders the tags listing page.
#[derive(Template)]
#[template(path = "views/tags.html")]
struct TagsTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    tags: Vec<TagWithEditUrl>,
    new_tag_route: &'a str,
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

/// Route handler for the tags listing page.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_tags_page(State(state): State<TagsPageState>) -> Response {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    match get_all_tags(&connection) {
        Ok(tags) => {
            let tags_with_edit_urls = tags
                .into_iter()
                .map(|tag| {
                    let transaction_count =
                        get_tag_transaction_count(tag.id, &connection).unwrap_or(0); // Default to 0 if count query fails
                    TagWithEditUrl {
                        edit_url: endpoints::format_endpoint(endpoints::EDIT_TAG_VIEW, tag.id),
                        tag,
                        transaction_count,
                    }
                })
                .collect();

            TagsTemplate {
                nav_bar: get_nav_bar(endpoints::TAGS_VIEW),
                tags: tags_with_edit_urls,
                new_tag_route: endpoints::NEW_TAG_VIEW,
            }
            .into_response()
        }
        Err(error) => {
            tracing::error!("Failed to retrieve tags: {error}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load tags").into_response()
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
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_tag_endpoint(
    State(state): State<CreateTagEndpointState>,
    Form(new_tag): Form<TagFormData>,
) -> impl IntoResponse {
    let name = match TagName::new(&new_tag.name) {
        Ok(name) => name,
        Err(error) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                NewTagFormTemplate {
                    create_tag_endpoint: endpoints::POST_TAG,
                    error_message: &format!("Error: {error}"),
                },
            )
                .into_response();
        }
    };

    create_tag(
        name,
        &state
            .db_connection
            .lock()
            .expect("Could not acquire database lock"),
    )
    .map(|_tag| {
        (
            HxRedirect(Uri::from_static(endpoints::TAGS_VIEW)),
            StatusCode::SEE_OTHER,
        )
    })
    .map_err(|error| {
        tracing::error!("An unexpected error occurred while creating a tag: {error}");

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            NewTagFormTemplate {
                create_tag_endpoint: endpoints::POST_TAG,
                error_message: "An unexpected error occurred. Please try again.",
            },
        )
    })
    .into_response()
}

/// Route handler for the edit tag page.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_edit_tag_page(
    Path(tag_id): Path<DatabaseID>,
    State(state): State<EditTagPageState>,
) -> Response {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let edit_endpoint = endpoints::format_endpoint(endpoints::EDIT_TAG_VIEW, tag_id);
    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_TAG, tag_id);

    match get_tag(tag_id, &connection) {
        Ok(tag) => EditTagTemplate {
            nav_bar: get_nav_bar(&edit_endpoint),
            form: EditTagFormTemplate {
                update_tag_endpoint: &update_endpoint,
                tag_name: tag.name.as_ref(),
                error_message: "",
            },
        }
        .into_response(),
        Err(error) => {
            let error_message = match error {
                Error::NotFound => "Tag not found",
                _ => {
                    tracing::error!("Failed to retrieve tag {tag_id}: {error}");
                    "Failed to load tag"
                }
            };

            EditTagTemplate {
                nav_bar: get_nav_bar(&edit_endpoint),
                form: EditTagFormTemplate {
                    update_tag_endpoint: &update_endpoint,
                    tag_name: "",
                    error_message,
                },
            }
            .into_response()
        }
    }
}

/// A route handler for updating a tag.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn update_tag_endpoint(
    Path(tag_id): Path<DatabaseID>,
    State(state): State<UpdateTagEndpointState>,
    Form(form_data): Form<TagFormData>,
) -> impl IntoResponse {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_TAG, tag_id);

    let name = match TagName::new(&form_data.name) {
        Ok(name) => name,
        Err(error) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                EditTagFormTemplate {
                    update_tag_endpoint: &update_endpoint,
                    tag_name: &form_data.name,
                    error_message: &format!("Error: {error}"),
                },
            )
                .into_response();
        }
    };

    update_tag(tag_id, name, &connection)
        .map(|_| {
            (
                HxRedirect(Uri::from_static(endpoints::TAGS_VIEW)),
                StatusCode::SEE_OTHER,
            )
        })
        .map_err(|error| {
            let (status, error_message) = match error {
                Error::NotFound => (StatusCode::NOT_FOUND, "Tag not found"),
                _ => {
                    tracing::error!(
                        "An unexpected error occurred while updating tag {tag_id}: {error}"
                    );
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "An unexpected error occurred. Please try again.",
                    )
                }
            };

            (
                status,
                EditTagFormTemplate {
                    update_tag_endpoint: &update_endpoint,
                    tag_name: &form_data.name,
                    error_message,
                },
            )
        })
        .into_response()
}

/// A route handler for deleting a tag.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn delete_tag_endpoint(
    Path(tag_id): Path<DatabaseID>,
    State(state): State<DeleteTagEndpointState>,
) -> impl IntoResponse {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    delete_tag(tag_id, &connection)
        .map(|_| {
            (
                HxRedirect(Uri::from_static(endpoints::TAGS_VIEW)),
                StatusCode::SEE_OTHER,
            )
        })
        .map_err(|error| {
            let error_message = match error {
                Error::NotFound => "Tag not found",
                _ => {
                    tracing::error!(
                        "An unexpected error occurred while deleting tag {tag_id}: {error}"
                    );
                    "An unexpected error occurred. Please try again."
                }
            };

            TagErrorTemplate { error_message }
        })
        .into_response()
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
pub fn get_tag(tag_id: DatabaseID, connection: &Connection) -> Result<Tag, Error> {
    connection
        .prepare("SELECT id, name FROM tag WHERE id = :id;")?
        .query_row(&[(":id", &tag_id)], map_row)
        .map_err(|error| error.into())
}

/// Update a tag's name in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error or if the tag doesn't exist.
pub fn update_tag(
    tag_id: DatabaseID,
    new_name: TagName,
    connection: &Connection,
) -> Result<(), Error> {
    let rows_affected = connection.execute(
        "UPDATE tag SET name = ?1 WHERE id = ?2",
        (new_name.as_ref(), tag_id),
    )?;

    if rows_affected == 0 {
        return Err(Error::NotFound);
    }

    Ok(())
}

/// Delete a tag from the database.
///
/// # Errors
/// This function will return an error if there is an SQL error or if the tag doesn't exist.
pub fn delete_tag(tag_id: DatabaseID, connection: &Connection) -> Result<(), Error> {
    let rows_affected = connection.execute("DELETE FROM tag WHERE id = ?1", [tag_id])?;

    if rows_affected == 0 {
        return Err(Error::NotFound);
    }

    Ok(())
}

/// Retrieve tags in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn get_all_tags(connection: &Connection) -> Result<Vec<Tag>, Error> {
    connection
        .prepare("SELECT id, name FROM tag;")?
        .query_map([], map_row)?
        .map(|maybe_tag| maybe_tag.map_err(|error| error.into()))
        .collect()
}

/// Get the number of transactions associated with a tag.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn get_tag_transaction_count(
    tag_id: DatabaseID,
    connection: &Connection,
) -> Result<i64, Error> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM transaction_tag WHERE tag_id = ?1",
        [tag_id],
        |row| row.get(0),
    )?;

    Ok(count)
}

pub fn create_tag_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS tag (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );",
        (),
    )?;

    Ok(())
}

fn map_row(row: &Row) -> Result<Tag, rusqlite::Error> {
    let id = row.get(0)?;
    let raw_name: String = row.get(1)?;
    let name = TagName::new_unchecked(&raw_name);

    Ok(Tag { id, name })
}

/// Create the transaction_tag junction table in the database.
///
/// # Errors
/// Returns an error if the table cannot be created or if there is an SQL error.
pub fn create_transaction_tag_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS transaction_tag (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE,
            FOREIGN KEY(tag_id) REFERENCES tag(id) ON UPDATE CASCADE ON DELETE CASCADE,
            UNIQUE(transaction_id, tag_id)
        )",
        (),
    )?;

    // Ensure the sequence starts at 1
    connection.execute(
        "INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('transaction_tag', 0)",
        (),
    )?;

    Ok(())
}

/// Add a tag to a transaction.
///
/// # Errors
/// This function will return a:
/// - [Error::InvalidTag] if `tag_id` does not refer to a valid tag,
/// - [Error::SqlError] if there is some other SQL error.
// TODO: Remove build config attribute once add_tag_to_transaction function is used elsewhere.
#[cfg(test)]
pub fn add_tag_to_transaction(
    transaction_id: DatabaseID,
    tag_id: DatabaseID,
    connection: &Connection,
) -> Result<(), Error> {
    connection
        .execute(
            "INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1, ?2)",
            (transaction_id, tag_id),
        )
        .map_err(|error| match error {
            // Code 787 occurs when a FOREIGN KEY constraint failed.
            rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                Error::InvalidTag
            }
            error => error.into(),
        })?;
    Ok(())
}

/// Remove a tag from a transaction.
///
/// # Errors
/// This function will return a [Error::SqlError] if there is a SQL error.
// TODO: Remove build config attribute once remove_tag_from_transaction function is used elsewhere.
#[cfg(test)]
pub fn remove_tag_from_transaction(
    transaction_id: DatabaseID,
    tag_id: DatabaseID,
    connection: &Connection,
) -> Result<(), Error> {
    connection.execute(
        "DELETE FROM transaction_tag WHERE transaction_id = ?1 AND tag_id = ?2",
        (transaction_id, tag_id),
    )?;
    Ok(())
}

/// Get all tags for a transaction.
///
/// # Errors
/// This function will return a [Error::SqlError] if there is a SQL error.
// TODO: Remove build config attribute once get_transaction_tags function is used elsewhere.
#[cfg(test)]
pub fn get_transaction_tags(
    transaction_id: DatabaseID,
    connection: &Connection,
) -> Result<Vec<Tag>, Error> {
    connection
        .prepare(
            "SELECT t.id, t.name 
             FROM tag t
             INNER JOIN transaction_tag tt ON t.id = tt.tag_id 
             WHERE tt.transaction_id = ?1
             ORDER BY t.name",
        )?
        .query_map([transaction_id], |row| {
            let id = row.get(0)?;
            let raw_name: String = row.get(1)?;
            let name = TagName::new_unchecked(&raw_name);
            Ok(Tag { id, name })
        })?
        .map(|maybe_tag| maybe_tag.map_err(Error::SqlError))
        .collect()
}

/// Set tags for a transaction, replacing any existing tags.
///
/// # Errors
/// This function will return a:
/// - [Error::InvalidTag] if any `tag_id` does not refer to a valid tag,
/// - [Error::SqlError] if there is some other SQL error.
// TODO: Remove build config attribute once set_transaction_tag function is used elsewhere.
#[cfg(test)]
pub fn set_transaction_tags(
    transaction_id: DatabaseID,
    tag_ids: &[DatabaseID],
    connection: &Connection,
) -> Result<(), Error> {
    let tx = connection.unchecked_transaction()?;

    // Remove existing tags
    tx.execute(
        "DELETE FROM transaction_tag WHERE transaction_id = ?1",
        [transaction_id],
    )?;

    // Add new tags
    let mut stmt =
        tx.prepare("INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1, ?2)")?;

    for &tag_id in tag_ids {
        stmt.execute((transaction_id, tag_id))
            .map_err(|error| match error {
                // Code 787 occurs when a FOREIGN KEY constraint failed.
                rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                    Error::InvalidTag
                }
                error => error.into(),
            })?;
    }

    drop(stmt);
    tx.commit()?;
    Ok(())
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
    fn new_succeeds_on_non_empty_string() {
        let tag_name = TagName::new("🔥");

        assert!(tag_name.is_ok())
    }
}

#[cfg(test)]
mod tag_query_tests {
    use std::collections::HashSet;

    use rusqlite::Connection;

    use crate::tag::{create_tag, get_all_tags, get_tag, update_tag};
    use crate::{Error, tag::TagName};

    use super::{
        add_tag_to_transaction, create_tag_table, create_transaction_tag_table, delete_tag,
        get_tag_transaction_count,
    };

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

        assert_eq!(result, Err(Error::NotFound));
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

        assert_eq!(result, Err(Error::NotFound));
    }

    #[test]
    fn delete_tag_with_transactions_succeeds_and_removes_relationships() {
        let connection = get_test_db_connection();

        // Create required tables
        crate::transaction::create_transaction_table(&connection)
            .expect("Could not create transaction table");
        create_transaction_tag_table(&connection).expect("Could not create junction table");

        // Create test data
        let tag_name = TagName::new_unchecked("TestTag");
        let tag = create_tag(tag_name, &connection).expect("Could not create test tag");

        let transaction = crate::transaction::create_transaction(
            crate::transaction::Transaction::build(100.0).description("Test transaction"),
            &connection,
        )
        .expect("Could not create test transaction");

        // Add tag to transaction
        add_tag_to_transaction(transaction.id(), tag.id, &connection)
            .expect("Could not add tag to transaction");

        // Verify relationship exists
        let count_before = get_tag_transaction_count(tag.id, &connection)
            .expect("Could not get transaction count");
        assert_eq!(count_before, 1);

        // Delete the tag
        let result = delete_tag(tag.id, &connection);
        assert!(result.is_ok());

        // Verify tag is deleted
        let get_result = get_tag(tag.id, &connection);
        assert_eq!(get_result, Err(Error::NotFound));

        // Verify relationship is also deleted (CASCADE DELETE)
        let count_after = get_tag_transaction_count(tag.id, &connection)
            .expect("Could not get transaction count");
        assert_eq!(count_after, 0);
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
    fn must_get_form(html: &Html) -> ElementRef {
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

    use askama_axum::IntoResponse;
    use axum::{
        Form,
        extract::State,
        http::{StatusCode, header::CONTENT_TYPE},
        response::Response,
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

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
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
    fn must_get_form(html: &Html) -> ElementRef {
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
mod transaction_tag_junction_tests {
    use rusqlite::Connection;
    use std::collections::HashSet;

    use crate::{
        Error,
        tag::{Tag, TagName, create_tag, create_tag_table},
        transaction::{Transaction, create_transaction, create_transaction_table},
    };

    use super::{
        add_tag_to_transaction, create_transaction_tag_table, get_transaction_tags,
        remove_tag_from_transaction, set_transaction_tags,
    };

    fn get_test_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();

        // Create all necessary tables
        create_tag_table(&connection).expect("Could not create tag table");
        create_transaction_table(&connection).expect("Could not create transaction table");
        create_transaction_tag_table(&connection).expect("Could not create junction table");

        connection
    }

    fn create_test_tag(name: &str, connection: &Connection) -> Tag {
        create_tag(TagName::new_unchecked(name), connection).expect("Could not create test tag")
    }

    fn create_test_transaction(
        amount: f64,
        description: &str,
        connection: &Connection,
    ) -> Transaction {
        create_transaction(
            Transaction::build(amount).description(description),
            connection,
        )
        .expect("Could not create test transaction")
    }

    // ============================================================================
    // BASIC CRUD TESTS
    // ============================================================================

    #[test]
    fn add_tag_to_transaction_succeeds() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        let result = add_tag_to_transaction(transaction.id(), tag.id, &connection);

        assert!(result.is_ok());

        // Verify the relationship was created
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], tag);
    }

    #[test]
    fn remove_tag_from_transaction_succeeds() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // First add the tag
        add_tag_to_transaction(transaction.id(), tag.id, &connection)
            .expect("Could not add tag to transaction");

        // Then remove it
        let result = remove_tag_from_transaction(transaction.id(), tag.id, &connection);

        assert!(result.is_ok());

        // Verify the relationship was removed
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn get_transaction_tags_returns_correct_tags() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let _tag2 = create_test_tag("Transport", &connection);
        let tag3 = create_test_tag("Entertainment", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // Add tags to transaction
        add_tag_to_transaction(transaction.id(), tag1.id, &connection).expect("Could not add tag1");
        add_tag_to_transaction(transaction.id(), tag3.id, &connection).expect("Could not add tag3");
        // Intentionally not adding tag2

        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        let tag_set: HashSet<_> = tags.into_iter().collect();

        let expected_set = HashSet::from([tag3, tag1]); // Note: should be sorted by name
        assert_eq!(tag_set, expected_set);
    }

    #[test]
    fn get_transaction_tags_returns_empty_for_no_tags() {
        let connection = get_test_connection();
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");

        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn set_transaction_tags_replaces_existing_tags() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let tag2 = create_test_tag("Transport", &connection);
        let tag3 = create_test_tag("Entertainment", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // First add some tags
        add_tag_to_transaction(transaction.id(), tag1.id, &connection).expect("Could not add tag1");
        add_tag_to_transaction(transaction.id(), tag2.id, &connection).expect("Could not add tag2");

        // Replace with different set of tags
        let new_tag_ids = vec![tag2.id, tag3.id];
        let result = set_transaction_tags(transaction.id(), &new_tag_ids, &connection);

        assert!(result.is_ok());

        // Verify the tags were replaced
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        let tag_set: HashSet<_> = tags.into_iter().collect();

        let expected_set = HashSet::from([tag2, tag3]);
        assert_eq!(tag_set, expected_set);
    }

    #[test]
    fn set_transaction_tags_with_empty_list_removes_all() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let tag2 = create_test_tag("Transport", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // First add some tags
        add_tag_to_transaction(transaction.id(), tag1.id, &connection).expect("Could not add tag1");
        add_tag_to_transaction(transaction.id(), tag2.id, &connection).expect("Could not add tag2");

        // Set to empty list
        let result = set_transaction_tags(transaction.id(), &[], &connection);

        assert!(result.is_ok());

        // Verify all tags were removed
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 0);
    }

    // ============================================================================
    // ERROR HANDLING TESTS
    // ============================================================================

    #[test]
    fn add_tag_to_transaction_fails_with_invalid_tag_id() {
        let connection = get_test_connection();
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);
        let invalid_tag_id = 999999; // Non-existent tag ID

        let result = add_tag_to_transaction(transaction.id(), invalid_tag_id, &connection);

        assert!(matches!(result, Err(Error::InvalidTag)));
    }

    #[test]
    fn set_transaction_tags_fails_with_invalid_tag_id() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);
        let invalid_tag_id = 999999; // Non-existent tag ID

        let invalid_tag_ids = vec![tag1.id, invalid_tag_id];
        let result = set_transaction_tags(transaction.id(), &invalid_tag_ids, &connection);

        assert!(matches!(result, Err(Error::InvalidTag)));

        // Verify transaction was rolled back - no tags should be added
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn remove_tag_from_transaction_succeeds_with_non_existent_relationship() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // Try to remove a tag that was never added
        let result = remove_tag_from_transaction(transaction.id(), tag.id, &connection);

        // Should succeed (idempotent operation)
        assert!(result.is_ok());
    }

    #[test]
    fn functions_handle_non_existent_transaction_id() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let invalid_transaction_id = 999999; // Non-existent transaction ID

        // Adding a tag to non-existent transaction should fail due to foreign key constraint
        let add_result = add_tag_to_transaction(invalid_transaction_id, tag.id, &connection);
        assert!(add_result.is_err());

        // Removing from non-existent transaction should succeed (idempotent)
        let remove_result =
            remove_tag_from_transaction(invalid_transaction_id, tag.id, &connection);
        assert!(remove_result.is_ok());

        // Getting tags for non-existent transaction should succeed and return empty
        let get_result = get_transaction_tags(invalid_transaction_id, &connection);
        assert!(get_result.is_ok());
        assert_eq!(get_result.unwrap().len(), 0);
    }

    // ============================================================================
    // EDGE CASE AND DATA INTEGRITY TESTS
    // ============================================================================

    #[test]
    fn add_duplicate_tag_to_transaction_fails_due_to_unique_constraint() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // Add tag once
        add_tag_to_transaction(transaction.id(), tag.id, &connection)
            .expect("Could not add tag first time");

        // Try to add the same tag again
        let result = add_tag_to_transaction(transaction.id(), tag.id, &connection);

        // Should fail due to unique constraint
        assert!(result.is_err());
    }

    #[test]
    fn multiple_transactions_can_have_same_tag() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction1 = create_test_transaction(50.0, "Store purchase", &connection);
        let transaction2 = create_test_transaction(30.0, "Market purchase", &connection);

        // Add same tag to both transactions
        add_tag_to_transaction(transaction1.id(), tag.id, &connection)
            .expect("Could not add tag to transaction1");
        add_tag_to_transaction(transaction2.id(), tag.id, &connection)
            .expect("Could not add tag to transaction2");

        // Verify both transactions have the tag
        let tags1 = get_transaction_tags(transaction1.id(), &connection)
            .expect("Could not get tags for transaction1");
        let tags2 = get_transaction_tags(transaction2.id(), &connection)
            .expect("Could not get tags for transaction2");

        assert_eq!(tags1.len(), 1);
        assert_eq!(tags2.len(), 1);
        assert_eq!(tags1[0], tag);
        assert_eq!(tags2[0], tag);
    }

    #[test]
    fn set_transaction_tags_is_atomic() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let tag2 = create_test_tag("Transport", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);
        let invalid_tag_id = 999999;

        // First add a tag
        add_tag_to_transaction(transaction.id(), tag1.id, &connection)
            .expect("Could not add initial tag");

        // Try to set tags with one valid and one invalid ID
        let mixed_tag_ids = vec![tag2.id, invalid_tag_id];
        let result = set_transaction_tags(transaction.id(), &mixed_tag_ids, &connection);

        assert!(matches!(result, Err(Error::InvalidTag)));

        // Verify the original tag is still there (transaction was rolled back)
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], tag1);
    }
}

#[cfg(test)]
mod edit_tag_endpoint_tests {
    use std::sync::{Arc, Mutex};

    use askama_axum::IntoResponse;
    use axum::{
        Form,
        extract::{Path, State},
        http::StatusCode,
        response::Response,
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

        let response = get_edit_tag_page(Path(tag.id), State(state)).await;

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

        let response = get_edit_tag_page(Path(invalid_id), State(state)).await;

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

        let html = parse_fragment_html(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_error_message(&form, "Tag not found");
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

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

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
    fn must_get_form(html: &Html) -> ElementRef {
        html.select(&scraper::Selector::parse("form").unwrap())
            .next()
            .expect("No form found")
    }

    #[track_caller]
    fn assert_hx_endpoint(form: &ElementRef, endpoint: &str, attribute: &str) {
        let hx_attr = form
            .value()
            .attr(attribute)
            .expect(&format!("{attribute} attribute missing"));

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

    use askama_axum::IntoResponse;
    use axum::{
        extract::{Path, State},
        http::StatusCode,
        response::Response,
    };
    use rusqlite::Connection;
    use scraper::Html;

    use crate::{
        endpoints,
        tag::{TagName, create_tag, delete_tag_endpoint},
    };

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

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_hx_redirect(&response, endpoints::TAGS_VIEW);
    }

    #[tokio::test]
    async fn delete_tag_endpoint_with_invalid_id_returns_error_html() {
        let state = get_delete_tag_state();
        let invalid_id = 999999;

        let response = delete_tag_endpoint(Path(invalid_id), State(state))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            get_header(&response, "content-type"),
            "text/html; charset=utf-8"
        );

        let html = parse_fragment_html(response).await;
        assert_valid_html(&html);
        assert_error_content(&html, "Tag not found");
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
