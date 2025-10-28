//! Defines the route handler for the page that displays transactions as a table.
use std::sync::{Arc, Mutex};

use askama::Template;
use axum::{
    extract::{FromRef, Query, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use rusqlite::Connection;
use serde::Deserialize;
use time::Date;

use crate::{
    AppState, Error,
    database_id::TransactionId,
    endpoints, filters,
    navigation::{NavbarTemplate, get_nav_bar},
    pagination::{PaginationConfig, PaginationIndicator, create_pagination_indicators},
    shared_templates::render,
    tag::TagName,
    transaction::core::count_transactions,
};

/// The state needed for the transactions page.
#[derive(Debug, Clone)]
pub struct TransactionsViewState {
    /// The database connection for managing transactions.
    db_connection: Arc<Mutex<Connection>>,
    /// Configuration for pagination controls.
    pagination_config: PaginationConfig,
}

impl FromRef<AppState> for TransactionsViewState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
            pagination_config: state.pagination_config.clone(),
        }
    }
}

/// Controls paginations of transactions table.
#[derive(Deserialize)]
pub struct Pagination {
    /// The page number to display. Starts from 1.
    page: Option<u64>,
    /// The maximum number of transactions to display per page.
    per_page: Option<u64>,
}

/// Renders the transaction page.
#[derive(Template)]
#[template(path = "views/transaction/table.html")]
struct TransactionsTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    /// The user's transactions for this week, as Askama templates.
    transactions: Vec<TransactionTableRow>,
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

/// Render an overview of the user's transactions.
pub async fn get_transactions_page(
    State(state): State<TransactionsViewState>,
    Query(query_params): Query<Pagination>,
) -> Response {
    let nav_bar = get_nav_bar(endpoints::TRANSACTIONS_VIEW);

    let current_page = query_params
        .page
        .unwrap_or(state.pagination_config.default_page);
    let per_page = query_params
        .per_page
        .unwrap_or(state.pagination_config.default_page_size);

    let limit = per_page;
    let offset = (current_page - 1) * per_page;
    let connection = state.db_connection.lock().unwrap();
    let page_count = match count_transactions(&connection) {
        Ok(transaction_count) => (transaction_count as f64 / per_page as f64).ceil() as u64,
        Err(error) => return error.into_response(),
    };

    let redirect_url = get_redirect_url(current_page, per_page);

    let transactions =
        get_transaction_table_rows_paginated(limit, offset, SortOrder::Descending, &connection);
    let transactions = match transactions {
        Ok(transactions) => transactions
            .into_iter()
            .map(|transaction| {
                TransactionTableRow::new_from_transaction(transaction, redirect_url.as_deref())
            })
            .collect(),
        Err(error) => return error.into_response(),
    };

    let max_pages = state.pagination_config.max_pages;
    let pagination_indicators = create_pagination_indicators(current_page, page_count, max_pages);

    render(
        StatusCode::OK,
        TransactionsTemplate {
            nav_bar,
            transactions,
            create_transaction_route: Uri::from_static(endpoints::NEW_TRANSACTION_VIEW),
            import_transaction_route: Uri::from_static(endpoints::IMPORT_VIEW),
            transactions_page_route: Uri::from_static(endpoints::TRANSACTIONS_VIEW),
            pagination: &pagination_indicators,
            per_page,
        },
    )
}

fn get_redirect_url(page: u64, per_page: u64) -> Option<String> {
    let redirect_url = format!(
        "{}?page={page}&per_page={per_page}",
        endpoints::TRANSACTIONS_VIEW
    );

    serde_urlencoded::to_string([("redirect_url", &redirect_url)])
        .inspect_err(|error| {
            tracing::error!(
                "Could not set redirect URL {redirect_url} due to encoding error: {error}"
            );
        })
        .ok()
}

