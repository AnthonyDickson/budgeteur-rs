//! This files defines the API routes for the category type.

use askama_axum::Template;
use axum::{
    Form,
    extract::State,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use serde::{Deserialize, Serialize};

use crate::{
    models::CategoryName,
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
        templates::NewCategoryFormTemplate,
    },
    state::CategoryState,
    stores::CategoryStore,
};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryData {
    pub name: String,
}

/// A route handler for creating a new category.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_category<C>(
    State(state): State<CategoryState<C>>,
    Form(new_category): Form<CategoryData>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
{
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

    state
        .category_store
        .create(name)
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

#[cfg(test)]
mod new_category_tests {
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
mod category_tests {
    use std::sync::{Arc, Mutex};

    use askama_axum::IntoResponse;
    use axum::{
        Form,
        extract::State,
        http::{StatusCode, header::CONTENT_TYPE},
        response::Response,
    };
    use scraper::{ElementRef, Html};

    use crate::{
        Error,
        category::create_category,
        models::{Category, CategoryName, DatabaseID},
        routes::endpoints,
        state::CategoryState,
        stores::CategoryStore,
    };

    use super::CategoryData;

    #[derive(Debug, Clone, PartialEq)]
    struct CreateCategoryCall {
        name: CategoryName,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct GetCategoryCall {
        category_id: DatabaseID,
    }

    #[derive(Clone)]
    struct SpyCategoryStore {
        // Use Arc Mutex so that clones of the store share state and can be passed into async route
        // handlers.
        create_calls: Arc<Mutex<Vec<CreateCategoryCall>>>,
        get_calls: Arc<Mutex<Vec<GetCategoryCall>>>,
        categories: Arc<Mutex<Vec<Category>>>,
    }

    impl CategoryStore for SpyCategoryStore {
        fn create(&self, name: CategoryName) -> Result<Category, Error> {
            self.create_calls
                .lock()
                .unwrap()
                .push(CreateCategoryCall { name: name.clone() });

            let category = Category { id: 0, name };
            self.categories.lock().unwrap().push(category.clone());

            Ok(category)
        }

        fn get(&self, category_id: DatabaseID) -> Result<Category, Error> {
            self.get_calls
                .lock()
                .unwrap()
                .push(GetCategoryCall { category_id });

            self.categories
                .lock()
                .unwrap()
                .iter()
                .find(|category| category.id == category_id)
                .ok_or(Error::NotFound)
                .map(|category| category.to_owned())
        }

        fn get_all(&self) -> Result<Vec<Category>, Error> {
            todo!()
        }
    }

    fn get_test_app_config() -> (CategoryState<SpyCategoryStore>, SpyCategoryStore) {
        let store = SpyCategoryStore {
            create_calls: Arc::new(Mutex::new(vec![])),
            get_calls: Arc::new(Mutex::new(vec![])),
            categories: Arc::new(Mutex::new(vec![])),
        };

        let state = CategoryState {
            category_store: store.clone(),
        };

        (state, store)
    }

    #[tokio::test]
    async fn can_create_category() {
        let (state, store) = get_test_app_config();
        let want = CreateCategoryCall {
            name: CategoryName::new_unchecked("Foo"),
        };

        let form = CategoryData {
            name: want.name.to_string(),
        };

        let response = create_category(State(state), Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_hx_redirect(&response, endpoints::NEW_TRANSACTION_VIEW);
        assert_create_calls(&store, &want);
    }

    #[tokio::test]
    async fn create_category_fails_on_empty_name() {
        let (state, _store) = get_test_app_config();
        let form = CategoryData {
            name: "".to_string(),
        };

        let response = create_category(State(state), Form(form))
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
    fn assert_create_calls(store: &SpyCategoryStore, want: &CreateCategoryCall) {
        let create_calls = store.create_calls.lock().unwrap().clone();
        assert!(
            create_calls.len() == 1,
            "got {} calls to route handler 'create_category', want 1",
            create_calls.len()
        );

        let got = create_calls.first().unwrap();
        assert_eq!(
            got, want,
            "got call to CategoryStore.create {:?}, want {:?}",
            got, want
        );
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
