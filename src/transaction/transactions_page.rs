//! Defines the route handler for the page that displays transactions as a table.
use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Query, State},
    response::{IntoResponse, Redirect, Response},
};
use rusqlite::Connection;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error, endpoints,
    tag::{
        Tag, TagId, TagWithExclusion, build_tags_with_exclusion_status, get_all_tags,
        get_excluded_tags,
    },
    timezone::get_local_offset,
};

use super::{
    grouping::{GroupingOptions, group_transactions},
    models::{Transaction, TransactionTableRow, TransactionsViewOptions},
    query::{SortOrder, get_transaction_table_rows_in_range},
    range::{
        DateRange, IntervalPreset, RangeNavLink, RangeNavigation, RangePreset, RangeQuery,
        compute_range, get_transaction_date_bounds, range_preset_can_contain_interval,
        smallest_range_for_interval,
    },
    view::transactions_view,
};

struct TransactionsInputs {
    /// Normalized options derived from query params.
    options: NormalizedQuery,
    /// Optional min/max transaction dates for the data set.
    bounds: Option<DateRange>,
    /// The date range for the active range.
    range: DateRange,
    /// Tag IDs excluded from interval totals and summaries.
    excluded_tag_ids: Vec<TagId>,
    /// Tags available for exclusion controls.
    available_tags: Vec<Tag>,
    /// Raw transaction rows from the database.
    transactions: Vec<Transaction>,
}

struct TransactionsViewModel {
    /// Grouped and summarized transactions for rendering.
    grouped: Vec<super::models::DateInterval>,
    /// Navigation model for range links.
    range_nav: RangeNavigation,
    /// Optional link to the latest range.
    latest_link: Option<RangeNavLink>,
    /// Whether the dataset contains any transactions at all.
    has_any_transactions: bool,
    /// Tags with exclusion state for controls.
    tags_with_status: Vec<TagWithExclusion>,
    /// Redirect URL back to the current transactions range.
    redirect_url: String,
    /// Selected view options for the page.
    options: TransactionsViewOptions,
}

struct NormalizedQuery {
    /// Range preset for navigation.
    range_preset: RangePreset,
    /// Interval preset for grouping.
    interval_preset: IntervalPreset,
    /// Whether category summary mode is enabled.
    show_category_summary: bool,
    /// Anchor date for range calculations.
    anchor_date: Date,
}

enum QueryDecision {
    Redirect(String),
    Normalized(NormalizedQuery),
}

/// The state needed for the transactions page.
#[derive(Debug, Clone)]
pub struct TransactionsViewState {
    /// The database connection for managing transactions.
    db_connection: Arc<Mutex<Connection>>,
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    local_timezone: String,
}

impl FromRef<AppState> for TransactionsViewState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
            local_timezone: state.local_timezone.clone(),
        }
    }
}

/// Render an overview of the user's transactions.
pub async fn get_transactions_page(
    State(state): State<TransactionsViewState>,
    Query(query_params): Query<RangeQuery>,
) -> Result<Response, Error> {
    let now_local = current_local_date(&state.local_timezone)?;
    let options = match normalize_query(query_params, now_local) {
        QueryDecision::Normalized(options) => options,
        QueryDecision::Redirect(redirect_url) => {
            return Ok(Redirect::to(&redirect_url).into_response());
        }
    };
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;
    let bounds = get_transaction_date_bounds(&connection)
        .inspect_err(|error| tracing::error!("could not get transaction bounds: {error}"))?;
    let range = compute_range(options.range_preset, options.anchor_date);
    let excluded_tag_ids = get_excluded_tags(&connection)
        .inspect_err(|error| tracing::error!("could not get excluded tags: {error}"))?;
    let available_tags = get_all_tags(&connection)
        .inspect_err(|error| tracing::error!("could not get tags: {error}"))?;

    let transactions =
        get_transaction_table_rows_in_range(range, SortOrder::Descending, &connection)
            .inspect_err(|error| {
                tracing::error!("could not get transaction table rows: {error}")
            })?;

    let model = build_transactions_view_model(TransactionsInputs {
        options,
        bounds,
        range,
        excluded_tag_ids,
        available_tags,
        transactions,
    });

    Ok(transactions_view(
        model.grouped,
        &model.range_nav,
        model.latest_link.as_ref(),
        model.has_any_transactions,
        &model.tags_with_status,
        &model.redirect_url,
        model.options,
    )
    .into_response())
}

