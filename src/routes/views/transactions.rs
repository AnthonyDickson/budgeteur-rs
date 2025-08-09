use askama_axum::Template;
use axum::{
    extract::{FromRef, Query, State},
    http::Uri,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::{
    AppState,
    pagination::{PaginationConfig, PaginationIndicator, create_pagination_indicators},
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
        templates::TransactionRow,
    },
    stores::{SortOrder, TransactionQuery, TransactionStore},
};

/// Render an overview of the user's transactions.
pub async fn get_transactions_page<T>(
    State(state): State<TransactionsViewState<T>>,
    Query(query_params): Query<Pagination>,
) -> Response
where
    T: TransactionStore + Send + Sync,
{
    let nav_bar = get_nav_bar(endpoints::TRANSACTIONS_VIEW);

    let curr_page = query_params
        .page
        .unwrap_or(state.pagination_config.default_page);
    let per_page = query_params
        .per_page
        .unwrap_or(state.pagination_config.default_page_size);

    let limit = per_page;
    let offset = (curr_page - 1) * per_page;
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

    let max_pages = state.pagination_config.max_pages;
    let pagination_indicators = create_pagination_indicators(curr_page, page_count, max_pages);

    TransactionsTemplate {
        nav_bar,
        transactions,
        create_transaction_route: Uri::from_static(endpoints::NEW_TRANSACTION_VIEW),
        import_transaction_route: Uri::from_static(endpoints::IMPORT_VIEW),
        transactions_page_route: Uri::from_static(endpoints::TRANSACTIONS_VIEW),
        pagination: &pagination_indicators,
        per_page,
    }
    .into_response()
}

/// The state needed for the transactions page.
#[derive(Debug, Clone)]
pub struct TransactionsViewState<T>
where
    T: TransactionStore + Send + Sync,
{
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
    pub pagination_config: PaginationConfig,
}

impl<T> FromRef<AppState<T>> for TransactionsViewState<T>
where
    T: TransactionStore + Clone + Send + Sync,
{
    fn from_ref(state: &AppState<T>) -> Self {
        Self {
            transaction_store: state.transaction_store.clone(),
            pagination_config: state.pagination_config.clone(),
        }
    }
}

/// Controls paginations of transactions table.
#[derive(Deserialize)]
pub struct Pagination {
    /// The page number to display. Starts from 1.
    pub page: Option<u64>,
    /// The maximum number of transactions to display per page.
    pub per_page: Option<u64>,
}

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
    pagination: &'a [PaginationIndicator],
    per_page: u64,
    // HACK: ^ Use reference for current page since (de)referencing doesn't work
    // in asakama template as expected.
}

#[cfg(test)]
mod tests {
    use askama::Result;
    use axum::{
        extract::{Query, State},
        response::Response,
    };
    use scraper::{ElementRef, Html, Selector, selectable::Selectable};

