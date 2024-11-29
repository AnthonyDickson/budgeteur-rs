//! This files defines the routes for the transaction type.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Form, Json,
};
use axum_extra::extract::PrivateCookieJar;
use serde::Deserialize;
use time::Date;

use crate::{
    auth::get_user_id_from_auth_cookie,
    models::{DatabaseID, Transaction, UserID},
    stores::{CategoryStore, TransactionStore, UserStore},
    AppError, AppState,
};

use super::templates::TransactionRow;

/// The form data for creating a transaction.
#[derive(Debug, Deserialize)]
pub struct TransactionForm {
    /// The value of the transaction in dollars.
    pub amount: f64,
    /// The date when the transaction ocurred.
    pub date: Date,
    /// Text detailing the transaction.
    pub description: String,
    /// The ID of the category to assign the transaction to.
    ///
    /// Zero should be interpreted as `None`.
    pub category_id: DatabaseID,
}

/// A route handler for creating a new transaction, returns [TransactionRow] as a [Response] on success.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_transaction<C, T, U>(
    State(mut state): State<AppState<C, T, U>>,
    _jar: PrivateCookieJar,
    Path(user_id): Path<UserID>,
    Form(data): Form<TransactionForm>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    // HACK: Zero is used as a sentinel value for None. Currently, options do not work with empty
    // form values. For example, the URL encoded form "num=" will return an error.
    let category = match data.category_id {
        0 => None,
        id => Some(id),
    };

    let transaction = Transaction::build(data.amount, user_id)
        .description(data.description)
        .category(category)
        .date(data.date)?;

    state
        .transaction_store()
        .create_from_builder(transaction)
        .map(|transaction| (StatusCode::OK, TransactionRow { transaction }))
        .map_err(AppError::TransactionError)
}

/// A route handler for getting a transaction by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_transaction<C, T, U>(
    State(mut state): State<AppState<C, T, U>>,
    jar: PrivateCookieJar,
    Path(transaction_id): Path<DatabaseID>,
) -> impl IntoResponse
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    state
        .transaction_store()
        .get(transaction_id)
        .map_err(AppError::TransactionError)
        .and_then(|transaction| {
            if get_user_id_from_auth_cookie(jar)? == transaction.user_id() {
                Ok(transaction)
            } else {
                // Respond with 404 not found so that unauthorized users cannot know whether another user's resource exists.
                Err(AppError::NotFound)
            }
        })
        .map(|transaction| (StatusCode::OK, Json(transaction)))
}

#[cfg(test)]
mod transaction_tests {
    use std::sync::{Arc, Mutex};

    use askama_axum::IntoResponse;
    use axum::body::Body;
    use axum::extract::{Path, State};
    use axum::http::{Response, StatusCode};
    use axum::Form;
    use axum_extra::extract::PrivateCookieJar;
    use time::OffsetDateTime;

    use crate::auth::set_auth_cookie;
    use crate::models::{
        CategoryError, DatabaseID, PasswordHash, TransactionBuilder, TransactionError,
    };
    use crate::routes::transaction::{create_transaction, get_transaction, TransactionForm};
    use crate::stores::{CategoryStore, TransactionStore, UserStore};
    use crate::{
        models::{Category, Transaction, UserID},
        AppState,
    };

    #[derive(Clone)]
    struct DummyUserStore {}

    impl UserStore for DummyUserStore {
        fn create(
            &mut self,
            _email: email_address::EmailAddress,
            _password_hash: PasswordHash,
        ) -> Result<crate::models::User, crate::stores::UserError> {
            todo!()
        }

        fn get(&self, _id: UserID) -> Result<crate::models::User, crate::stores::UserError> {
            todo!()
        }

        fn get_by_email(
            &self,
            _email: &email_address::EmailAddress,
        ) -> Result<crate::models::User, crate::stores::UserError> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyCategoryStore {}

    impl CategoryStore for DummyCategoryStore {
        fn create(
            &self,
            _name: crate::models::CategoryName,
            _user_id: UserID,
        ) -> Result<Category, CategoryError> {
            todo!()
        }

        fn get(&self, _category_id: DatabaseID) -> Result<Category, CategoryError> {
            todo!()
        }

        fn get_by_user(&self, _user_id: UserID) -> Result<Vec<Category>, CategoryError> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct FakeTransactionStore {
        transactions: Vec<Transaction>,
        create_calls: Arc<Mutex<Vec<Transaction>>>,
    }

