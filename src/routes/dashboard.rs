//! This file defines the dashboard route and its handlers.

use super::{
    endpoints::{self},
    navigation::{get_nav_bar, NavbarTemplate},
};
use askama_axum::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    Extension,
};
use time::{Duration, OffsetDateTime};

use crate::{
    models::UserID,
    stores::{transaction::TransactionQuery, CategoryStore, TransactionStore, UserStore},
    AppError, AppState,
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate<'a> {
    navbar: NavbarTemplate<'a>,
    user_id: UserID,
    /// How much over or under budget the user is for this week.
    balance: f64,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page<C, T, U>(
    State(mut state): State<AppState<C, T, U>>,
    Extension(user_id): Extension<UserID>,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let navbar = get_nav_bar(endpoints::DASHBOARD);

    let today = OffsetDateTime::now_utc().date();
    let one_week_ago = match today.checked_sub(Duration::weeks(1)) {
        Some(date) => date,
        None => {
            tracing::warn!(
                "Could not get date for one week before {today}. Using today's date ({today}) instead."
            );

            today
        }
    };

    let transactions = state.transaction_store().get_query(TransactionQuery {
        user_id: Some(user_id),
        date_range: Some(one_week_ago..=today),
        ..Default::default()
    });

    let balance = match transactions {
        Ok(transactions) => transactions
            .iter()
            .map(|transaction| transaction.amount())
            .sum(),
        Err(error) => return AppError::TransactionError(error).into_response(),
    };

    DashboardTemplate {
        navbar,
        user_id,
        balance,
    }
    .into_response()
}

#[cfg(test)]
mod dashboard_route_tests {
    use axum::{
        body::Body,
        extract::State,
        http::{Response, StatusCode},
        Extension,
    };
    use time::{Duration, OffsetDateTime};

    use crate::{
        models::{
            Category, CategoryError, CategoryName, DatabaseID, PasswordHash, Transaction,
            TransactionBuilder, TransactionError, User, UserID,
        },
        stores::{
            transaction::TransactionQuery, CategoryStore, TransactionStore, UserError, UserStore,
        },
        AppState,
    };

    use super::get_dashboard_page;

    #[derive(Clone)]
    struct DummyUserStore {}

    impl UserStore for DummyUserStore {
        fn create(
            &mut self,
            _email: email_address::EmailAddress,
            _password_hash: PasswordHash,
        ) -> Result<User, UserError> {
            todo!()
        }

        fn get(&self, _id: UserID) -> Result<User, UserError> {
            todo!()
        }

        fn get_by_email(&self, _email: &email_address::EmailAddress) -> Result<User, UserError> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyCategoryStore {}

    impl CategoryStore for DummyCategoryStore {
        fn create(&self, _name: CategoryName, _user_id: UserID) -> Result<Category, CategoryError> {
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

            Ok(transaction)
        }

        fn get(&self, _id: DatabaseID) -> Result<Transaction, TransactionError> {
            todo!()
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, TransactionError> {
            todo!()
        }

        fn get_query(
            &self,
            filter: TransactionQuery,
        ) -> Result<Vec<Transaction>, TransactionError> {
            self.transactions
                .iter()
                .filter(|transaction| {
                    let mut should_keep = true;

                    if let Some(user_id) = filter.user_id {
                        should_keep &= transaction.user_id() == user_id;
                    }

                    if let Some(ref date_range) = filter.date_range {
                        should_keep &= date_range.start() <= transaction.date()
                            && transaction.date() <= date_range.end();
                    }

                    should_keep
                })
                .map(|transaction| Ok(transaction.to_owned()))
                .collect()
        }
    }

    #[tokio::test]
    async fn dashboard_displays_correct_balance() {
        let user_id = UserID::new(321);
        let transactions = vec![
            // Transactions before the current week should not be included in the balance.
            Transaction::build(12.3, user_id)
                .date(
                    OffsetDateTime::now_utc()
                        .date()
                        .checked_sub(Duration::weeks(2))
                        .unwrap(),
                )
                .unwrap()
                .finalise(1),
            // These transactions should be included.
            Transaction::build(45.6, user_id).finalise(2),
            Transaction::build(-45.6, user_id).finalise(3),
            Transaction::build(123.0, user_id).finalise(4),
            // Transactions from other users should not be included either.
            Transaction::build(999.99, UserID::new(999)).finalise(5),
        ];
        let state = AppState::new(
            "123",
            DummyCategoryStore {},
            FakeTransactionStore { transactions },
            DummyUserStore {},
        );

        let response = get_dashboard_page(State(state), Extension(user_id)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_amount(response, 123.0).await;
    }

    async fn assert_body_contains_amount(response: Response<Body>, want: f64) {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        let text = String::from_utf8_lossy(&body).to_string();

        assert!(
            text.contains(&want.to_string()),
            "response body should contain '{}' but got {}",
            want,
            text
        );
    }
}
