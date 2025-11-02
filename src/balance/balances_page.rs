//! Displays accounts and their balances.

use std::sync::{Arc, Mutex};

use askama::Template;
use axum::{
    extract::{FromRef, State},
    http::StatusCode,
    response::Response,
};
use rusqlite::Connection;
use time::Date;

use crate::{
    AppState, Error,
    endpoints::{self, format_endpoint},
    filters,
    internal_server_error::render_internal_server_error,
    navigation::{NavbarTemplate, get_nav_bar},
    shared_templates::render,
};

/// The state needed for the [get_balances_page](crate::balance::get_balances_page) route handler.
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

/// The account data to display in the view
#[derive(Debug, PartialEq)]
struct AccountTableRow {
    account: String,
    balance: f64,
    date: Date,
    edit_url: String,
    delete_url: String,
}

/// Renders the balances page.
#[derive(Template)]
#[template(path = "views/balance/balances.html")]
struct BalancesTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    balances: &'a [AccountTableRow],
    create_account_balance_page_url: &'a str,
}

/// Renders the balances page showing all account balances.
pub async fn get_balances_page(State(state): State<BalanceState>) -> Response {
    let connection = &state
        .db_connection
        .lock()
        .expect("Could not allocate database connection lock");

    let balances: Vec<AccountTableRow> = match get_all_balances(connection) {
        Ok(balances) => balances,
        Err(error) => {
            tracing::error!("{error}");
            return render_internal_server_error(Default::default());
        }
    };

    let template = BalancesTemplate {
        nav_bar: get_nav_bar(endpoints::BALANCES),
        balances: &balances,
        create_account_balance_page_url: endpoints::NEW_BALANCE_VIEW,
    };

    render(StatusCode::OK, template)
}

fn get_all_balances(connection: &Connection) -> Result<Vec<AccountTableRow>, Error> {
    connection
        .prepare("SELECT id, account, balance, date FROM balance ORDER BY account ASC;")?
        .query_map([], |row| {
            let id = row.get(0)?;

            Ok(AccountTableRow {
                account: row.get(1)?,
                balance: row.get(2)?,
                date: row.get(3)?,
                edit_url: format_endpoint(endpoints::EDIT_BALANCE_VIEW, id),
                delete_url: format_endpoint(endpoints::DELETE_BALANCE, id),
            })
        })?
        .map(|maybe_balance| maybe_balance.map_err(Error::from))
        .collect()
}