#[derive(Debug, PartialEq)]
struct Transaction {
    /// The ID of the transaction.
    id: TransactionId,
    /// The amount of money spent or earned in this transaction.
    amount: f64,
    /// When the transaction happened.
    date: Date,
    /// A text description of what the transaction was for.
    description: String,
    /// The name of the transactions tag.
    tag_name: Option<TagName>,
}

/// Renders a transaction with its tags as a table row.
#[derive(Debug, Template, PartialEq)]
#[template(path = "partials/transaction_table_row.html")]
struct TransactionTableRow {
    /// The amount of money spent or earned in this transaction.
    amount: f64,
    /// When the transaction happened.
    date: Date,
    /// A text description of what the transaction was for.
    description: String,
    /// The name of the transactions tag.
    tag_name: Option<TagName>,
    /// The API path to edit this transaction
    edit_url: String,
    /// The API path to delete this transaction
    delete_url: String,
}

impl TransactionTableRow {
    fn new_from_transaction(transaction: Transaction, redirect_url: Option<&str>) -> Self {
        let mut edit_url =
            endpoints::format_endpoint(endpoints::EDIT_TRANSACTION_VIEW, transaction.id);

        if let Some(redirect_url) = redirect_url {
            edit_url = format!("{edit_url}?{redirect_url}");
        }

        Self {
            amount: transaction.amount,
            date: transaction.date,
            description: transaction.description,
            tag_name: transaction.tag_name,
            edit_url,
            delete_url: endpoints::format_endpoint(endpoints::DELETE_TRANSACTION, transaction.id),
        }
    }
}

/// The order to sort transactions in a [TransactionQuery].
enum SortOrder {
    /// Sort in order of increasing value.
    // TODO: Remove #[allow(dead_code)] once Ascending is used
    #[allow(dead_code)]
    Ascending,
    /// Sort in order of decreasing value.
    Descending,
}

