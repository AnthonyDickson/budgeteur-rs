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
    /// The route to the transactions (current) page.
    transactions_page_route: Uri,
    /// The current page.
    page: u64,
    /// The maximum number of transactions to show per page.
    per_page: u64,
    /// The total number of pages.
    page_count: u64,
}

/// Controls paginations of transactions table.
#[derive(Deserialize)]
pub struct Pagination {
    /// The page number to display. Starts from 1.
    pub page: Option<u64>,
    /// The maximum number of transactions to display per page.
    pub per_page: Option<u64>,
}

/// The page number to default to when not specified in a request.
const DEFAULT_PAGE: u64 = 1;
/// The maximum transactions to display per page when not specified in a request.
const DEFAULT_PAGE_SIZE: u64 = 20;
/// The maximum number of pages to show in the pagination indicator.
const MAX_PAGES: u64 = 5;

/// Render an overview of the user's transactions.
pub async fn get_transactions_page<T>(
    State(state): State<TransactionsViewState<T>>,
    Query(query_params): Query<Pagination>,
) -> Response
where
    T: TransactionStore + Send + Sync,
{
    let nav_bar = get_nav_bar(endpoints::TRANSACTIONS_VIEW);

    let page = query_params.page.unwrap_or(DEFAULT_PAGE);
    let per_page = query_params.per_page.unwrap_or(DEFAULT_PAGE_SIZE);

    let limit = per_page;
    let offset = (page - 1) * per_page;
    let page_count = match state.transaction_store.count() {
        Ok(transaction_count) => (transaction_count as f64 / per_page as f64).ceil() as u64,
        Err(error) => return error.into_response(),
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
        transactions_page_route: Uri::from_static(endpoints::TRANSACTIONS_VIEW),
        page,
        per_page,
        page_count,
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
        routes::{
            endpoints,
            views::transactions::{MAX_PAGES, Pagination},
        },
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

        fn count(&self) -> std::result::Result<usize, Error> {
            Ok(self.transactions.len())
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
        for i in 1..=20 {
            let transaction = Transaction::build(i as f64).finalise(i as i64);
            transactions.push(transaction.clone());

            if i > (page - 1) * per_page && i <= page * per_page {
                want.push(transaction);
            }
        }
        let total_pages = (transactions.len() as f64 / per_page as f64).ceil() as u64;
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
        let pagination = must_get_pagination_indicator(&html);
        assert_has_pagination_indicators(pagination, total_pages, page, per_page);
    }

    /// If total pages <= MAX_PAGES, render links to all pages
    #[tokio::test]
    async fn pagination_indicator_shows_all_pages() {
        let mut transactions = Vec::new();
        let mut want = Vec::new();
        let transaction_count = MAX_PAGES * MAX_PAGES;
        let per_page = transaction_count / MAX_PAGES;
        let total_pages = (transaction_count as f64 / per_page as f64).ceil() as u64;
        let page = total_pages / 2;
        for i in 1..=transaction_count {
            let transaction = Transaction::build(i as f64).finalise(i as i64);
            transactions.push(transaction.clone());

            if i > (page - 1) * per_page && i <= page * per_page {
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
        let pagination = must_get_pagination_indicator(&html);
        assert_pagination_shows_all_pages(pagination, total_pages, page, per_page);
    }

    /// If total pages > MAX_PAGES and page <= MAX_PAGES, render links up to
    /// MAX_PAGES and then display a single list item with elipsis.
    #[tokio::test]
    async fn pagination_indicator_shows_page_subset_on_left() {}

    /// If total pages > MAX_PAGES and page >= (total pages - MAX_PAGES), render
    /// links up to MAX_PAGES and then display a single list item with elipsis.
    #[tokio::test]
    async fn pagination_indicator_shows_page_subset_on_right() {}

    /// If total pages > MAX_PAGES and MAX_PAGES < page < (total pages - MAX_PAGES), render
    /// links up to MAX_PAGES and then display a single list item with elipsis.
    #[tokio::test]
    async fn pagination_indicator_shows_page_subset_in_center() {}

    /// If page < total pages, display button with text 'next' that links to page + 1.
    #[tokio::test]
    async fn pagination_indicator_shows_next_button() {}

    /// If page > 1, display button with text 'back' that links to page - 1.
    #[tokio::test]
    async fn pagination_indicator_shows_back_button() {}

    /// If 1 < page < total pages, display button with text 'next' that links
    /// to page + 1 and a button with text 'back' that links to page - 1.
    #[tokio::test]
    async fn pagination_indicator_shows_next_and_back_buttons() {}

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX)
            .await
            .expect("Could not get response body");
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
    fn must_get_table(html: &Html) -> ElementRef {
        html.select(&Selector::parse("table").unwrap())
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

    #[track_caller]
    fn must_get_pagination_indicator(html: &Html) -> ElementRef {
        html.select(&Selector::parse("nav.pagination > ul.pagination").unwrap())
            .next()
            .expect("No pagination indicator found")
    }

    #[track_caller]
    fn assert_has_pagination_indicators(
        pagination_indicator: ElementRef,
        want_page_count: u64,
        want_page: u64,
        want_per_page: u64,
    ) {
        let li_selector = Selector::parse("li").unwrap();
        let list_items: Vec<ElementRef> = pagination_indicator.select(&li_selector).collect();
        assert_eq!(list_items.len(), want_page_count as usize);

        let link_selector = Selector::parse("a").unwrap();

        for (i, list_item) in (1..=want_page_count).zip(list_items) {
            let link = list_item
                .select(&link_selector)
                .next()
                .expect(&format!("Could not get link (<a> tag) for list item {i}"));
            let link_text = {
                let text = link.text().collect::<String>();
                text.trim().to_owned()
            };
            let got_page_number = link_text.parse::<u64>().expect(&format!(
                "Could not parse page number {link_text} for page {i} as usize"
            ));

            assert_eq!(i, got_page_number);

            if i == want_page {
                link.attr("aria-current").expect(&format!(
                    "The current page, page {want_page}, did not have aria-current attribute."
                ));
            } else {
                assert!(
                    link.attr("aria-current").is_none(),
                    "The current page, page {i}, should not have aria-current attribute."
                );
            }

            let link_target = link
                .attr("href")
                .expect(&format!("Link for page {i} did not have href element"));
            let want_target = format!(
                "{}?page={i}&per_page={want_per_page}",
                endpoints::TRANSACTIONS_VIEW
            );
            assert_eq!(
                want_target, link_target,
                "Got incorrect page link for page {i}"
            );
        }
    }

    #[track_caller]
    fn assert_pagination_shows_all_pages(
        pagination_indicator: ElementRef,
        want_page_count: u64,
        want_page: u64,
        want_per_page: u64,
    ) {
        let li_selector = Selector::parse("li").unwrap();
        let list_items: Vec<ElementRef> = pagination_indicator.select(&li_selector).collect();
        assert_eq!(list_items.len(), want_page_count as usize);

        let link_selector = Selector::parse("a").unwrap();

        for (i, list_item) in (1..=want_page_count).zip(list_items) {
            let link = list_item
                .select(&link_selector)
                .next()
                .expect(&format!("Could not get link (<a> tag) for list item {i}"));
            let link_text = {
                let text = link.text().collect::<String>();
                text.trim().to_owned()
            };
            let got_page_number = link_text.parse::<u64>().expect(&format!(
                "Could not parse page number {link_text} for page {i} as usize"
            ));

            assert_eq!(i, got_page_number);

            if i == want_page {
                link.attr("aria-current").expect(&format!(
                    "The current page, page {want_page}, did not have aria-current attribute."
                ));
            } else {
                assert!(
                    link.attr("aria-current").is_none(),
                    "The current page, page {i}, should not have aria-current attribute."
                );
            }

            let link_target = link
                .attr("href")
                .expect(&format!("Link for page {i} did not have href element"));
            let want_target = format!(
                "{}?page={i}&per_page={want_per_page}",
                endpoints::TRANSACTIONS_VIEW
            );
            assert_eq!(
                want_target, link_target,
                "Got incorrect page link for page {i}"
            );
        }
    }

    async fn get_response_body_text(response: Response) -> String {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        String::from_utf8_lossy(&body).to_string()
    }
}