    use crate::{
        Error,
        models::{DatabaseID, Transaction, TransactionBuilder},
        pagination::PaginationConfig,
        routes::{
            endpoints,
            views::transactions::{Pagination, PaginationIndicator, TransactionsViewState},
        },
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
    async fn displays_paged_data() {
        let state = TransactionsViewState {
            transaction_store: StubTransactionStore {
                transactions: vec![
                    TransactionBuilder::new(1.0).finalise(1),
                    TransactionBuilder::new(1.0).finalise(2),
                    TransactionBuilder::new(1.0).finalise(3),
                    TransactionBuilder::new(1.0).finalise(4),
                    TransactionBuilder::new(1.0).finalise(5),
                    TransactionBuilder::new(1.0).finalise(6),
                    TransactionBuilder::new(1.0).finalise(7),
                    TransactionBuilder::new(1.0).finalise(8),
                    TransactionBuilder::new(1.0).finalise(9),
                    TransactionBuilder::new(1.0).finalise(10),
                    TransactionBuilder::new(1.0).finalise(11),
                    TransactionBuilder::new(1.0).finalise(12),
                    TransactionBuilder::new(1.0).finalise(13),
                    TransactionBuilder::new(1.0).finalise(14),
                    TransactionBuilder::new(1.0).finalise(15),
                    TransactionBuilder::new(1.0).finalise(16),
                    TransactionBuilder::new(1.0).finalise(17),
                    TransactionBuilder::new(1.0).finalise(18),
                    TransactionBuilder::new(1.0).finalise(19),
                    TransactionBuilder::new(1.0).finalise(20),
                    TransactionBuilder::new(1.0).finalise(21),
                    TransactionBuilder::new(1.0).finalise(22),
                    TransactionBuilder::new(1.0).finalise(23),
                    TransactionBuilder::new(1.0).finalise(24),
                    TransactionBuilder::new(1.0).finalise(25),
                    TransactionBuilder::new(1.0).finalise(26),
                    TransactionBuilder::new(1.0).finalise(27),
                    TransactionBuilder::new(1.0).finalise(28),
                    TransactionBuilder::new(1.0).finalise(29),
                    TransactionBuilder::new(1.0).finalise(30),
                ],
            },
            pagination_config: PaginationConfig {
                max_pages: 5,
                ..Default::default()
            },
        };
        let per_page = 3;
        let page = 5;
        let want_transactions = [
            TransactionBuilder::new(1.0).finalise(13),
            TransactionBuilder::new(1.0).finalise(14),
            TransactionBuilder::new(1.0).finalise(15),
        ];
        let want_indicators = [
            PaginationIndicator::BackButton(4),
            PaginationIndicator::Page(1),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(3),
            PaginationIndicator::Page(4),
            PaginationIndicator::CurrPage(5),
            PaginationIndicator::Page(6),
            PaginationIndicator::Page(7),
            PaginationIndicator::Ellipsis,
            PaginationIndicator::Page(10),
            PaginationIndicator::NextButton(6),
        ];

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
        assert_table_has_transactions(table, &want_transactions);
        let pagination = must_get_pagination_indicator(&html);
        assert_correct_pagination_indicators(pagination, per_page, &want_indicators);
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

        assert_eq!(
            table_rows.len(),
            transactions.len(),
            "want table with {} rows, got {}",
            transactions.len(),
            table_rows.len()
        );

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

            assert_eq!(
                got_id,
                want.id(),
                "Want transaction with ID {}, got {got_id}",
                want.id()
            );
        }
    }

    #[track_caller]
    fn must_get_pagination_indicator(html: &Html) -> ElementRef {
        html.select(&Selector::parse("nav.pagination > ul.pagination").unwrap())
            .next()
            .expect("No pagination indicator found")
    }

