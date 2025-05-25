//! This files defines the routes for the transaction type.

use axum::{
    Extension, Form, Json,
    extract::{Path, State},
    http::{StatusCode, Uri},
    response::IntoResponse,
};
use axum_htmx::HxRedirect;
use serde::Deserialize;
use time::Date;

use crate::{
    Error,
    models::{DatabaseID, Transaction, UserID},
    state::TransactionState,
    stores::TransactionStore,
};

use super::endpoints;

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
pub async fn create_transaction<T>(
    State(mut state): State<TransactionState<T>>,
    Extension(user_id): Extension<UserID>,
    Form(data): Form<TransactionForm>,
) -> impl IntoResponse
where
    T: TransactionStore + Send + Sync,
{
    // HACK: Zero is used as a sentinel value for None. Currently, options do not work with empty
    // form values. For example, the URL encoded form "num=" will return an error.
    let category = match data.category_id {
        0 => None,
        id => Some(id),
    };

    let transaction = Transaction::build(data.amount, user_id)
        .description(&data.description)
        .category(category)
        .date(data.date);

    let transaction = match transaction {
        Ok(transaction) => transaction,
        Err(e) => return e.into_response(),
    };

    match state.transaction_store.create_from_builder(transaction) {
        Ok(_) => {}
        Err(e) => return e.into_response(),
    }

    (
        HxRedirect(Uri::from_static(endpoints::TRANSACTIONS_VIEW)),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

/// A route handler for getting a transaction by its database ID.
///
/// This function will return the status code 404 if the requested resource does not exist (e.g., not created yet).
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_transaction<T>(
    State(state): State<TransactionState<T>>,
    Extension(logged_in_user_id): Extension<UserID>,
    Path(transaction_id): Path<DatabaseID>,
) -> impl IntoResponse
where
    T: TransactionStore + Send + Sync,
{
    state
        .transaction_store
        .get(transaction_id)
        .and_then(|transaction| {
            if logged_in_user_id == transaction.user_id() {
                Ok(transaction)
            } else {
                // Respond with 404 not found so that unauthorized users cannot know whether another user's resource exists.
                Err(Error::NotFound)
            }
        })
        .map(|transaction| (StatusCode::OK, Json(transaction)))
}

#[cfg(test)]
mod transaction_tests {
    use std::sync::{Arc, Mutex};

    use askama_axum::IntoResponse;
    use axum::{
        Extension, Form,
        body::Body,
        extract::{Path, State},
        http::{Response, StatusCode},
    };
    use axum_htmx::HX_REDIRECT;
    use time::OffsetDateTime;

    use crate::{Error, stores::TransactionStore};
    use crate::{
        models::{DatabaseID, Transaction, TransactionBuilder, UserID},
        routes::transaction::{TransactionForm, create_transaction, get_transaction},
        state::TransactionState,
        stores::transaction::TransactionQuery,
    };

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
        fn create(&mut self, amount: f64, user_id: UserID) -> Result<Transaction, Error> {
            self.create_from_builder(TransactionBuilder::new(amount, user_id))
        }

        fn create_from_builder(
            &mut self,
            builder: TransactionBuilder,
        ) -> Result<Transaction, Error> {
            let next_id = match self.transactions.last() {
                Some(transaction) => transaction.id() + 1,
                None => 0,
            };

            let transaction = builder.finalise(next_id);

            self.transactions.push(transaction.clone());
            self.create_calls.lock().unwrap().push(transaction.clone());

            Ok(transaction)
        }

        fn import(
            &mut self,
            _builders: Vec<TransactionBuilder>,
        ) -> Result<Vec<Transaction>, Error> {
            todo!()
        }

        fn get(&self, id: DatabaseID) -> Result<Transaction, Error> {
            self.transactions
                .iter()
                .find(|transaction| transaction.id() == id)
                .ok_or(Error::NotFound)
                .map(|transaction| transaction.to_owned())
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, Error> {
            todo!()
        }

        fn get_query(&self, _filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            todo!()
        }
    }

    #[tokio::test]
    async fn can_create_transaction() {
        let state = TransactionState {
            transaction_store: FakeTransactionStore::new(),
        };

        let user_id = UserID::new(123);

        let want = Transaction::build(12.3, user_id)
            .date(OffsetDateTime::now_utc().date())
            .unwrap()
            .description("aaaaaaaaaaaaa")
            .category(Some(1))
            .finalise(0);

        let form = TransactionForm {
            description: want.description().to_string(),
            amount: want.amount(),
            date: want.date().to_owned(),
            category_id: want.category_id().unwrap(),
        };

        let response = create_transaction(State(state.clone()), Extension(user_id), Form(form))
            .await
            .into_response();

        assert_create_calls(state, want.clone());
        assert_redirects_to_transactions_view(response);
    }

    #[tokio::test]
    async fn can_get_transaction() {
        let user_id = UserID::new(42);

        let mut state = TransactionState {
            transaction_store: FakeTransactionStore::new(),
        };

        let transaction = state
            .transaction_store
            .create_from_builder(
                TransactionBuilder::new(13.34, user_id)
                    .category(Some(24))
                    .description("foobar"),
            )
            .unwrap();

        let response = get_transaction(State(state), Extension(user_id), Path(transaction.id()))
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

        let mut state = TransactionState {
            transaction_store: FakeTransactionStore::new(),
        };

        let transaction = state
            .transaction_store
            .create_from_builder(
                TransactionBuilder::new(12.34, user_id)
                    .category(Some(24))
                    .description("foobar"),
            )
            .unwrap();

        let response = get_transaction(
            State(state),
            Extension(unauthorized_user_id),
            Path(transaction.id()),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    async fn extract_from_json(response: Response<Body>) -> Transaction {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        serde_json::from_slice(&body).unwrap()
    }

    #[track_caller]
    fn assert_create_calls(state: TransactionState<FakeTransactionStore>, want: Transaction) {
        let create_calls = state.transaction_store.create_calls.lock().unwrap().clone();

        assert_eq!(
            create_calls.len(),
            1,
            "got {} calls to create transaction, want 1",
            create_calls.len()
        );

        let got = &create_calls[0];
        assert_eq!(got, &want, "got transaction {:#?} want {:#?}", got, want);
    }

    #[track_caller]
    fn assert_redirects_to_transactions_view(response: Response<Body>) {
        let location = response
            .headers()
            .get(HX_REDIRECT)
            .expect("expected response to have the header hx-redirect");
        assert_eq!(
            location, "/transactions",
            "got redirect to {location:?}, want redirect to /transactions"
        );
    }
}