    impl FakeTransactionStore {
        fn new() -> Self {
            Self {
                transactions: Vec::new(),
                create_calls: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl TransactionStore for FakeTransactionStore {
        fn create(
            &mut self,
            amount: f64,
            user_id: UserID,
        ) -> Result<Transaction, TransactionError> {
            self.create_from_builder(TransactionBuilder::new(amount, user_id))
        }

        fn create_from_builder(
            &mut self,
            builder: TransactionBuilder,
        ) -> Result<Transaction, TransactionError> {
            let next_id = match self.transactions.last() {
                Some(transaction) => transaction.id() + 1,
                None => 0,
            };

            let transaction = builder.finalise(next_id);

            self.transactions.push(transaction.clone());
            self.create_calls.lock().unwrap().push(transaction.clone());

            Ok(transaction)
        }

        fn get(&self, id: DatabaseID) -> Result<Transaction, TransactionError> {
            self.transactions
                .iter()
                .find(|transaction| transaction.id() == id)
                .ok_or(TransactionError::NotFound)
                .map(|transaction| transaction.to_owned())
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, TransactionError> {
            todo!()
        }

        fn get_filtered(
            &self,
            _filter: crate::stores::transaction::TransactionFilter,
        ) -> Result<Vec<Transaction>, TransactionError> {
            todo!()
        }
    }

    #[tokio::test]
    async fn can_create_transaction() {
        let state = AppState::new(
            "42",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );

        let jar = PrivateCookieJar::new(state.cookie_key().to_owned());

        let user_id = UserID::new(123);

        let want = Transaction::build(12.3, user_id)
            .date(OffsetDateTime::now_utc().date())
            .unwrap()
            .description("aaaaaaaaaaaaa".to_string())
            .category(Some(1))
            .finalise(0);

        let form = TransactionForm {
            description: want.description().to_string(),
            amount: want.amount(),
            date: want.date().to_owned(),
            category_id: want.category_id().unwrap(),
        };

        let response = create_transaction(State(state.clone()), jar, Path(user_id), Form(form))
            .await
            .into_response();

        assert_create_calls(state, want.clone());
        assert_response_contains_transaction(response, want).await;
    }

    #[tokio::test]
    async fn can_get_transaction() {
        let user_id = UserID::new(42);

        let mut state = AppState::new(
            "42",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );

        let transaction = state
            .transaction_store()
            .create_from_builder(
                TransactionBuilder::new(13.34, user_id)
                    .category(Some(24))
                    .description("foobar".to_string()),
            )
            .unwrap();

        let jar = PrivateCookieJar::new(state.cookie_key().to_owned());
        let jar = set_auth_cookie(jar, user_id);

        let response = get_transaction(State(state), jar, Path(transaction.id()))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        let json_response = extract_from_json(response).await;

        assert_eq!(json_response, transaction);
    }

    #[tokio::test]
    async fn cannot_get_transaction_with_unauthorized_user() {
        let user_id = UserID::new(42);
        let unauthorized_user_id = UserID::new(1337);

        let mut state = AppState::new(
            "42",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );

        let transaction = state
            .transaction_store()
            .create_from_builder(
                TransactionBuilder::new(12.34, user_id)
                    .category(Some(24))
                    .description("foobar".to_string()),
            )
            .unwrap();

        let jar = PrivateCookieJar::new(state.cookie_key().to_owned());
        let jar = set_auth_cookie(jar, unauthorized_user_id);

        let response = get_transaction(State(state), jar, Path(transaction.id()))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    async fn extract_text(response: Response<Body>) -> String {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        String::from_utf8_lossy(&body).to_string()
    }

    async fn extract_from_json(response: Response<Body>) -> Transaction {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        serde_json::from_slice(&body).unwrap()
    }

    fn assert_create_calls(
        mut state: AppState<DummyCategoryStore, FakeTransactionStore, DummyUserStore>,
        want: Transaction,
    ) {
        let create_calls = state
            .transaction_store()
            .create_calls
            .lock()
            .unwrap()
            .clone();

        assert_eq!(
            create_calls.len(),
            1,
            "got {} calls to create transaction, want 1",
            create_calls.len()
        );

        let got = &create_calls[0];
        assert_eq!(got, &want, "got transaction {:#?} want {:#?}", got, want);
    }

    async fn assert_response_contains_transaction(response: Response<Body>, want: Transaction) {
        assert_eq!(response.status(), StatusCode::OK);

        let html_response = extract_text(response).await;

        assert!(html_response.contains(&want.amount().to_string()));
        assert!(html_response.contains(&want.date().to_string()));
        assert!(html_response.contains(want.description()));
        assert!(html_response.contains(
            &want
                .category_id()
                .expect("category id should not be None")
                .to_string()
        ));
    }
}
