use askama_axum::Template;
use axum::{
    extract::State,
    http::Uri,
    response::{IntoResponse, Response},
};

use crate::{
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
        templates::TransactionRow,
    },
    state::TransactionsViewState,
    stores::{SortOrder, TransactionQuery, TransactionStore},
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/transactions.html")]
struct TransactionsTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// The user's transactions for this week, as Askama templates.
    transactions: Vec<TransactionRow>,
    /// The route for creating a new transaction for the current user.
    create_transaction_route: Uri,
    /// The route for importing transactions from CSV files.
    import_transaction_route: Uri,
}

// TODO: implement pagination
/// Render an overview of the user's transactions.
pub async fn get_transactions_page<T>(State(state): State<TransactionsViewState<T>>) -> Response
where
    T: TransactionStore + Send + Sync,
{
    let nav_bar = get_nav_bar(endpoints::TRANSACTIONS_VIEW);

    let transactions = state.transaction_store.get_query(TransactionQuery {
        limit: Some(20),
        sort_date: Some(SortOrder::Descending),
        ..Default::default()
    });
    let transactions = match transactions {
        Ok(transactions) => transactions,
        Err(error) => return error.into_response(),
    };

    let transactions = transactions
        .into_iter()
        .map(|transaction| TransactionRow { transaction })
        .collect();

    TransactionsTemplate {
        nav_bar,
        transactions,
        create_transaction_route: Uri::from_static(endpoints::NEW_TRANSACTION_VIEW),
        import_transaction_route: Uri::from_static(endpoints::IMPORT_VIEW),
    }
    .into_response()
}

#[cfg(test)]
mod transactions_route_tests {
    use askama::Result;
    use axum::{extract::State, http::StatusCode, response::Response};
    use scraper::Html;

    use crate::{
        Error,
        models::{DatabaseID, Transaction, TransactionBuilder},
        state::TransactionsViewState,
        stores::{TransactionQuery, TransactionStore},
    };

    use super::get_transactions_page;

    #[derive(Debug, Clone)]
    struct StubTransactionStore {
        transactions: Vec<Transaction>,
    }

    impl TransactionStore for StubTransactionStore {
        fn create(&mut self, _amount: f64) -> Result<Transaction, Error> {
            todo!()
        }

        fn create_from_builder(
            &mut self,
            _builder: TransactionBuilder,
        ) -> Result<Transaction, Error> {
            todo!()
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

        fn get_query(&self, _query: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            Ok(self.transactions.clone())
        }
    }

    #[tokio::test]
    async fn transactions_page_displays_correct_info() {
        let transactions = vec![
            Transaction::build(1.0).description("foo").finalise(1),
            Transaction::build(2.0).description("bar").finalise(2),
        ];
        let transaction_store = StubTransactionStore {
            transactions: transactions.clone(),
        };
        let state = TransactionsViewState { transaction_store };

        let response = get_transactions_page(State(state)).await;

        assert_eq!(response.status(), StatusCode::OK);

        let transactions_page_text = get_response_body_text(response).await;
        let html = Html::parse_document(&transactions_page_text);
        assert_valid_html(&html);

        for transaction in transactions {
            assert!(
                transactions_page_text.contains(&transaction.date().to_string()),
                "Could not find date {} in text \"{}\"",
                transaction.date(),
                transactions_page_text
            );
            assert!(
                transactions_page_text.contains(transaction.description()),
                "Could not find description \"{}\" in text \"{}\"",
                transaction.description(),
                transactions_page_text
            );
        }
    }

    async fn get_response_body_text(response: Response) -> String {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        String::from_utf8_lossy(&body).to_string()
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }
}
