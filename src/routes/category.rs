//! This files defines the API routes for the category type.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Form, Json,
};
use axum_extra::extract::PrivateCookieJar;

use serde::{Deserialize, Serialize};

use crate::{
    auth::cookie::get_user_id_from_auth_cookie,
    models::{CategoryName, DatabaseID, UserID},
    stores::{CategoryStore, TransactionStore, UserStore},
    AppError, AppState,
};

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
    Path(user_id): Path<UserID>,
    _: PrivateCookieJar,
    Form(new_category): Form<CategoryData>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let name = CategoryName::new(&new_category.name)?;

    state
        .category_store()
        .create(name, user_id)
        .map(|category| (StatusCode::OK, Json(category)))
        .map_err(AppError::CategoryError)
}

/// A route handler for getting a category by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_category<C, T, U>(
    State(state): State<AppState<C, T, U>>,
    jar: PrivateCookieJar,
    Path(category_id): Path<DatabaseID>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    state
        .category_store()
        .get(category_id)
        .map_err(AppError::CategoryError)
        .and_then(|category| {
            let user_id = get_user_id_from_auth_cookie(&jar)?;

            if user_id == category.user_id() {
                Ok(category)
            } else {
                // Respond with 404 not found so that unauthorized users cannot know whether another user's resource exists.
                Err(AppError::NotFound)
            }
        })
        .map(|category| (StatusCode::OK, Json(category)))
}

#[cfg(test)]
mod category_tests {
    use std::sync::{Arc, Mutex};

    use askama_axum::IntoResponse;
    use axum::{
        extract::{Path, State},
        http::StatusCode,
        Form,
    };
    use axum_extra::extract::{cookie::Key, PrivateCookieJar};

    use crate::{
        auth::cookie::{set_auth_cookie, COOKIE_DURATION},
        models::{
            Category, CategoryError, CategoryName, DatabaseID, PasswordHash, Transaction,
            TransactionBuilder, TransactionError, User, UserID,
        },
        routes::category::{create_category, get_category},
        stores::{transaction::TransactionQuery, CategoryStore, TransactionStore, UserStore},
        AppState,
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
        fn create(&self, name: CategoryName, user_id: UserID) -> Result<Category, CategoryError> {
            self.create_calls.lock().unwrap().push(CreateCategoryCall {
                name: name.clone(),
                user_id,
            });

            let category = Category::new(0, name, user_id);
            self.categories.lock().unwrap().push(category.clone());

            Ok(category)
        }

        fn get(&self, category_id: DatabaseID) -> Result<Category, CategoryError> {
            self.get_calls
                .lock()
                .unwrap()
                .push(GetCategoryCall { category_id });

            self.categories
                .lock()
                .unwrap()
                .iter()
                .find(|category| category.id() == category_id)
                .ok_or(CategoryError::NotFound)
                .map(|category| category.to_owned())
        }

        fn get_by_user(&self, _user_id: UserID) -> Result<Vec<Category>, CategoryError> {
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
        ) -> Result<User, crate::stores::UserError> {
            todo!()
        }

        fn get(&self, _id: UserID) -> Result<User, crate::stores::UserError> {
            todo!()
        }

        fn get_by_email(
            &self,
            _email: &email_address::EmailAddress,
        ) -> Result<User, crate::stores::UserError> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyTransactionStore {}

    impl TransactionStore for DummyTransactionStore {
        fn create(
            &mut self,
            _amount: f64,
            _user_id: UserID,
        ) -> Result<Transaction, TransactionError> {
            todo!()
        }

        fn create_from_builder(
            &mut self,
            _builder: TransactionBuilder,
        ) -> Result<Transaction, TransactionError> {
            todo!()
        }

        fn get(&self, _id: DatabaseID) -> Result<Transaction, TransactionError> {
            todo!()
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, TransactionError> {
            todo!()
        }

        fn get_query(
            &self,
            _filter: TransactionQuery,
        ) -> Result<Vec<Transaction>, TransactionError> {
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
        let jar = get_cookie_jar(want.user_id, state.cookie_key().to_owned());

        let response = create_category(State(state), Path(want.user_id), jar, Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_create_calls(&store, &want);
    }

    #[tokio::test]
    async fn create_category_fails_on_empty_name() {
        let (state, _store) = get_test_app_config();

        let user_id = UserID::new(123);

        let form = CategoryData {
            name: "".to_string(),
        };
        let jar = get_cookie_jar(user_id, state.cookie_key().to_owned());

        let response = create_category(State(state), Path(user_id), jar, Form(form))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn can_get_category() {
        let (state, store) = get_test_app_config();

        let category = store
            .create(CategoryName::new_unchecked("Foo"), UserID::new(123))
            .unwrap();

        let want = GetCategoryCall {
            category_id: category.id(),
        };

        let jar = get_cookie_jar(category.user_id(), state.cookie_key().to_owned());

        let response = get_category(State(state), jar, Path(category.id()))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_get_calls(&store, &want);
    }

    #[tokio::test]
    async fn get_category_fails_on_wrong_user() {
        let (state, store) = get_test_app_config();

        let category = store
            .create(CategoryName::new_unchecked("Foo"), UserID::new(123))
            .unwrap();
        let unauthorized_user_id = UserID::new(category.user_id().as_i64() + 999);

        let want = GetCategoryCall {
            category_id: category.id(),
        };
        let jar = get_cookie_jar(unauthorized_user_id, state.cookie_key().to_owned());

        let response = get_category(State(state), jar, Path(category.id()))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_get_calls(&store, &want);
    }

    fn get_cookie_jar(user_id: UserID, key: Key) -> PrivateCookieJar {
        let jar = PrivateCookieJar::new(key);
        set_auth_cookie(jar, user_id, COOKIE_DURATION).unwrap()
    }

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

    fn assert_get_calls(store: &SpyCategoryStore, want: &GetCategoryCall) {
        let get_calls = store.get_calls.lock().unwrap().clone();
        assert!(
            get_calls.len() == 1,
            "got {} calls to route handler 'get_category', want 1",
            get_calls.len()
        );

        let got = get_calls.first().unwrap();
        assert_eq!(
            got, want,
            "got call to CategoryStore.get {:?}, want {:?}",
            got, want
        );
    }
}
