//! This file defines the dashboard route and its handlers.

use askama_axum::Template;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use time::{Duration, OffsetDateTime};

use crate::{
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
    },
    state::DashboardState,
    stores::{TransactionQuery, TransactionStore},
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/dashboard.html")]
struct DashboardTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// How much over or under budget the user is for this week.
    balance: f64,
}

/// Display a page with an overview of the user's data.
pub async fn get_dashboard_page<T>(State(state): State<DashboardState<T>>) -> Response
where
    T: TransactionStore + Send + Sync,
{
    let nav_bar = get_nav_bar(endpoints::DASHBOARD_VIEW);

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

    let transactions = state.transaction_store.get_query(TransactionQuery {
        date_range: Some(one_week_ago..=today),
        ..Default::default()
    });

    let balance = match transactions {
        Ok(transactions) => transactions
            .iter()
            .map(|transaction| transaction.amount())
            .sum(),
        Err(error) => return error.into_response(),
    };

    DashboardTemplate { nav_bar, balance }.into_response()
}

#[cfg(test)]
mod dashboard_route_tests {
    use axum::{
        body::Body,
        extract::State,
        http::{Response, StatusCode},
    };
    use time::{Duration, OffsetDateTime};

    use crate::{
        Error,
        models::{DatabaseID, Transaction, TransactionBuilder},
        state::DashboardState,
        stores::{TransactionQuery, TransactionStore},
    };

    use super::get_dashboard_page;

    #[derive(Clone)]
    struct FakeTransactionStore {
        transactions: Vec<Transaction>,
    }

    impl TransactionStore for FakeTransactionStore {
        fn create(&mut self, amount: f64) -> Result<Transaction, Error> {
            self.create_from_builder(TransactionBuilder::new(amount))
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

            Ok(transaction)
        }

        fn import(
            &mut self,
            _builders: Vec<TransactionBuilder>,
        ) -> Result<Vec<Transaction>, Error> {
            todo!()
        }

        fn get(&self, _id: DatabaseID) -> Result<Transaction, Error> {
            todo!()
        }

        fn get_query(&self, filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            self.transactions
                .iter()
                .filter(|transaction| {
                    let mut should_keep = true;

                    if let Some(ref date_range) = filter.date_range {
                        should_keep &= date_range.start() <= transaction.date()
                            && transaction.date() <= date_range.end();
                    }

                    should_keep
                })
                .map(|transaction| Ok(transaction.to_owned()))
                .collect()
        }

        fn count(&self) -> Result<usize, Error> {
            todo!()
        }
    }

    #[tokio::test]
    async fn dashboard_displays_correct_balance() {
        let transactions = vec![
            // Transactions before the current week should not be included in the balance.
            Transaction::build(12.3)
                .date(
                    OffsetDateTime::now_utc()
                        .date()
                        .checked_sub(Duration::weeks(2))
                        .unwrap(),
                )
                .unwrap()
                .finalise(1),
            // These transactions should be included.
            Transaction::build(45.6).finalise(2),
            Transaction::build(-45.6).finalise(3),
            Transaction::build(123.0).finalise(4),
        ];
        let state = DashboardState {
            transaction_store: FakeTransactionStore { transactions },
        };

        let response = get_dashboard_page(State(state)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_amount(response, "$123").await;
    }

    #[tokio::test]
    async fn dashboard_displays_negative_balance_without_sign() {
        let transactions = vec![Transaction::build(-123.0).finalise(2)];
        let state = DashboardState {
            transaction_store: FakeTransactionStore { transactions },
        };

        let response = get_dashboard_page(State(state)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_amount(response, "$123").await;
    }

    async fn assert_body_contains_amount(response: Response<Body>, want: &str) {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        let text = String::from_utf8_lossy(&body).to_string();

        assert!(
            text.contains(want),
            "response body should contain '{}' but got {}",
            want,
            text
        );
    }
}
