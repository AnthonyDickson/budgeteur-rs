//! This file defines the `Category` type, the types needed to create a category and the API routes for the category type.
//! A category acts like a tag for a transaction, however a transaction may only have one category.

use std::fmt::Display;
use std::sync::{Arc, Mutex};

use askama_axum::Template;
use axum::{
    Form,
    extract::{FromRef, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, Error,
    models::DatabaseID,
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
        templates::NewCategoryFormTemplate,
    },
    stores::TransactionStore,
};

/// The name of a category.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct CategoryName(String);

impl CategoryName {
    /// Create a category name.
    ///
    /// # Errors
    ///
    /// This function will return an error if `name` is an empty string.
    pub fn new(name: &str) -> Result<Self, Error> {
        if name.is_empty() {
            Err(Error::EmptyCategoryName)
        } else {
            Ok(Self(name.to_string()))
        }
    }

    /// Create a category name without validation.
    ///
    /// The caller should ensure that the string is not empty.
    ///
    /// This function has `_unchecked` in the name but is not `unsafe`, because if the non-empty invariant is violated it will cause incorrect behaviour but not affect memory safety.
    pub fn new_unchecked(name: &str) -> Self {
        Self(name.to_string())
    }
}

impl AsRef<str> for CategoryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Display for CategoryName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A category for expenses and income, e.g., 'Groceries', 'Eating Out', 'Wages'.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Category {
    /// The id of the category.
    pub id: DatabaseID,

    /// The name of the category.
    pub name: CategoryName,
}

/// Renders the new Category page.
#[derive(Template)]
#[template(path = "views/new_category.html")]
struct NewCategoryTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    form: NewCategoryFormTemplate<'a>,
}

pub async fn get_new_category_page() -> Response {
    NewCategoryTemplate {
        nav_bar: get_nav_bar(endpoints::NEW_CATEGORY_VIEW),
        form: NewCategoryFormTemplate {
            category_route: endpoints::CATEGORIES,
            error_message: "",
        },
    }
    .into_response()
}

/// The state needed for creating a category.
#[derive(Debug, Clone)]
pub struct CategoryState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl<T> FromRef<AppState<T>> for CategoryState
where
    T: TransactionStore + Send + Sync,
{
    fn from_ref(state: &AppState<T>) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryData {
    pub name: String,
}

/// A route handler for creating a new category.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_category_endpoint(
    State(state): State<CategoryState>,
    Form(new_category): Form<CategoryData>,
) -> impl IntoResponse {
    let name = match CategoryName::new(&new_category.name) {
        Ok(name) => name,
        Err(error) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                NewCategoryFormTemplate {
                    category_route: endpoints::CATEGORIES,
                    error_message: &format!("Error: {error}"),
                },
            )
                .into_response();
        }
    };

    create_category(
        name,
        &state
            .db_connection
            .lock()
            .expect("Could not acquire database lock"),
    )
    .map(|_category| {
        (
            HxRedirect(Uri::from_static(endpoints::NEW_TRANSACTION_VIEW)),
            StatusCode::SEE_OTHER,
        )
    })
    .map_err(|error| {
        tracing::error!("An unexpected error occurred while creating a category: {error}");

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            NewCategoryFormTemplate {
                category_route: endpoints::CATEGORIES,
                error_message: "An unexpected error occurred. Please try again.",
            },
        )
    })
    .into_response()
}

/// Create a category in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn create_category(name: CategoryName, connection: &Connection) -> Result<Category, Error> {
    connection.execute("INSERT INTO category (name) VALUES (?1);", (name.as_ref(),))?;

    let id = connection.last_insert_rowid();

    Ok(Category { id, name })
}

/// Retrieve categories in the database for the category with `category_id`.
///
/// # Errors
/// This function will return an error if there is an SQL error.
// TODO: Remove build config attribute once get_category function is used elsewhere.
#[cfg(test)]
fn get_category(category_id: DatabaseID, connection: &Connection) -> Result<Category, Error> {
    connection
        .prepare("SELECT id, name FROM category WHERE id = :id;")?
        .query_row(&[(":id", &category_id)], map_row)
        .map_err(|error| error.into())
}

/// Retrieve categories in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn get_all_categories(connection: &Connection) -> Result<Vec<Category>, Error> {
    connection
        .prepare("SELECT id, name FROM category;")?
        .query_map([], map_row)?
        .map(|maybe_category| maybe_category.map_err(|error| error.into()))
        .collect()
}