    #[track_caller]
    fn assert_correct_pagination_indicators(
        pagination_indicator: ElementRef,
        want_per_page: u64,
        want_indicators: &[PaginationIndicator],
    ) {
        let li_selector = Selector::parse("li").unwrap();
        let list_items: Vec<ElementRef> = pagination_indicator.select(&li_selector).collect();
        let list_len = list_items.len();
        let want_len = want_indicators.len();
        assert_eq!(list_len, want_len, "got {list_len} pages, want {want_len}");

        let link_selector = Selector::parse("a").unwrap();

        for (i, (list_item, want_indicator)) in list_items.iter().zip(want_indicators).enumerate() {
            match *want_indicator {
                PaginationIndicator::CurrPage(want_page) => {
                    assert!(
                        list_item.select(&link_selector).next().is_none(),
                        "The current page indicator should not contain a link"
                    );

                    let paragraph_selector =
                        Selector::parse("p").expect("Could not create selector 'p'");
                    let paragraph = list_item
                        .select(&paragraph_selector)
                        .next()
                        .expect("Current page indicator should have a paragraph element ('<p>')");

                    assert_eq!(paragraph.attr("aria-current"), Some("page"));

                    let text = {
                        let text = paragraph.text().collect::<String>();
                        text.trim().to_owned()
                    };

                    let got_page_number: u64 = text.parse().expect(&format!(
                        "Could not parse \"{text}\" as a u64 for list item {i} in {}",
                        list_item.html()
                    ));

                    assert_eq!(
                        want_page,
                        got_page_number,
                        "want page number {want_page}, got {got_page_number} for list item {i} in {}",
                        pagination_indicator.html()
                    );
                }
                PaginationIndicator::Page(want_page) => {
                    let link = list_item
                        .select(&link_selector)
                        .next()
                        .expect(&format!("Could not get link (<a> tag) for list item {i}"));
                    let link_text = {
                        let text = link.text().collect::<String>();
                        text.trim().to_owned()
                    };
                    let got_page_number = link_text.parse::<u64>().expect(&format!(
                        "Could not parse page number {link_text} for page {want_page} as usize"
                    ));

                    assert_eq!(
                        want_page,
                        got_page_number,
                        "want page number {want_page}, got {got_page_number} for list item {i} in {}",
                        pagination_indicator.html()
                    );

                    let link_target = link.attr("href").expect(&format!(
                        "Link for page {want_page} did not have href element"
                    ));
                    let want_target = format!(
                        "{}?page={want_page}&per_page={want_per_page}",
                        endpoints::TRANSACTIONS_VIEW
                    );
                    assert_eq!(
                        want_target, link_target,
                        "Got incorrect page link for page {want_page}"
                    );
                }
                PaginationIndicator::Ellipsis => {
                    assert!(
                        list_item.select(&link_selector).next().is_none(),
                        "Item {i} should not contain a link tag (<a>) in {}",
                        pagination_indicator.html()
                    );
                    let got_text = list_item.text().collect::<String>();
                    let got_text = got_text.trim();
                    assert_eq!(got_text, "...");
                }
                PaginationIndicator::NextButton(want_page) => {
                    let link = list_item
                        .select(&link_selector)
                        .next()
                        .expect(&format!("Could not get link (<a> tag) for list item {i}"));
                    let link_text = {
                        let text = link.text().collect::<String>();
                        text.trim().to_owned()
                    };
                    assert_eq!(
                        "Next", link_text,
                        "want link text \"Next\", got \"{link_text}\""
                    );

                    let role = link
                        .attr("role")
                        .expect(&format!("The next button did not have \"role\" attribute."));
                    assert_eq!(
                        role, "button",
                        "The next page anchor tag should be marked as a button."
                    );

                    let link_target = link
                        .attr("href")
                        .expect(&format!("Link for next button did not have href element"));
                    let want_target = format!(
                        "{}?page={want_page}&per_page={want_per_page}",
                        endpoints::TRANSACTIONS_VIEW
                    );
                    assert_eq!(
                        want_target, link_target,
                        "Got link to {link_target} for next button, want {want_page}"
                    );
                }
                PaginationIndicator::BackButton(want_page) => {
                    let link = list_item
                        .select(&link_selector)
                        .next()
                        .expect(&format!("Could not get link (<a> tag) for list item {i}"));
                    let link_text = {
                        let text = link.text().collect::<String>();
                        text.trim().to_owned()
                    };
                    assert_eq!(
                        "Back", link_text,
                        "want link text \"Back\", got \"{link_text}\""
                    );

                    let role = link
                        .attr("role")
                        .expect(&format!("The back button did not have \"role\" attribute."));
                    assert_eq!(
                        role, "button",
                        "The back button's anchor tag should be marked as a button."
                    );

                    let link_target = link
                        .attr("href")
                        .expect(&format!("Link for back button did not have href element"));
                    let want_target = format!(
                        "{}?page={want_page}&per_page={want_per_page}",
                        endpoints::TRANSACTIONS_VIEW
                    );
                    assert_eq!(
                        want_target, link_target,
                        "Got link to {link_target} for back button, want {want_page}"
                    );
                }
            }
        }
    }
}