fn current_local_date(local_timezone: &str) -> Result<Date, Error> {
    let Some(local_offset) = get_local_offset(local_timezone) else {
        tracing::error!("Invalid timezone {}", local_timezone);
        return Err(Error::InvalidTimezoneError(local_timezone.to_owned()));
    };

    Ok(OffsetDateTime::now_utc().to_offset(local_offset).date())
}

fn build_redirect_param(redirect_url: &str) -> Option<String> {
    serde_urlencoded::to_string([("redirect_url", &redirect_url)])
        .inspect_err(|error| {
            tracing::error!(
                "Could not set redirect URL {redirect_url} due to encoding error: {error}"
            );
        })
        .ok()
}

fn transactions_page_url(
    range_preset: RangePreset,
    interval_preset: IntervalPreset,
    show_category_summary: bool,
    anchor_date: Date,
) -> String {
    let summary_param = if show_category_summary {
        "&summary=true"
    } else {
        ""
    };

    format!(
        "{}?range={}&interval={}&anchor={}{summary_param}",
        endpoints::TRANSACTIONS_VIEW,
        range_preset.as_query_value(),
        interval_preset.as_query_value(),
        anchor_date,
    )
}

fn normalize_query(query: RangeQuery, now_local: Date) -> QueryDecision {
    let requested_range_preset = query.range.unwrap_or(RangePreset::default_preset());
    let interval_preset = query.interval.unwrap_or(IntervalPreset::default_preset());
    let show_category_summary = query.summary.unwrap_or(false);
    let anchor_date = query.anchor.unwrap_or(now_local);
    let range_preset = if range_preset_can_contain_interval(requested_range_preset, interval_preset)
    {
        requested_range_preset
    } else {
        smallest_range_for_interval(interval_preset)
    };

    if range_preset != requested_range_preset {
        let redirect_url = transactions_page_url(
            range_preset,
            interval_preset,
            show_category_summary,
            anchor_date,
        );
        return QueryDecision::Redirect(redirect_url);
    }

    QueryDecision::Normalized(NormalizedQuery {
        range_preset,
        interval_preset,
        show_category_summary,
        anchor_date,
    })
}