pub fn create_category_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS category (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL UNIQUE
            );",
        (),
    )?;

    Ok(())
}

fn map_row(row: &Row) -> Result<Category, rusqlite::Error> {
    let id = row.get(0)?;
    let raw_name: String = row.get(1)?;
    let name = CategoryName::new_unchecked(&raw_name);

    Ok(Category { id, name })
}

#[cfg(test)]
mod category_name_tests {
    use crate::{Error, category::CategoryName};

    #[test]
    fn new_fails_on_empty_string() {
        let category_name = CategoryName::new("");

        assert_eq!(category_name, Err(Error::EmptyCategoryName));
    }

    #[test]
    fn new_succeeds_on_non_empty_string() {
        let category_name = CategoryName::new("🔥");

        assert!(category_name.is_ok())
    }
}

#[cfg(test)]
mod category_query_tests {
    use std::collections::HashSet;

    use rusqlite::Connection;

    use crate::category::{create_category, get_all_categories, get_category};
    use crate::{Error, category::CategoryName};

    use super::create_category_table;

    fn get_test_db_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        create_category_table(&connection).expect("Could not create category table");
        connection
    }

    #[test]
    fn create_category_succeeds() {
        let connection = get_test_db_connection();
        let name = CategoryName::new("Categorically a category").unwrap();

        let category = create_category(name.clone(), &connection);

        let category = category.expect("Could not create category");
        assert!(category.id > 0);
        assert_eq!(category.name, name);
    }

    #[test]
    fn get_category_succeeds() {
        let connection = get_test_db_connection();
        let name = CategoryName::new_unchecked("Foo");
        let inserted_category =
            create_category(name, &connection).expect("Could not create test category");

        let selected_category = get_category(inserted_category.id, &connection);

        assert_eq!(Ok(inserted_category), selected_category);
    }

    #[test]
    fn get_category_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let inserted_category = create_category(CategoryName::new_unchecked("Foo"), &connection)
            .expect("Could not create test category");

        let selected_category = get_category(inserted_category.id + 123, &connection);

        assert_eq!(selected_category, Err(Error::NotFound));
    }

    #[test]
    fn test_get_all_categories() {
        let store = get_test_db_connection();

        let inserted_categories = HashSet::from([
            create_category(CategoryName::new_unchecked("Foo"), &store)
                .expect("Could not create test category"),
            create_category(CategoryName::new_unchecked("Bar"), &store)
                .expect("Could not create test category"),
        ]);

        let selected_categories = get_all_categories(&store).expect("Could not get all categories");
        let selected_categories = HashSet::from_iter(selected_categories);

        assert_eq!(inserted_categories, selected_categories);
    }
}

#[cfg(test)]
mod new_category_page_tests {
    use axum::{http::StatusCode, response::Response};
    use scraper::{ElementRef, Html};

    use crate::{category::get_new_category_page, routes::endpoints};

    #[tokio::test]
    async fn render_page() {
        let response = get_new_category_page().await;

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
        assert_hx_endpoint(&form, endpoints::CATEGORIES);
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
mod create_category_endpoint_tests {
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
        category::{Category, CategoryName, create_category_endpoint, get_category},
        routes::endpoints,
    };

    use super::{CategoryData, CategoryState, create_category_table};

    fn get_category_state() -> CategoryState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_category_table(&connection).expect("Could not create category table");

        CategoryState {
            db_connection: Arc::new(Mutex::new(connection)),
        }
    }

    #[tokio::test]
    async fn can_create_category() {
        let state = get_category_state();
        let name = CategoryName::new_unchecked("Foo");
        let want = Category {
            id: 1,
            name: name.clone(),
        };
        let form = CategoryData {
            name: name.to_string(),
        };

        let response = create_category_endpoint(State(state.clone()), Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_hx_redirect(&response, endpoints::NEW_TRANSACTION_VIEW);
        assert_eq!(
            Ok(want),
            get_category(1, &state.db_connection.lock().unwrap())
        );
    }

    #[tokio::test]
    async fn create_category_fails_on_empty_name() {
        let state = get_category_state();
        let form = CategoryData {
            name: "".to_string(),
        };

        let response = create_category_endpoint(State(state), Form(form))
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
        assert_error_message(&form, "Error: Category name cannot be empty");
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
