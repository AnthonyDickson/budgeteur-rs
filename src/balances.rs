//! Displays accounts and their balances.

use std::sync::{Arc, Mutex};

use askama::Template;
use askama_axum::IntoResponse;
use axum::{
    extract::{FromRef, State},
    response::Response,
};
use rusqlite::Connection;
use time::Date;

use crate::{
    AppState, Error,
    database_id::DatabaseID,
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
};

/// Renders the balances page showing all account balances.
pub async fn get_balances_page(State(state): State<BalanceState>) -> Response {
    let connection = &state
        .db_connection
        .lock()
        .expect("Could not allocate database connection lock");

    let balances = match get_all_balances(connection) {
        Ok(balances) => balances,
        Err(error) => return error.into_response(),
    };

    BalancesTemplate {
        nav_bar: get_nav_bar(endpoints::BALANCES_VIEW),
        balances: &balances,
        import_page_link: endpoints::IMPORT_VIEW,
    }
    .into_response()
}

/// The state needed for the [get_balances_page](crate::balances::get_balances_page) route handler.
#[derive(Debug, Clone)]
pub struct BalanceState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for BalanceState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Renders the balances page.
#[derive(Template)]
#[template(path = "views/balances.html")]
struct BalancesTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    balances: &'a [Balance],
    import_page_link: &'a str,
}

/// The amount of money available for a bank account or credit card.
#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    /// The id for the account balance.
    pub id: DatabaseID,
    /// The account with which to associate the balance.
    pub account: String,
    /// The balance.
    pub balance: f64,
    /// When the balance was updated.
    pub date: Date,
}

pub fn create_balance_table(connection: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS balance (
            id INTEGER PRIMARY KEY,
            account TEXT NOT NULL UNIQUE,
            balance REAL NOT NULL,
            date TEXT NOT NULL
        )",
        (),
    )?;

    Ok(())
}

fn get_all_balances(connection: &Connection) -> Result<Vec<Balance>, Error> {
    connection
        .prepare("SELECT id, account, balance, date FROM balance;")?
        .query_map([], map_row)?
        .map(|maybe_balance| maybe_balance.map_err(|error| error.into()))
        .collect()
}

fn map_row(row: &rusqlite::Row) -> Result<Balance, rusqlite::Error> {
    let id = row.get(0)?;
    let account = row.get(1)?;
    let balance = row.get(2)?;
    let date = row.get(3)?;

    Ok(Balance {
        id,
        account,
        balance,
        date,
    })
}

/// Get the total balance across all accounts.
///
/// # Arguments
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
pub fn get_total_account_balance(connection: &Connection) -> Result<f64, Error> {
    let mut stmt = connection.prepare(
        "SELECT COALESCE(SUM(balance), 0) FROM balance"
    )?;

    let total: f64 = stmt.query_row([], |row| row.get(0))?;
    
    Ok(total)
}

#[cfg(test)]
mod create_balances_table_tests {
    use rusqlite::Connection;

    use super::create_balance_table;

    #[test]
    fn sql_is_valid() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");

        assert_eq!(Ok(()), create_balance_table(&connection));
    }
}

#[cfg(test)]
mod get_all_balances_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::balances::get_all_balances;

    use super::{Balance, create_balance_table};

    #[test]
    fn returns_all_balances() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let want_balances = vec![
            Balance {
                id: 1,
                account: "foo".to_owned(),
                balance: 1.0,
                date: date!(2025 - 07 - 20),
            },
            Balance {
                id: 2,
                account: "bar".to_owned(),
                balance: 1.0,
                date: date!(2025 - 07 - 20),
            },
        ];
        want_balances.iter().for_each(|balance| {
            connection
                .execute(
                    "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
                    (
                        balance.id,
                        &balance.account,
                        balance.balance,
                        balance.date.to_string(),
                    ),
                )
                .unwrap_or_else(|_| {
                    panic!("Could not insert balance {balance:?} into the database")
                });
        });

        let balances = get_all_balances(&connection);

        assert_eq!(Ok(want_balances), balances);
    }

    #[test]
    fn returns_error_on_no_balances() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");

        let balances = get_all_balances(&connection);

        assert_eq!(Ok(vec![]), balances);
    }
}

#[cfg(test)]
mod balances_template_tests {
    use std::iter::zip;

    use askama_axum::IntoResponse;
    use axum::{http::StatusCode, response::Response};
    use scraper::{ElementRef, Html, Selector};
    use time::macros::date;

    use crate::{
        balances::{Balance, BalancesTemplate},
        {endpoints, navigation::get_nav_bar},
    };