fn build_transactions_view_model(input: TransactionsInputs) -> TransactionsViewModel {
    let range_nav = RangeNavigation::new(input.options.range_preset, input.range, input.bounds);
    let latest_link = input.bounds.and_then(|bounds| {
        let latest_range = compute_range(input.options.range_preset, bounds.end);
        if latest_range == input.range {
            None
        } else {
            Some(RangeNavLink::new(input.options.range_preset, latest_range))
        }
    });
    let has_any_transactions = input.bounds.is_some();

    let redirect_url = transactions_page_url(
        input.options.range_preset,
        input.options.interval_preset,
        input.options.show_category_summary,
        input.options.anchor_date,
    );
    let redirect_param = build_redirect_param(&redirect_url);

    let tags_with_status =
        build_tags_with_exclusion_status(input.available_tags, &input.excluded_tag_ids);

    let redirect_param = redirect_param.as_deref();
    let transaction_rows = input
        .transactions
        .into_iter()
        .map(|transaction| TransactionTableRow::new_from_transaction(transaction, redirect_param))
        .collect::<Vec<_>>();

    let grouped = group_transactions(
        transaction_rows,
        GroupingOptions {
            interval_preset: input.options.interval_preset,
            excluded_tag_ids: &input.excluded_tag_ids,
            show_category_summary: input.options.show_category_summary,
        },
    );

    TransactionsViewModel {
        grouped,
        range_nav,
        latest_link,
        has_any_transactions,
        tags_with_status,
        redirect_url,
        options: TransactionsViewOptions {
            range_preset: input.options.range_preset,
            interval_preset: input.options.interval_preset,
            show_category_summary: input.options.show_category_summary,
            anchor_date: input.options.anchor_date,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::{Query, State},
        response::Response,
    };
    use rusqlite::Connection;
    use scraper::{ElementRef, Html, Selector};
    use time::{Date, macros::date};

    use crate::{
        db::initialize,
        tag::{TagName, create_tag, save_excluded_tags},
        transaction::{Transaction, create_transaction},
    };

    use super::{TransactionsViewState, get_transactions_page, transactions_page_url};
    use crate::transaction::range::{
        IntervalPreset, RangePreset, RangeQuery, compute_range, range_anchor_query,
    };

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
    async fn transactions_page_displays_range_data() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);

        for i in 1..=3 {
            create_transaction(Transaction::build(i as f64, today, ""), &conn).unwrap();
        }

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };
        let want_transactions = [
            Transaction {
                id: 1,
                amount: 1.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
                tag_id: None,
            },
            Transaction {
                id: 2,
                amount: 2.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
                tag_id: None,
            },
            Transaction {
                id: 3,
                amount: 3.0,
                date: today,
                description: "".to_owned(),
                import_id: None,
                tag_id: None,
            },
        ];

        let response = get_transactions_page(
            State(state),
            Query(RangeQuery {
                range: Some(RangePreset::Month),
                interval: None,
                summary: None,
                anchor: Some(today),
            }),
        )
        .await
        .unwrap();

        let html = parse_html(response).await;
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_has_transactions(table, &want_transactions);
        assert_range_navigation_present(&html);
    }

    #[tokio::test]
    async fn transactions_page_shows_navigation_with_empty_range() {
        let conn = get_test_connection();
        let transaction_date = date!(2025 - 10 - 05);
        let anchor = date!(2025 - 01 - 05);

        create_transaction(Transaction::build(1.0, transaction_date, ""), &conn).unwrap();

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_transactions_page(
            State(state),
            Query(RangeQuery {
                range: Some(RangePreset::Month),
                interval: None,
                summary: None,
                anchor: Some(anchor),
            }),
        )
        .await
        .unwrap();

        let html = parse_html(response).await;
        assert_valid_html(&html);
        assert_range_navigation_present(&html);
        assert_empty_state_present(&html);
    }

    #[tokio::test]
    async fn transactions_page_shows_latest_link_when_not_latest_range() {
        let conn = get_test_connection();
        let transaction_date = date!(2025 - 10 - 05);
        let anchor = date!(2025 - 08 - 05);

        create_transaction(Transaction::build(1.0, transaction_date, ""), &conn).unwrap();

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_transactions_page(
            State(state),
            Query(RangeQuery {
                range: Some(RangePreset::Month),
                interval: None,
                summary: None,
                anchor: Some(anchor),
            }),
        )
        .await
        .unwrap();

        let html = parse_html(response).await;
        assert_valid_html(&html);
        assert_latest_link_present(&html, RangePreset::Month, transaction_date);
    }

    #[tokio::test]
    async fn transactions_page_shows_summary_empty_state_when_all_excluded() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);

        let tag = create_tag(TagName::new_unchecked("Excluded"), &conn).unwrap();
        save_excluded_tags(&[tag.id], &conn).unwrap();
        create_transaction(
            Transaction::build(50.0, today, "Excluded transaction").tag_id(Some(tag.id)),
            &conn,
        )
        .unwrap();

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_transactions_page(
            State(state),
            Query(RangeQuery {
                range: Some(RangePreset::Month),
                interval: None,
                summary: Some(true),
                anchor: Some(today),
            }),
        )
        .await
        .unwrap();

        let html = parse_html(response).await;
        assert_valid_html(&html);
        assert_summary_empty_state_present(&html);
    }

    #[tokio::test]
    async fn transactions_page_autoselects_range_when_interval_exceeds_range() {
        let conn = get_test_connection();
        let transaction_date = date!(2025 - 10 - 05);

        create_transaction(Transaction::build(1.0, transaction_date, ""), &conn).unwrap();

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_transactions_page(
            State(state),
            Query(RangeQuery {
                range: Some(RangePreset::Week),
                interval: Some(IntervalPreset::Month),
                summary: None,
                anchor: Some(transaction_date),
            }),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::SEE_OTHER);
        let location = response
            .headers()
            .get("location")
            .expect("Missing redirect location header");
        let expected_url = transactions_page_url(
            RangePreset::Month,
            IntervalPreset::Month,
            false,
            transaction_date,
        );
        assert_eq!(
            location,
            expected_url.as_str(),
            "Expected redirect to adjusted range preset"
        );
    }

    #[track_caller]
    fn must_get_table(html: &Html) -> ElementRef<'_> {
        html.select(&Selector::parse("table").unwrap())
            .next()
            .expect("No table found")
    }

    #[track_caller]
    fn assert_table_has_transactions(table: ElementRef, transactions: &[Transaction]) {
        let row_selector = Selector::parse("tbody tr[data-transaction-row='true']").unwrap();
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
    fn assert_range_navigation_present(html: &Html) {
        let nav_selector = Selector::parse("nav.pagination > ul.pagination").unwrap();
        let nav = html
            .select(&nav_selector)
            .next()
            .expect("No range navigation found");

        let current_selector = Selector::parse("[aria-current='page']").unwrap();
        nav.select(&current_selector)
            .next()
            .expect("Range nav should include aria-current for range label");
    }

    #[track_caller]
    fn assert_latest_link_present(html: &Html, preset: RangePreset, latest_date: Date) {
        let latest_range = compute_range(preset, latest_date);
        let latest_href = range_anchor_query(preset, latest_range.end);
        let link_selector = Selector::parse("a").unwrap();
        let latest_link = html
            .select(&link_selector)
            .find(|link| link.text().collect::<String>().trim() == "Latest")
            .expect("No Latest link found");
        let href = latest_link
            .value()
            .attr("href")
            .expect("Latest link missing href");
        assert!(
            href.contains(&latest_href),
            "Latest link href did not include expected query. want {latest_href}, got {href}"
        );
    }

    #[track_caller]
    fn assert_empty_state_present(html: &Html) {
        let empty_row_selector = Selector::parse("tbody tr td[data-empty-state='true']").unwrap();
        let empty_row = html
            .select(&empty_row_selector)
            .next()
            .expect("No empty-state row found");
        let text = empty_row.text().collect::<String>();
        assert!(
            text.contains("No transactions in this range."),
            "Empty-state row did not include expected text: {text}"
        );
    }

    #[track_caller]
    fn assert_summary_empty_state_present(html: &Html) {
        let empty_row_selector = Selector::parse("tbody tr td[data-empty-state='true']").unwrap();
        let empty_row = html
            .select(&empty_row_selector)
            .next()
            .expect("No empty-state row found");
        let text = empty_row.text().collect::<String>();
        assert!(
            text.contains("No transactions in this summary after exclusions."),
            "Summary empty-state row did not include expected text: {text}"
        );
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
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_transactions_page(
            State(state),
            Query(RangeQuery {
                range: Some(RangePreset::Month),
                interval: None,
                summary: None,
                anchor: Some(today),
            }),
        )
        .await
        .unwrap();

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
            .select(&Selector::parse("tbody tr[data-transaction-row='true']").unwrap())
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

    #[tokio::test]
    async fn transactions_page_shows_excluded_tags_controls() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);

        let groceries = create_tag(TagName::new_unchecked("Groceries"), &conn).unwrap();
        let rent = create_tag(TagName::new_unchecked("Rent"), &conn).unwrap();
        save_excluded_tags(&[groceries.id], &conn).unwrap();

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_transactions_page(
            State(state),
            Query(RangeQuery {
                range: Some(RangePreset::Month),
                interval: None,
                summary: None,
                anchor: Some(today),
            }),
        )
        .await
        .unwrap();

        let html = parse_html(response).await;
        assert_valid_html(&html);

        let checkbox_selector =
            Selector::parse("input[type='checkbox'][name='excluded_tags']").unwrap();
        let checkboxes: Vec<_> = html.select(&checkbox_selector).collect();
        assert_eq!(checkboxes.len(), 2, "Expected two excluded tag checkboxes");

        let mut found_groceries = false;
        let mut found_rent = false;

        for checkbox in checkboxes {
            let value = checkbox
                .value()
                .attr("value")
                .expect("Checkbox missing value attribute");
            let is_checked = checkbox.value().attr("checked").is_some();

            if value == groceries.id.to_string() {
                found_groceries = true;
                assert!(is_checked, "Groceries should be marked as excluded");
            } else if value == rent.id.to_string() {
                found_rent = true;
                assert!(
                    !is_checked,
                    "Rent should not be marked as excluded by default"
                );
            }
        }

        assert!(found_groceries, "Groceries checkbox should be present");
        assert!(found_rent, "Rent checkbox should be present");
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
