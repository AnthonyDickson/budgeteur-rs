//! Defines the route handler for the page that displays transactions as a table.
use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Query, State},
    http::Uri,
    response::{IntoResponse, Redirect, Response},
};
use maud::{Markup, html};
use rusqlite::Connection;
use time::{Date, Month, OffsetDateTime};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    AppState, Error, endpoints,
    html::{
        BUTTON_DELETE_STYLE, LINK_STYLE, PAGE_CONTAINER_STYLE, TABLE_CELL_STYLE,
        TABLE_HEADER_STYLE, TABLE_ROW_STYLE, TAG_BADGE_STYLE, base, format_currency,
    },
    navigation::NavBar,
    tag::{TagId, TagName, get_excluded_tags},
    timezone::get_local_offset,
    transaction::TransactionId,
};

use super::window::{
    BucketPreset, WindowNavLink, WindowNavigation, WindowPreset, WindowQuery, WindowRange,
    compute_bucket_range, compute_window_range, get_transaction_date_bounds,
    smallest_window_for_bucket, window_preset_can_contain_bucket, window_range_label,
};

/// The max number of graphemes to display in the transaction table rows before
/// trunctating and displaying elipses.
const MAX_DESCRIPTION_GRAPHEMES: usize = 32;

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
    /// The ID of the transactions tag.
    tag_id: Option<TagId>,
}

/// Renders a transaction with its tags as a table row.
#[derive(Debug, PartialEq)]
struct TransactionTableRow {
    /// The amount of money spent or earned in this transaction.
    amount: f64,
    /// When the transaction happened.
    date: Date,
    /// A text description of what the transaction was for.
    description: String,
    /// The name of the transactions tag.
    tag_name: Option<TagName>,
    /// The ID of the transactions tag.
    tag_id: Option<TagId>,
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
            tag_id: transaction.tag_id,
            edit_url,
            delete_url: endpoints::format_endpoint(endpoints::DELETE_TRANSACTION, transaction.id),
        }
    }
}

#[derive(Debug, PartialEq)]
struct BucketTotals {
    income: f64,
    expenses: f64,
}

#[derive(Debug, PartialEq)]
struct DayGroup {
    date: Date,
    transactions: Vec<TransactionTableRow>,
}

#[derive(Debug, PartialEq)]
struct DateBucket {
    range: WindowRange,
    totals: BucketTotals,
    days: Vec<DayGroup>,
}

impl DateBucket {
    fn new(range: WindowRange) -> Self {
        Self {
            range,
            totals: BucketTotals {
                income: 0.0,
                expenses: 0.0,
            },
            days: Vec::new(),
        }
    }
}

/// Render an overview of the user's transactions.
pub async fn get_transactions_page(
    State(state): State<TransactionsViewState>,
    Query(query_params): Query<WindowQuery>,
) -> Result<Response, Error> {
    let requested_window_preset = query_params
        .window
        .unwrap_or(WindowPreset::default_preset());
    let bucket_preset = query_params
        .bucket
        .unwrap_or(BucketPreset::default_preset());
    let anchor_date = match query_params.anchor {
        Some(anchor) => anchor,
        None => default_anchor_date(&state.local_timezone)?,
    };
    let window_preset = if window_preset_can_contain_bucket(requested_window_preset, bucket_preset)
    {
        requested_window_preset
    } else {
        smallest_window_for_bucket(bucket_preset)
    };
    if window_preset != requested_window_preset {
        let redirect_url = transactions_page_url(window_preset, bucket_preset, anchor_date);
        return Ok(Redirect::to(&redirect_url).into_response());
    }
    let connection = state.db_connection.lock().unwrap();
    let bounds = get_transaction_date_bounds(&connection)
        .inspect_err(|error| tracing::error!("could not get transaction bounds: {error}"))?;
    let window_range = compute_window_range(window_preset, anchor_date);
    let window_nav = WindowNavigation::new(window_preset, window_range, bounds);
    let latest_link = bounds.and_then(|bounds| {
        let latest_range = compute_window_range(window_preset, bounds.end);
        if latest_range == window_range {
            None
        } else {
            Some(WindowNavLink::new(window_preset, latest_range))
        }
    });
    let has_any_transactions = bounds.is_some();

    let redirect_url = get_redirect_url(window_preset, bucket_preset, anchor_date);

    let excluded_tag_ids = get_excluded_tags(&connection)
        .inspect_err(|error| tracing::error!("could not get excluded tags: {error}"))?;

    let transactions =
        get_transaction_table_rows_in_range(window_range, SortOrder::Descending, &connection)
            .inspect_err(|error| tracing::error!("could not get transaction table rows: {error}"))?
            .into_iter()
            .map(|transaction| {
                TransactionTableRow::new_from_transaction(transaction, redirect_url.as_deref())
            })
            .collect::<Vec<_>>();

    let grouped_transactions =
        group_transactions_by_bucket(transactions, bucket_preset, &excluded_tag_ids);

    Ok(transactions_view(
        grouped_transactions,
        &window_nav,
        latest_link.as_ref(),
        has_any_transactions,
        window_preset,
        bucket_preset,
        anchor_date,
    )
    .into_response())
}