    #[tokio::test]
    async fn test_get_balances_view() {
        let want_balance = Balance {
            id: 1,
            account: "1234-5678-9101-12".to_string(),
            balance: 1234.56,
            date: date!(2025 - 05 - 31),
        };
        let balances = vec![want_balance];

        let response = BalancesTemplate {
            nav_bar: get_nav_bar(endpoints::BALANCES_VIEW),
            balances: &balances,
            import_page_link: endpoints::IMPORT_VIEW,
        }
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let html = parse_html(response).await;
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_contains_balances(table, &balances);
    }

    #[tokio::test]
    async fn test_get_balances_view_no_data() {
        let balances = vec![];

        let response = BalancesTemplate {
            nav_bar: get_nav_bar(endpoints::BALANCES_VIEW),
            balances: &balances,
            import_page_link: endpoints::IMPORT_VIEW,
        }
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let html = parse_html(response).await;
        assert_valid_html(&html);
        let paragraph = must_get_no_data_paragraph(&html);
        assert_paragraph_contains_link(paragraph, endpoints::IMPORT_VIEW);
    }

    #[track_caller]
    fn must_get_table(html: &Html) -> ElementRef<'_> {
        let table_selector = Selector::parse("table").unwrap();
        html.select(&table_selector)
            .next()
            .expect("Could not find table in HTML")
    }

    #[track_caller]
    fn must_get_table_rows(table: ElementRef<'_>, want_row_count: usize) -> Vec<ElementRef<'_>> {
        let table_row_selector = Selector::parse("tbody tr").unwrap();
        let table_rows = table.select(&table_row_selector).collect::<Vec<_>>();

        assert_eq!(
            table_rows.len(),
            want_row_count,
            "want {want_row_count} table row, got {}",
            table_rows.len()
        );

        table_rows
    }

    #[track_caller]
    fn assert_table_contains_balances(table: ElementRef<'_>, balances: &[Balance]) {
        let table_rows = must_get_table_rows(table, balances.len());
        let row_header_selector = Selector::parse("th").unwrap();
        let row_cell_selector = Selector::parse("td").unwrap();

        for (row, (table_row, want)) in zip(table_rows, balances).enumerate() {
            let got_account: String = table_row
                .select(&row_header_selector)
                .next()
                .unwrap_or_else(|| panic!("Could not find table header <th> in table row {row}."))
                .text()
                .collect();
            let columns: Vec<ElementRef<'_>> = table_row.select(&row_cell_selector).collect();
            assert_eq!(
                2,
                columns.len(),
                "Want 2 table cells <td> in table row {row}, got {}",
                columns.len()
            );
            let got_balance: String = columns[0].text().collect();
            let got_date: String = columns[1].text().collect();

            assert_eq!(
                want.account, got_account,
                "want account '{}', got '{got_account}'.",
                want.account
            );
            assert_eq!(
                format!("${}", want.balance),
                got_balance,
                "want balance ${}, got {got_balance}.",
                want.balance
            );
            assert_eq!(
                want.date.to_string(),
                got_date,
                "want date {}, got {got_date}",
                want.date
            );
        }
    }

    #[track_caller]
    fn must_get_no_data_paragraph(html: &Html) -> ElementRef<'_> {
        let paragraph_selector = Selector::parse("p.no-data").unwrap();
        html.select(&paragraph_selector)
            .next()
            .expect("Could not find paragraph with class 'no-data' in HTML")
    }
    #[track_caller]
    fn assert_paragraph_contains_link(paragraph: ElementRef<'_>, want_url: &str) {
        let link_selector = Selector::parse("a").unwrap();
        let link = paragraph
            .select(&link_selector)
            .next()
            .expect("Could not find link element in paragraph.");
        let link_target = link
            .attr("href")
            .expect("Link element does define an href attribute.");

        assert_eq!(
            want_url, link_target,
            "want link with href = \"{want_url}\", but got \"{link_target}\""
        );
    }

    #[track_caller]
    fn assert_content_type(response: &Response, content_type: &str) {
        let content_type_header = response
            .headers()
            .get("content-type")
            .expect("content-type header missing");
        assert_eq!(content_type_header, content_type);
    }

    async fn parse_html(response: Response) -> scraper::Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        scraper::Html::parse_document(&text)
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

#[cfg(test)]
mod get_balances_page_tests {
    use std::{
        iter::zip,
        sync::{Arc, Mutex},
    };

    use axum::{extract::State, http::StatusCode, response::Response};
    use rusqlite::Connection;
    use scraper::{ElementRef, Html, Selector};
    use time::macros::date;

    use crate::balances::{Balance, BalanceState, create_balance_table, get_balances_page};

