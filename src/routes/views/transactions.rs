use askama_axum::Template;
use axum::{
    extract::{Query, State},
    http::Uri,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

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

/// Controls paginations of transactions table.
#[derive(Deserialize)]
pub struct Pagination {
    /// The page number to display. Starts from 1.
    pub page: Option<u32>,
    /// The maximum number of transactions to display per page.
    pub per_page: Option<u32>,
}

/// Render an overview of the user's transactions.
pub async fn get_transactions_page<T>(
    State(state): State<TransactionsViewState<T>>,
    Query(query_params): Query<Pagination>,
) -> Response
where
    T: TransactionStore + Send + Sync,
{
    let nav_bar = get_nav_bar(endpoints::TRANSACTIONS_VIEW);

    let Pagination { page, per_page } = query_params;

    let (limit, offset) = match (page, per_page) {
        (Some(page), Some(per_page)) => (per_page, (page - 1) * per_page),
        _ => (20, 0),
    };

    let transactions = state.transaction_store.get_query(TransactionQuery {
        limit: Some(limit),
        offset,
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
    use axum::{
        extract::{Query, State},
        http::StatusCode,
        response::Response,
    };
    use scraper::{ElementRef, Html, Selector, selectable::Selectable};

    use crate::{
        Error,
        models::{DatabaseID, Transaction, TransactionBuilder},
        routes::views::transactions::Pagination,
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

        fn get_query(&self, query: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            if let Some(limit) = query.limit {
                let offset = query.offset as usize;
                let limit = limit as usize;

                if offset > self.transactions.len() || offset + limit > self.transactions.len() {
                    Ok(self.transactions.clone())
                } else {
                    Ok(self.transactions[offset..offset + limit].to_owned())
                }
            } else {
                Ok(self.transactions.clone())
            }
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

        let response = get_transactions_page(
            State(state),
            Query(Pagination {
                page: None,
                per_page: None,
            }),
        )
        .await;

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

    #[tokio::test]
    async fn displays_paged_data() {
        let mut transactions = Vec::new();
        let mut want = Vec::new();
        let page = 3;
        let per_page = 2;
        for i in 0..20_u32 {
            let transaction = Transaction::build(i as f64).finalise(i as i64);
            transactions.push(transaction.clone());

            if i >= (page - 1) * per_page && i < page * per_page {
                want.push(transaction);
            }
        }
        let state = TransactionsViewState {
            transaction_store: StubTransactionStore {
                transactions: transactions.clone(),
            },
        };

        let response = get_transactions_page(
            State(state),
            Query(Pagination {
                page: Some(page),
                per_page: Some(per_page),
            }),
        )
        .await;

        let html = parse_html(response).await;
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_has_transactions(table, &want);
        // TODO: check for pagination indicator
        // TODO: check that pagination indicator displays up ten pages
        // TODO: check that pagination indicator displays current page in
        //  numerical order if current page < max pages
        // TODO: check that pagination indicator displays current page in
        //  numerical order if current page > (page count - current page)
        // TODO: check that pagination indicator displays current page in
        //  middle if current page > max pages and current page <= (page count - max pages)
        // TODO: check that previous page link is rendered if current page > 0
        // TODO: check that next page link is rendered if current page < page count
    }

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX)
            .await
            .expect("Could not get response body");
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_document(&text)
    }

    #[track_caller]
    fn must_get_table(html: &Html) -> ElementRef {
        html.select(&scraper::Selector::parse("table").unwrap())
            .next()
            .expect("No table found")
    }

    #[track_caller]
    fn assert_table_has_transactions(table: ElementRef, transactions: &[Transaction]) {
        let row_selector = Selector::parse("tbody tr").unwrap();
        let table_rows: Vec<ElementRef<'_>> = table.select(&row_selector).collect();

        assert_eq!(table_rows.len(), transactions.len());

        let th_selector = Selector::parse("th").unwrap();
        for (i, (row, want)) in table_rows.iter().zip(transactions).enumerate() {
            let th = row
                .select(&th_selector)
                .next()
                .expect(&format!("Could not find th element in table row {i}"));

            let id_str = th.text().collect::<String>();
            let got_id: i64 = id_str.trim().parse().expect(&format!(
                "Could not parse ID {id_str} on table row {i} as integer"
            ));

            assert_eq!(got_id, want.id());
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