fn default_anchor_date(local_timezone: &str) -> Result<Date, Error> {
    let Some(local_offset) = get_local_offset(local_timezone) else {
        tracing::error!("Invalid timezone {}", local_timezone);
        return Err(Error::InvalidTimezoneError(local_timezone.to_owned()));
    };

    Ok(OffsetDateTime::now_utc().to_offset(local_offset).date())
}

fn get_redirect_url(
    window_preset: WindowPreset,
    bucket_preset: BucketPreset,
    anchor_date: Date,
) -> Option<String> {
    let redirect_url = transactions_page_url(window_preset, bucket_preset, anchor_date);

    serde_urlencoded::to_string([("redirect_url", &redirect_url)])
        .inspect_err(|error| {
            tracing::error!(
                "Could not set redirect URL {redirect_url} due to encoding error: {error}"
            );
        })
        .ok()
}

fn transactions_page_url(
    window_preset: WindowPreset,
    bucket_preset: BucketPreset,
    anchor_date: Date,
) -> String {
    format!(
        "{}?window={}&bucket={}&anchor={}",
        endpoints::TRANSACTIONS_VIEW,
        window_preset.as_query_value(),
        bucket_preset.as_query_value(),
        anchor_date
    )
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

/// Get transactions with sorting by date in a windowed date range.
///
/// # Arguments
/// * `window_range` - Inclusive date range of transactions to return
/// * `sort_order` - Sort direction for date field
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
/// - Transaction row mapping fails
fn get_transaction_table_rows_in_range(
    window_range: WindowRange,
    sort_order: SortOrder,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
    let order_clause = match sort_order {
        SortOrder::Ascending => "ORDER BY date ASC",
        SortOrder::Descending => "ORDER BY date DESC",
    };

    // Sort by date, and then ID to keep transaction order stable after updates
    let query = format!(
        "SELECT \"transaction\".id, amount, date, description, tag.name, tag.id FROM \"transaction\" \
        LEFT JOIN tag ON \"transaction\".tag_id = tag.id \
        WHERE \"transaction\".date BETWEEN ?1 AND ?2 \
        {}, \"transaction\".id ASC",
        order_clause
    );

    connection
        .prepare(&query)?
        .query_map(
            [window_range.start.to_string(), window_range.end.to_string()],
            |row| {
                let tag_name = row
                    .get::<usize, Option<String>>(4)?
                    .map(|some_tag_name| TagName::new_unchecked(&some_tag_name));

                Ok(Transaction {
                    id: row.get(0)?,
                    amount: row.get(1)?,
                    date: row.get(2)?,
                    description: row.get(3)?,
                    tag_name,
                    tag_id: row.get(5)?,
                })
            },
        )?
        .map(|transaction_result| transaction_result.map_err(Error::SqlError))
        .collect()
}

fn window_navigation_html(
    window_nav: &WindowNavigation,
    latest_link: Option<&WindowNavLink>,
    bucket_preset: BucketPreset,
    transactions_page_route: &Uri,
) -> Markup {
    let current_label = window_range_label(window_nav.range);
    let row_classes = if latest_link.is_some() {
        "grid-rows-2 gap-y-0.5"
    } else {
        "grid-rows-1"
    };

    html! {
        nav class="pagination flex justify-center"
        {
            ul class={ "pagination grid grid-cols-3 gap-x-4 p-0 m-0 items-center w-full " (row_classes) }
            {
                @if let Some(prev) = &window_nav.prev {
                    li class="flex items-center justify-start row-start-1" {
                        a
                            href={(transactions_page_route) "?" (&prev.href) "&bucket=" (bucket_preset.as_query_value())}
                            role="button"
                            class="block px-3 py-2 rounded-sm text-blue-600 hover:underline"
                        { (window_range_label(prev.range)) }
                    }
                } @else {
                    li class="flex items-center justify-start row-start-1" {}
                }
                li class="flex items-center justify-center row-start-1" {
                    span
                        aria-current="page"
                        class="block px-3 py-2 rounded-sm font-bold text-black dark:text-white"
                    { (current_label) }
                }
                @if let Some(next) = &window_nav.next {
                    li class="flex items-center justify-end row-start-1" {
                        a
                            href={(transactions_page_route) "?" (&next.href) "&bucket=" (bucket_preset.as_query_value())}
                            role="button"
                            class="block px-3 py-2 rounded-sm text-blue-600 hover:underline"
                        { (window_range_label(next.range)) }
                    }
                } @else {
                    li class="flex items-center justify-end row-start-1" {}
                }

                @if let Some(latest) = latest_link {
                    li class="flex items-center justify-center row-start-2 col-start-2" {
                        a
                            href={(transactions_page_route) "?" (&latest.href) "&bucket=" (bucket_preset.as_query_value())}
                            role="button"
                            class="block px-3 pb-1 text-blue-600 hover:underline"
                        { "Latest" }
                    }
                }
            }
        }
    }
}

fn transaction_row_view(row: &TransactionTableRow) -> Markup {
    let amount_str = format_currency(row.amount);
    let description_length = row.description.graphemes(true).count();

    // Truncate long descriptions to prevent visual artifacts from the table growing too wide.
    let (description, tooltip) = if description_length <= MAX_DESCRIPTION_GRAPHEMES {
        (row.description.clone(), None)
    } else {
        let description: String = row
            .description
            .graphemes(true)
            .take(MAX_DESCRIPTION_GRAPHEMES - 3)
            .collect();
        let description = description + "...";
        (description, Some(&row.description))
    };

    html! {
        tr class=(TABLE_ROW_STYLE) data-transaction-row="true"
        {
            td class="px-6 py-4 text-right" { (amount_str) }
            td class="sr-only" { (row.date) }
            td class=(TABLE_CELL_STYLE) title=[tooltip] { (description) }
            td class=(TABLE_CELL_STYLE)
            {
                @if let Some(ref tag_name) = row.tag_name {
                    span class=(TAG_BADGE_STYLE)
                    {
                        (tag_name)
                    }
                } @else {
                    span class="text-gray-400 dark:text-gray-500" { "-" }
                }
            }
            td class=(TABLE_CELL_STYLE)
            {
                div class="flex gap-4"
                {
                    a href=(row.edit_url) class=(LINK_STYLE)
                    {
                        "Edit"
                    }

                    button
                        hx-delete=(row.delete_url)
                        hx-confirm={
                            "Are you sure you want to delete the transaction '"
                            (row.description) "'? This cannot be undone."
                        }
                        hx-target="closest tr"
                        hx-target-error="#alert-container"
                        hx-swap="outerHTML"
                        class=(BUTTON_DELETE_STYLE)
                    {
                       "Delete"
                    }
                }
            }
        }
    }
}

fn transactions_view(
    grouped_transactions: Vec<DateBucket>,
    window_nav: &WindowNavigation,
    latest_link: Option<&WindowNavLink>,
    has_any_transactions: bool,
    window_preset: WindowPreset,
    bucket_preset: BucketPreset,
    anchor_date: Date,
) -> Markup {
    let create_transaction_route = Uri::from_static(endpoints::NEW_TRANSACTION_VIEW);
    let import_transaction_route = Uri::from_static(endpoints::IMPORT_VIEW);
    let transactions_page_route = Uri::from_static(endpoints::TRANSACTIONS_VIEW);
    let nav_bar = NavBar::new(endpoints::TRANSACTIONS_VIEW).into_html();
    // Cache this result so it can be accessed after `grouped_transactions` is moved by for loop.
    let transactions_empty = grouped_transactions.is_empty();

    let content = html! {
        (nav_bar)

        div class=(PAGE_CONTAINER_STYLE)
        {
            div class="relative"
            {
                div class="flex justify-between flex-wrap items-end mb-4"
                {
                    h1 class="text-xl font-bold" { "Transactions" }

                    a href=(import_transaction_route) class=(LINK_STYLE)
                    {
                        "Import Transactions"
                    }

                    a href=(create_transaction_route) class=(LINK_STYLE)
                    {
                        "Create Transaction"
                    }
                }

                div class="dark:bg-gray-800"
                {
                    @if has_any_transactions {
                        (window_navigation_html(
                            window_nav,
                            latest_link,
                            bucket_preset,
                            &transactions_page_route,
                        ))
                    }

                    (bucket_controls_html(
                        window_preset,
                        bucket_preset,
                        anchor_date,
                        &transactions_page_route,
                    ))

                    table class="w-full my-2 text-sm text-left rtl:text-right
                        text-gray-500 dark:text-gray-400"
                    {
                        thead class=(TABLE_HEADER_STYLE)
                        {
                            tr
                            {
                                th scope="col" class="px-6 py-3 text-right"
                                {
                                    "Amount"
                                }
                                th scope="col" class="sr-only"
                                {
                                    "Date"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Description"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Tags"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Actions"
                                }
                            }
                        }

                        tbody
                        {
                            @for bucket in grouped_transactions {
                                (bucket_header_row_view(&bucket))

                                @for day in &bucket.days {
                                    (day_header_row_view(day.date))

                                    @for transaction_row in &day.transactions {
                                        (transaction_row_view(transaction_row))
                                    }
                                }
                            }

                            @if transactions_empty {
                                tr
                                {
                                    td colspan="5" class="px-6 py-4 text-center" {
                                        "No transactions in this range."
                                    }
                                }
                            }
                        }
                    }

                    @if has_any_transactions {
                        (window_navigation_html(
                            window_nav,
                            latest_link,
                            bucket_preset,
                            &transactions_page_route,
                        ))
                    }
                }
            }
        }
    };

    base("Transactions", &[], &content)
}

fn group_transactions_by_bucket(
    transactions: Vec<TransactionTableRow>,
    bucket_preset: BucketPreset,
    excluded_tag_ids: &[TagId],
) -> Vec<DateBucket> {
    let mut buckets: Vec<DateBucket> = Vec::new();

    for transaction in transactions {
        let bucket_range = compute_bucket_range(bucket_preset, transaction.date);
        let bucket = match buckets.last_mut() {
            Some(current) if current.range == bucket_range => current,
            _ => {
                buckets.push(DateBucket::new(bucket_range));
                buckets.last_mut().expect("bucket just added")
            }
        };

        if transaction
            .tag_id
            .map(|tag_id| !excluded_tag_ids.contains(&tag_id))
            .unwrap_or(true)
        {
            if transaction.amount < 0.0 {
                bucket.totals.expenses += transaction.amount;
            } else {
                bucket.totals.income += transaction.amount;
            }
        }

        let day_group = match bucket.days.last_mut() {
            Some(current) if current.date == transaction.date => current,
            _ => {
                bucket.days.push(DayGroup {
                    date: transaction.date,
                    transactions: Vec::new(),
                });
                bucket.days.last_mut().expect("day group just added")
            }
        };

        day_group.transactions.push(transaction);
    }

    buckets
}

fn bucket_header_row_view(bucket: &DateBucket) -> Markup {
    let label = window_range_label(bucket.range);
    let income = format_currency(bucket.totals.income);
    let expenses = format_currency(bucket.totals.expenses);

    html! {
        tr class="bg-gray-50 dark:bg-gray-700" data-bucket-header="true"
        {
            td colspan="5" class="px-6 py-3"
            {
                div class="flex items-center justify-between font-semibold text-gray-900 dark:text-white"
                {
                    span { (label) }
                    span class="flex items-center gap-4"
                    {
                        span class="text-green-700 dark:text-green-300" { (income) }
                        span class="text-red-700 dark:text-red-300" { (expenses) }
                    }
                }
            }
        }
    }
}

fn day_header_row_view(date: Date) -> Markup {
    let label = format_day_label(date);

    html! {
        tr class="bg-gray-50 dark:bg-gray-800" data-day-header="true"
        {
            td colspan="5" class="px-6 py-2 text-xs font-semibold uppercase text-gray-600 dark:text-gray-300"
            {
                (label)
            }
        }
    }
}

fn format_day_label(date: Date) -> String {
    format!("{:02} {}", date.day(), month_abbrev(date.month()))
}

fn month_abbrev(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

fn bucket_controls_html(
    window_preset: WindowPreset,
    bucket_preset: BucketPreset,
    anchor_date: Date,
    transactions_page_route: &Uri,
) -> Markup {
    let bucket_presets = [
        BucketPreset::Week,
        BucketPreset::Fortnight,
        BucketPreset::Month,
        BucketPreset::Quarter,
        BucketPreset::HalfYear,
        BucketPreset::Year,
    ];
    let bucket_links: Vec<(BucketPreset, String)> = bucket_presets
        .iter()
        .map(|preset| {
            let href = format!(
                "{route}?window={window}&bucket={bucket}&anchor={anchor}",
                route = transactions_page_route,
                window = window_preset.as_query_value(),
                bucket = preset.as_query_value(),
                anchor = anchor_date
            );
            (*preset, href)
        })
        .collect();

    html! {
        div class="flex flex-wrap items-center gap-2 px-6 py-2 text-sm text-gray-600 dark:text-gray-300"
        {
            span class="font-semibold text-gray-900 dark:text-white" { "Bucket:" }
            @for (preset, href) in bucket_links {
                @if preset == bucket_preset {
                    span class="px-2 py-1 rounded bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white"
                    { (preset.label()) }
                } @else {
                    a
                        class="px-2 py-1 rounded text-blue-600 hover:underline"
                        href=(href)
                    { (preset.label()) }
                }
            }
        }
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
        tag::{TagName, create_tag},
        transaction::{Transaction, create_transaction},
    };

    use super::{TransactionsViewState, get_transactions_page, transactions_page_url};
    use crate::transaction::window::{
        BucketPreset, WindowPreset, WindowQuery, compute_window_range, window_anchor_query,
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
    async fn transactions_page_displays_windowed_data() {
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
            Query(WindowQuery {
                window: Some(WindowPreset::Month),
                bucket: None,
                anchor: Some(today),
            }),
        )
        .await
        .unwrap();

        let html = parse_html(response).await;
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_has_transactions(table, &want_transactions);
        assert_window_navigation_present(&html);
    }

    #[tokio::test]
    async fn transactions_page_shows_navigation_with_empty_window() {
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
            Query(WindowQuery {
                window: Some(WindowPreset::Month),
                bucket: None,
                anchor: Some(anchor),
            }),
        )
        .await
        .unwrap();

        let html = parse_html(response).await;
        assert_valid_html(&html);
        assert_window_navigation_present(&html);
        assert_empty_state_present(&html);
    }

    #[tokio::test]
    async fn transactions_page_shows_latest_link_when_not_latest_window() {
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
            Query(WindowQuery {
                window: Some(WindowPreset::Month),
                bucket: None,
                anchor: Some(anchor),
            }),
        )
        .await
        .unwrap();

        let html = parse_html(response).await;
        assert_valid_html(&html);
        assert_latest_link_present(&html, WindowPreset::Month, transaction_date);
    }

    #[tokio::test]
    async fn transactions_page_autoselects_window_when_bucket_exceeds_window() {
        let conn = get_test_connection();
        let transaction_date = date!(2025 - 10 - 05);

        create_transaction(Transaction::build(1.0, transaction_date, ""), &conn).unwrap();

        let state = TransactionsViewState {
            db_connection: Arc::new(Mutex::new(conn)),
            local_timezone: "Etc/UTC".to_owned(),
        };

        let response = get_transactions_page(
            State(state),
            Query(WindowQuery {
                window: Some(WindowPreset::Week),
                bucket: Some(BucketPreset::Month),
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
            WindowPreset::Month,
            BucketPreset::Month,
            transaction_date,
        );
        assert_eq!(
            location,
            expected_url.as_str(),
            "Expected redirect to adjusted window preset"
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
    fn assert_window_navigation_present(html: &Html) {
        let nav_selector = Selector::parse("nav.pagination > ul.pagination").unwrap();
        let nav = html
            .select(&nav_selector)
            .next()
            .expect("No window navigation found");

        let current_selector = Selector::parse("[aria-current='page']").unwrap();
        nav.select(&current_selector)
            .next()
            .expect("Window nav should include aria-current for range label");
    }

    #[track_caller]
    fn assert_latest_link_present(html: &Html, preset: WindowPreset, latest_date: Date) {
        let latest_range = compute_window_range(preset, latest_date);
        let latest_href = window_anchor_query(preset, latest_range.end);
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
        let empty_row_selector = Selector::parse("tbody tr td[colspan='5']").unwrap();
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
            Query(WindowQuery {
                window: Some(WindowPreset::Month),
                bucket: None,
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
        db::initialize,
        transaction::{
            Transaction, TransactionId, create_transaction,
            transactions_page::{
                SortOrder, Transaction as TableTransaction, get_transaction_table_rows_in_range,
            },
            window::WindowRange,
        },
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn get_transactions_in_range() {
        let conn = get_test_connection();

        let today = OffsetDateTime::now_utc().date();

        for i in 0..10 {
            let transaction_builder = Transaction::build(
                (i + 1) as f64,
                today - Duration::days(i),
                &format!("transaction #{i}"),
            );

            create_transaction(transaction_builder, &conn).unwrap();
        }

        let window_range = WindowRange {
            start: today - Duration::days(4),
            end: today,
        };
        let got =
            get_transaction_table_rows_in_range(window_range, SortOrder::Ascending, &conn).unwrap();

        assert_eq!(got.len(), 5, "got {} transactions, want 5", got.len());
    }

    #[test]
    fn get_transactions_in_range_orders_by_date() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let mut want = Vec::new();
        for i in 1..=6 {
            let date = if i <= 3 {
                today
            } else {
                today - Duration::days(1)
            };
            let transaction = create_transaction(Transaction::build(i as f64, date, ""), &conn)
                .expect("Could not create transaction");

            want.push(TableTransaction {
                id: i as TransactionId,
                amount: transaction.amount,
                date: transaction.date,
                description: transaction.description.clone(),
                tag_name: None,
                tag_id: None,
            });
        }

        want.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));

        let window_range = WindowRange {
            start: today - Duration::days(1),
            end: today,
        };
        let got = get_transaction_table_rows_in_range(window_range, SortOrder::Ascending, &conn)
            .expect("Could not query transactions");

        assert_eq!(want.len(), 6, "expected 6 transactions, got {}", want.len());
        assert_eq!(want, got);
    }
}