#[cfg(test)]
mod get_all_balances_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        balance::{
            Balance,
            balances_page::{AccountTableRow, get_all_balances},
            create_balance_table,
        },
        endpoints::{self, format_endpoint},
    };

    #[test]
    fn returns_all_balances() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let balances = vec![
            Balance {
                id: 2,
                account: "bar".to_owned(),
                balance: 1.0,
                date: date!(2025 - 07 - 20),
            },
            Balance {
                id: 1,
                account: "foo".to_owned(),
                balance: 1.0,
                date: date!(2025 - 07 - 20),
            },
        ];
        let want_balances = balances
            .clone()
            .into_iter()
            .map(
                |Balance {
                     id,
                     account,
                     balance,
                     date,
                 }| AccountTableRow {
                    account,
                    balance,
                    date,
                    edit_url: format_endpoint(endpoints::EDIT_BALANCE_VIEW, id),
                    delete_url: format_endpoint(endpoints::DELETE_BALANCE, id),
                },
            )
            .collect();
        balances.iter().for_each(|balance| {
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

    use askama::Template;
    use scraper::{ElementRef, Html, Selector};
    use time::macros::date;

    use crate::{
        balance::{
            Balance,
            balances_page::{AccountTableRow, BalancesTemplate},
        },
        endpoints::{self, format_endpoint},
        filters::currency,
        navigation::get_nav_bar,
    };

    #[test]
    fn test_get_balances_view() {
        let want_balance = Balance {
            id: 1,
            account: "1234-5678-9101-12".to_string(),
            balance: 1234.56,
            date: date!(2025 - 05 - 31),
        };
        let balances = vec![AccountTableRow {
            account: want_balance.account,
            balance: want_balance.balance,
            date: want_balance.date,
            edit_url: format_endpoint(endpoints::EDIT_BALANCE_VIEW, want_balance.id),
            delete_url: format_endpoint(endpoints::DELETE_BALANCE, want_balance.id),
        }];

        let rendered_template = BalancesTemplate {
            nav_bar: get_nav_bar(endpoints::BALANCES),
            balances: &balances,
            create_account_balance_page_url: endpoints::NEW_BALANCE_VIEW,
        }
        .render()
        .expect("Could not render template");

        let html = scraper::Html::parse_document(&rendered_template);
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_contains_balances(table, &balances);
    }

    #[test]
    fn test_get_balances_view_no_data() {
        let balances = vec![];

        let rendered_template = BalancesTemplate {
            nav_bar: get_nav_bar(endpoints::BALANCES),
            balances: &balances,
            create_account_balance_page_url: endpoints::NEW_BALANCE_VIEW,
        }
        .render()
        .expect("Could not render template");

        let html = Html::parse_document(&rendered_template);
        assert_valid_html(&html);
        let paragraph = must_get_no_data_paragraph(&html);
        assert_paragraph_contains_link(paragraph, endpoints::NEW_BALANCE_VIEW);
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
    fn assert_table_contains_balances(table: ElementRef<'_>, balances: &[AccountTableRow]) {
        let table_rows = must_get_table_rows(table, balances.len());
        let row_header_selector = Selector::parse("th").unwrap();
        let row_cell_selector = Selector::parse("td").unwrap();
        let button_selector = Selector::parse("button").unwrap();

        for (row, (table_row, want)) in zip(table_rows, balances).enumerate() {
            let got_account: String = table_row
                .select(&row_header_selector)
                .next()
                .unwrap_or_else(|| panic!("Could not find table header <th> in table row {row}."))
                .text()
                .collect::<String>()
                .trim()
                .to_string();
            let columns: Vec<ElementRef<'_>> = table_row.select(&row_cell_selector).collect();
            assert_eq!(
                3,
                columns.len(),
                "Want 3 table cells <td> in table row {row}, got {}",
                columns.len()
            );
            let got_balance: String = columns[0].text().collect::<String>().trim().to_string();
            let got_date: String = columns[1].text().collect::<String>().trim().to_string();

            assert_eq!(
                want.account, got_account,
                "want account '{}', got '{got_account}'.",
                want.account
            );
            let want_balance = currency(want.balance, &()).unwrap();
            assert_eq!(
                want_balance, got_balance,
                "want balance {want_balance}, got {got_balance}."
            );
            assert_eq!(
                want.date.to_string(),
                got_date,
                "want date {}, got {got_date}",
                want.date
            );

            // Check delete URL
            let got_actions: Vec<ElementRef<'_>> = columns[2].select(&button_selector).collect();
            assert_eq!(
                1,
                got_actions.len(),
                "Want 1 delete button per table row, got {} for table row {row}",
                got_actions.len()
            );
            let got_delete_url = got_actions[0].attr("hx-delete").unwrap_or_else(|| {
                panic!("hx-delete attribute not set for button in table row {row}")
            });
            assert_eq!(
                want.delete_url, got_delete_url,
                "want edit URL {}, got {got_delete_url}",
                want.delete_url
            );
        }
    }

    #[track_caller]
    fn must_get_no_data_paragraph(html: &Html) -> ElementRef<'_> {
        let paragraph_selector = Selector::parse("td[colspan='3']").unwrap();
        html.select(&paragraph_selector)
            .next()
            .expect("Could not find table cell with colspan='3' in HTML")
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

    use crate::{
        balance::{
            Balance,
            balances_page::{AccountTableRow, BalanceState},
            create_balance_table, get_balances_page,
        },
        endpoints::{self, format_endpoint},
        filters::currency,
    };

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
        let balances = vec![AccountTableRow {
            account: want_balance.account,
            balance: want_balance.balance,
            date: want_balance.date,
            edit_url: format_endpoint(endpoints::EDIT_BALANCE_VIEW, want_balance.id),
            delete_url: format_endpoint(endpoints::DELETE_BALANCE, want_balance.id),
        }];

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
    fn assert_table_contains_balances(table: ElementRef<'_>, balances: &[AccountTableRow]) {
        let table_rows = must_get_table_rows(table, balances.len());
        let row_header_selector = Selector::parse("th").unwrap();
        let row_cell_selector = Selector::parse("td").unwrap();
        let button_selector = Selector::parse("button").unwrap();

        for (row, (table_row, want)) in zip(table_rows, balances).enumerate() {
            let got_account: String = table_row
                .select(&row_header_selector)
                .next()
                .unwrap_or_else(|| panic!("Could not find table header <th> in table row {row}."))
                .text()
                .collect::<String>()
                .trim()
                .to_string();
            let columns: Vec<ElementRef<'_>> = table_row.select(&row_cell_selector).collect();
            assert_eq!(
                3,
                columns.len(),
                "Want 3 table cells <td> in table row {row}, got {}",
                columns.len()
            );
            let got_balance: String = columns[0].text().collect::<String>().trim().to_string();
            let got_date: String = columns[1].text().collect::<String>().trim().to_string();

            assert_eq!(
                want.account, got_account,
                "want account '{}', got '{got_account}'.",
                want.account
            );
            let want_balance = currency(want.balance, &()).unwrap();
            assert_eq!(
                want_balance, got_balance,
                "want balance {want_balance}, got {got_balance}."
            );
            assert_eq!(
                want.date.to_string(),
                got_date,
                "want date {}, got {got_date}",
                want.date
            );

            // Check delete URL
            let got_actions: Vec<ElementRef<'_>> = columns[2].select(&button_selector).collect();
            assert_eq!(
                1,
                got_actions.len(),
                "Want 1 delete button per table row, got {} for table row {row}",
                got_actions.len()
            );
            let got_delete_url = got_actions[0].attr("hx-delete").unwrap_or_else(|| {
                panic!("hx-delete attribute not set for button in table row {row}")
            });
            assert_eq!(
                want.delete_url, got_delete_url,
                "want edit URL {}, got {got_delete_url}",
                want.delete_url
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