    #[tokio::test]
    async fn test_get_balances_view() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let want_balance = Balance {
            id: 1,
            account: "1234-5678-9101-12".to_string(),
            balance: 1234.56,
            date: date!(2025 - 05 - 31),
        };
        connection
            .execute(
                "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4);",
                (
                    want_balance.id,
                    &want_balance.account,
                    want_balance.balance,
                    want_balance.date,
                ),
            )
            .expect("Could not insert test data into database");
        let balances = vec![want_balance];

        let state = BalanceState {
            db_connection: Arc::new(Mutex::new(connection)),
        };

        let response = get_balances_page(State(state)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let html = parse_html(response).await;
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_contains_balances(table, &balances);
    }

    #[track_caller]
    fn must_get_table(html: &Html) -> ElementRef<'_> {
        let table_selector = Selector::parse("table").unwrap();
        html.select(&table_selector)
            .next()
            .expect("Could not find table in HTML")
    }

    #[track_caller]
    fn must_get_table_rows(table: ElementRef<'_>, want_row_count: usize) -> Vec<ElementRef<'_>> {
        let table_row_selector = Selector::parse("tbody tr").unwrap();
        let table_rows = table.select(&table_row_selector).collect::<Vec<_>>();

        assert_eq!(
            table_rows.len(),
            want_row_count,
            "want {want_row_count} table row, got {}",
            table_rows.len()
        );

        table_rows
    }

    #[track_caller]
    fn assert_table_contains_balances(table: ElementRef<'_>, balances: &[Balance]) {
        let table_rows = must_get_table_rows(table, balances.len());
        let row_header_selector = Selector::parse("th").unwrap();
        let row_cell_selector = Selector::parse("td").unwrap();

        for (row, (table_row, want)) in zip(table_rows, balances).enumerate() {
            let got_account: String = table_row
                .select(&row_header_selector)
                .next()
                .unwrap_or_else(|| panic!("Could not find table header <th> in table row {row}."))
                .text()
                .collect();
            let columns: Vec<ElementRef<'_>> = table_row.select(&row_cell_selector).collect();
            assert_eq!(
                2,
                columns.len(),
                "Want 2 table cells <td> in table row {row}, got {}",
                columns.len()
            );
            let got_balance: String = columns[0].text().collect();
            let got_date: String = columns[1].text().collect();

            assert_eq!(
                want.account, got_account,
                "want account '{}', got '{got_account}'.",
                want.account
            );
            assert_eq!(
                format!("${}", want.balance),
                got_balance,
                "want balance ${}, got {got_balance}.",
                want.balance
            );
            assert_eq!(
                want.date.to_string(),
                got_date,
                "want date {}, got {got_date}",
                want.date
            );
        }
    }

    #[track_caller]
    fn assert_content_type(response: &Response, content_type: &str) {
        let content_type_header = response
            .headers()
            .get("content-type")
            .expect("content-type header missing");
        assert_eq!(content_type_header, content_type);
    }

    async fn parse_html(response: Response) -> scraper::Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        scraper::Html::parse_document(&text)
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

#[cfg(test)]
mod get_total_account_balance_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use super::{create_balance_table, get_total_account_balance};

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        create_balance_table(&conn).unwrap();
        conn
    }

    #[test]
    fn returns_sum_of_all_balances() {
        let conn = get_test_connection();
        
        // Insert test balances
        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (1, "Account 1", 100.50, date!(2024 - 01 - 01).to_string()),
        ).unwrap();
        
        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (2, "Account 2", 250.75, date!(2024 - 01 - 01).to_string()),
        ).unwrap();
        
        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (3, "Account 3", -50.25, date!(2024 - 01 - 01).to_string()),
        ).unwrap();

        let result = get_total_account_balance(&conn).unwrap();
        
        assert_eq!(result, 301.0);
    }

    #[test]
    fn returns_zero_for_no_balances() {
        let conn = get_test_connection();

        let result = get_total_account_balance(&conn).unwrap();
        
        assert_eq!(result, 0.0);
    }

    #[test]
    fn handles_negative_balances() {
        let conn = get_test_connection();
        
        // Insert test balances with negative total
        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (1, "Account 1", -200.0, date!(2024 - 01 - 01).to_string()),
        ).unwrap();
        
        conn.execute(
            "INSERT INTO balance (id, account, balance, date) VALUES (?1, ?2, ?3, ?4)",
            (2, "Account 2", 100.0, date!(2024 - 01 - 01).to_string()),
        ).unwrap();

        let result = get_total_account_balance(&conn).unwrap();
        
        assert_eq!(result, -100.0);
    }
}
