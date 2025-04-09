//! This files defines the API routes for the category type.

use axum::{
    Form,
    extract::State,
    http::{StatusCode, Uri},
    response::IntoResponse,
};

use axum_extra::extract::PrivateCookieJar;
use axum_htmx::HxRedirect;
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    auth::cookie::get_user_id_from_auth_cookie,
    models::CategoryName,
    stores::{CategoryStore, TransactionStore, UserStore},
};

use super::endpoints;

#[derive(Debug, Serialize, Deserialize)]
pub struct CategoryData {
    pub name: String,
}

/// A route handler for creating a new category.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_category<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    jar: PrivateCookieJar,
    Form(new_category): Form<CategoryData>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let user_id = get_user_id_from_auth_cookie(&jar)?;
    let name = CategoryName::new(&new_category.name)?;

    state.category_store.create(name, user_id).map(|_category| {
        (
            HxRedirect(Uri::from_static(endpoints::CATEGORIES)),
            StatusCode::SEE_OTHER,
        )
    })
}

#[cfg(test)]
mod category_tests {
    use std::sync::{Arc, Mutex};

    use askama_axum::IntoResponse;
    use axum::{
        Form,
        body::Body,
        extract::State,
        http::{Response, StatusCode},
    };
    use axum_extra::extract::{PrivateCookieJar, cookie::Key};

    use crate::{
        AppState, Error,
        auth::cookie::{DEFAULT_COOKIE_DURATION, set_auth_cookie},
        models::{
            Category, CategoryName, DatabaseID, PasswordHash, Transaction, TransactionBuilder,
            User, UserID,
        },
        routes::{category::create_category, endpoints},
        stores::{CategoryStore, TransactionStore, UserStore, transaction::TransactionQuery},
    };

    use super::CategoryData;

    #[derive(Debug, Clone, PartialEq)]
    struct CreateCategoryCall {
        name: CategoryName,
        user_id: UserID,
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
        fn create(&self, name: CategoryName, user_id: UserID) -> Result<Category, Error> {
            self.create_calls.lock().unwrap().push(CreateCategoryCall {
                name: name.clone(),
                user_id,
            });

            let category = Category {
                id: 0,
                name,
                user_id,
            };
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

        fn get_by_user(&self, _user_id: UserID) -> Result<Vec<Category>, Error> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyUserStore {}

    impl UserStore for DummyUserStore {
        fn create(
            &mut self,
            _email: email_address::EmailAddress,
            _password_hash: PasswordHash,
        ) -> Result<User, Error> {
            todo!()
        }

        fn get(&self, _id: UserID) -> Result<User, Error> {
            todo!()
        }

        fn get_by_email(&self, _email: &email_address::EmailAddress) -> Result<User, Error> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyTransactionStore {}

    impl TransactionStore for DummyTransactionStore {
        fn create(&mut self, _amount: f64, _user_id: UserID) -> Result<Transaction, Error> {
            todo!()
        }

        fn create_from_builder(
            &mut self,
            _builder: TransactionBuilder,
        ) -> Result<Transaction, Error> {
            todo!()
        }

        fn get(&self, _id: DatabaseID) -> Result<Transaction, Error> {
            todo!()
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, Error> {
            todo!()
        }

        fn get_query(&self, _filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            todo!()
        }
    }

    fn get_test_app_config() -> (
        AppState<SpyCategoryStore, DummyTransactionStore, DummyUserStore>,
        SpyCategoryStore,
    ) {
        let store = SpyCategoryStore {
            create_calls: Arc::new(Mutex::new(vec![])),
            get_calls: Arc::new(Mutex::new(vec![])),
            categories: Arc::new(Mutex::new(vec![])),
        };

        let state = AppState::new(
            "42",
            store.clone(),
            DummyTransactionStore {},
            DummyUserStore {},
        );

        (state, store)
    }

    #[tokio::test]
    async fn can_create_category() {
        let (state, store) = get_test_app_config();

        let want = CreateCategoryCall {
            user_id: UserID::new(123),
            name: CategoryName::new_unchecked("Foo"),
        };

        let form = CategoryData {
            name: want.name.to_string(),
        };
        let jar = get_cookie_jar(want.user_id, state.cookie_key.clone());

        let response = create_category(State(state), jar, Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_hx_redirect(&response, endpoints::CATEGORIES);
        assert_create_calls(&store, &want);
    }

    #[tokio::test]
    async fn create_category_fails_on_empty_name() {
        let (state, _store) = get_test_app_config();

        let user_id = UserID::new(123);

        let form = CategoryData {
            name: "".to_string(),
        };
        let jar = get_cookie_jar(user_id, state.cookie_key.clone());

        let response = create_category(State(state), jar, Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        // TODO: Assert HTML content type
        // TODO: Assert renders new category form with error message
    }

    fn get_cookie_jar(user_id: UserID, key: Key) -> PrivateCookieJar {
        let jar = PrivateCookieJar::new(key);
        set_auth_cookie(jar, user_id, DEFAULT_COOKIE_DURATION).unwrap()
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
    fn assert_hx_redirect(response: &Response<Body>, endpoint: &str) {
        assert_eq!(
            response
                .headers()
                .get("hx-redirect")
                .expect("Headers missing hx-redirect")
                .to_str()
                .expect("Could not convert to str"),
            endpoint,
        );
    }
}
