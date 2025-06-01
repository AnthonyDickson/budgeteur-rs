//! This files defines the API routes for the category type.

use axum::{
    Form,
    extract::State,
    http::{StatusCode, Uri},
    response::IntoResponse,
};

use axum_htmx::HxRedirect;
use serde::{Deserialize, Serialize};

use crate::{models::CategoryName, state::CategoryState, stores::CategoryStore};

use super::{endpoints, templates::NewCategoryFormTemplate};

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
        models::{Category, CategoryName, DatabaseID},
        routes::{category::create_category, endpoints},
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