/// Get transactions with pagination and sorting by date.
///
/// # Arguments
/// * `limit` - Maximum number of transactions to return
/// * `offset` - Number of transactions to skip
/// * `sort_order` - Sort direction for date field
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
/// - Transaction row mapping fails
fn get_transaction_table_rows_paginated(
    limit: u64,
    offset: u64,
    sort_order: SortOrder,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
    let order_clause = match sort_order {
        SortOrder::Ascending => "ORDER BY date ASC",
        SortOrder::Descending => "ORDER BY date DESC",
    };

    // Sort by date, and then ID to keep transaction order stable after updates
    let query = format!(
        "SELECT \"transaction\".id, amount, date, description, tag.name FROM \"transaction\" \
        LEFT JOIN tag ON \"transaction\".tag_id = tag.id \
        {}, \"transaction\".id ASC \
        LIMIT {} OFFSET {}",
        order_clause, limit, offset
    );

    connection
        .prepare(&query)?
        .query_map([], |row| {
            let tag_name = row
                .get::<usize, Option<String>>(4)?
                .map(|some_tag_name| TagName::new_unchecked(&some_tag_name));

            Ok(Transaction {
                id: row.get(0)?,
                amount: row.get(1)?,
                date: row.get(2)?,
                description: row.get(3)?,
                tag_name,
            })
        })?
        .map(|transaction_result| transaction_result.map_err(Error::SqlError))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::{Query, State},
        response::Response,
    };
    use rusqlite::Connection;
    use scraper::{ElementRef, Html, Selector, selectable::Selectable};
    use time::macros::date;

    use crate::{
        db::initialize,
        endpoints,
        pagination::{PaginationConfig, PaginationIndicator},
        tag::{TagName, create_tag},
        transaction::{Transaction, create_transaction},
    };

    use super::{Pagination, TransactionsViewState, get_transactions_page};

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }

    #[tokio::test]
    async fn transactions_page_displays_paged_data() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);

        // Create 30 transactions in the database
        for i in 1..=30 {
            create_transaction(Transaction::build(i as f64, today, ""), &conn).unwrap();
        }

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            pagination_config: PaginationConfig {
                max_pages: 5,
                ..Default::default()
            },
        };
        let per_page = 3;
        let page = 5;
        let want_transactions = [
            Transaction {
                id: 13,
                amount: 13.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
                tag_id: None,
            },
            Transaction {
                id: 14,
                amount: 14.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
                tag_id: None,
            },
            Transaction {
                id: 15,
                amount: 15.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
                tag_id: None,
            },
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

    #[track_caller]
    fn must_get_table(html: &Html) -> ElementRef<'_> {
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

        let td_selector = Selector::parse("td").unwrap();
        for (i, (row, want)) in table_rows.iter().zip(transactions).enumerate() {
            let td = row
                .select(&td_selector)
                .next()
                .unwrap_or_else(|| panic!("Could not find th element in table row {i}"));

            let amount_str = td.text().collect::<String>();
            let got_amount: f64 = amount_str
                .trim()
                .strip_prefix("$")
                .unwrap()
                .parse()
                .unwrap_or_else(|_| {
                    panic!("Could not parse amount {amount_str} on table row {i} as integer")
                });

            assert_eq!(
                got_amount, want.amount,
                "Want transaction with amount (ID) {}, got {got_amount}",
                want.amount
            );
        }
    }

    #[track_caller]
    fn must_get_pagination_indicator(html: &Html) -> ElementRef<'_> {
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

                    let got_page_number: u64 = text.parse().unwrap_or_else(|_| {
                        panic!(
                            "Could not parse \"{text}\" as a u64 for list item {i} in {}",
                            list_item.html()
                        )
                    });

                    assert_eq!(
                        want_page,
                        got_page_number,
                        "want page number {want_page}, got {got_page_number} for list item {i} in {}",
                        pagination_indicator.html()
                    );
                }
                PaginationIndicator::Page(want_page) => {
                    let link = list_item.select(&link_selector).next().unwrap_or_else(|| {
                        panic!("Could not get link (<a> tag) for list item {i}")
                    });
                    let link_text = {
                        let text = link.text().collect::<String>();
                        text.trim().to_owned()
                    };
                    let got_page_number = link_text.parse::<u64>().unwrap_or_else(|_| {
                        panic!(
                            "Could not parse page number {link_text} for page {want_page} as usize"
                        )
                    });

                    assert_eq!(
                        want_page,
                        got_page_number,
                        "want page number {want_page}, got {got_page_number} for list item {i} in {}",
                        pagination_indicator.html()
                    );

                    let link_target = link.attr("href").unwrap_or_else(|| {
                        panic!("Link for page {want_page} did not have href element")
                    });
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
                    let link = list_item.select(&link_selector).next().unwrap_or_else(|| {
                        panic!("Could not get link (<a> tag) for list item {i}")
                    });
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
                        .expect("The next button did not have \"role\" attribute.");
                    assert_eq!(
                        role, "button",
                        "The next page anchor tag should be marked as a button."
                    );

                    let link_target = link
                        .attr("href")
                        .expect("Link for next button did not have href element");
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
                    let link = list_item.select(&link_selector).next().unwrap_or_else(|| {
                        panic!("Could not get link (<a> tag) for list item {i}")
                    });
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
                        .expect("The back button did not have \"role\" attribute.");
                    assert_eq!(
                        role, "button",
                        "The back button's anchor tag should be marked as a button."
                    );

                    let link_target = link
                        .attr("href")
                        .expect("Link for back button did not have href element");
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

    #[tokio::test]
    async fn transactions_page_displays_tags_column() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);

        // Create test tags
        let tag1 = create_tag(TagName::new_unchecked("Groceries"), &conn).unwrap();
        let tag2 = create_tag(TagName::new_unchecked("Food"), &conn).unwrap();

        // Create transactions
        create_transaction(
            Transaction::build(50.0, today, "Store purchase").tag_id(Some(tag2.id)),
            &conn,
        )
        .unwrap();
        create_transaction(
            Transaction::build(25.0, today, "Restaurant").tag_id(Some(tag1.id)),
            &conn,
        )
        .unwrap();
        create_transaction(
            Transaction::build(100.0, today, "No tags transaction"),
            &conn,
        )
        .unwrap();

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            pagination_config: PaginationConfig::default(),
        };

        let response = get_transactions_page(
            State(state),
            Query(Pagination {
                page: Some(1),
                per_page: Some(10),
            }),
        )
        .await;

        let html = parse_html(response).await;
        assert_valid_html(&html);

        // Check that Tags column header exists
        let headers = html
            .select(&Selector::parse("thead th").unwrap())
            .collect::<Vec<_>>();
        let header_texts: Vec<String> = headers
            .iter()
            .map(|h| h.text().collect::<String>().trim().to_string())
            .collect();
        assert!(
            header_texts.contains(&"Tags".to_string()),
            "Tags column header should exist. Found headers: {:?}",
            header_texts
        );

        // Check table rows for tag content
        let table_rows = html
            .select(&Selector::parse("tbody tr").unwrap())
            .collect::<Vec<_>>();
        assert_eq!(table_rows.len(), 3, "Should have 3 transaction rows");

        // Check that each row has 5 columns (Amount, Date, Description, Tags, Actions)
        for (i, row) in table_rows.iter().enumerate() {
            let cells = row
                .select(&Selector::parse("th, td").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                cells.len(),
                5,
                "Row {} should have 5 columns (Amount, Date, Description, Tags, Actions)",
                i
            );

            // The second to last cell should be the Tags column
            let tags_cell = &cells[3];
            let tags_cell_html = tags_cell.html();

            // Check if this row should have tags or not
            if tags_cell_html.contains("-") && !tags_cell_html.contains("bg-blue-100") {
                // This is the "no tags" case showing "-"
                assert!(
                    tags_cell_html.contains("text-gray-400"),
                    "Empty tags should be displayed with gray text"
                );
            } else {
                // Should contain tag badges
                assert!(
                    tags_cell_html.contains("bg-blue-100"),
                    "Tag should have blue background styling"
                );
            }
        }
    }

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX)
            .await
            .expect("Could not get response body");
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_document(&text)
    }
}

#[cfg(test)]
mod database_tests {
    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime, macros::date};

    use crate::{
        database_id::TransactionId,
        db::initialize,
        transaction::{
            Transaction, create_transaction,
            transactions_page::{
                SortOrder, Transaction as TableTransaction, get_transaction_table_rows_paginated,
            },
        },
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn get_transactions_with_limit() {
        let conn = get_test_connection();

        let today = OffsetDateTime::now_utc().date();

        for i in 1..=10 {
            let transaction_builder = Transaction::build(
                i as f64,
                today - Duration::days(i),
                &format!("transaction #{i}"),
            );

            create_transaction(transaction_builder, &conn).unwrap();
        }

        let got = get_transaction_table_rows_paginated(5, 0, SortOrder::Ascending, &conn).unwrap();

        assert_eq!(got.len(), 5, "got {} transactions, want 5", got.len());
    }

    #[test]
    fn get_transactions_with_offset() {
        let conn = get_test_connection();
        let offset = 10;
        let limit = 5;
        let today = date!(2025 - 10 - 05);
        let mut want = Vec::new();
        for i in 1..20 {
            let transaction = create_transaction(Transaction::build(i as f64, today, ""), &conn)
                .expect("Could not create transaction");

            if i > offset && i <= offset + limit {
                want.push(TableTransaction {
                    id: i as TransactionId,
                    amount: transaction.amount,
                    date: transaction.date,
                    description: transaction.description.clone(),
                    tag_name: None,
                });
            }
        }

        let got = get_transaction_table_rows_paginated(limit, offset, SortOrder::Ascending, &conn)
            .expect("Could not query transactions");

        assert_eq!(want, got);
    }
}
