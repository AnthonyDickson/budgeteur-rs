//! Displays accounts and their balances.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, State},
    response::{IntoResponse, Response},
};
use maud::{Markup, html};
use rusqlite::Connection;
use time::{Date, format_description::BorrowedFormatItem, macros::format_description};

use crate::{
    AppState, Error,
    endpoints::{self, format_endpoint},
    html::{
        LINK_STYLE, PAGE_CONTAINER_STYLE, TABLE_CELL_STYLE, TABLE_HEADER_STYLE, TABLE_ROW_STYLE,
        base, edit_delete_action_links, format_currency,
    },
    navigation::NavBar,
};

/// The state needed for the [get_accounts_page](crate::account::get_accounts_page) route handler.
#[derive(Debug, Clone)]
pub struct AccountState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for AccountState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// The account data to display in the view
#[derive(Debug, PartialEq)]
struct AccountTableRow {
    name: String,
    balance: f64,
    date: Date,
    edit_url: String,
    delete_url: String,
}

fn accounts_view(accounts: &[AccountTableRow]) -> Markup {
    let create_account_page_url = endpoints::NEW_ACCOUNT_VIEW;
    let nav_bar = NavBar::new(endpoints::ACCOUNTS).into_html();

    let table_row = |account: &AccountTableRow| {
        let balance_str = format_currency(account.balance);
        let datetime = date_datetime_attr(account.date);
        let action_links = edit_delete_action_links(
            &account.edit_url,
            &account.delete_url,
            &format!(
                "Are you sure you want to delete the account '{}'? This cannot be undone.",
                account.name
            ),
            "closest tr",
            "delete",
        );

        html!(
            tr class=(TABLE_ROW_STYLE)
            {
                th
                    scope="row"
                    class="px-6 py-4 font-medium text-gray-900 whitespace-nowrap dark:text-white"
                {
                    (account.name)
                }

                td class="px-6 py-4 text-right"
                {
                    (balance_str)
                }

                td class=(TABLE_CELL_STYLE)
                {
                    time datetime=(datetime) { (account.date) }
                }

                td class=(TABLE_CELL_STYLE)
                {
                    div class="flex gap-4"
                    {
                        (action_links)
                    }
                }
            }
        )
    };

    let content = html!(
        (nav_bar)

        main class=(PAGE_CONTAINER_STYLE)
        {
            section class="space-y-4"
            {
                header class="flex justify-between flex-wrap items-end"
                {
                    h1 class="text-xl font-bold" { "Accounts" }

                    a href=(create_account_page_url) class=(LINK_STYLE)
                    {
                        "Add Account"
                    }
                }

                (accounts_cards_view(accounts, create_account_page_url))

                section class="hidden lg:block w-full overflow-x-auto lg:overflow-visible dark:bg-gray-800 lg:max-w-5xl lg:w-full lg:mx-auto"
                {
                    table class="w-full text-sm text-left rtl:text-right
                        text-gray-500 dark:text-gray-400"
                    {
                        thead class=(TABLE_HEADER_STYLE)
                        {
                            tr
                            {
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Name"
                                }
                                th scope="col" class="px-6 py-3 text-right"
                                {
                                    "Balance"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Date"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Actions"
                                }
                            }
                        }

                        tbody
                        {
                            @for account in accounts {
                                (table_row(account))
                            }

                            @if accounts.is_empty() {
                                tr
                                {
                                    td
                                        colspan="4"
                                        class="px-6 py-4 text-center
                                            text-gray-500 dark:text-gray-400"
                                    {
                                        "No accounts  found. Create an account "
                                        a href=(create_account_page_url) class=(LINK_STYLE)
                                        {
                                            "here"
                                        }
                                        "."
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    );

    base("Accounts", &[], &content)
}

fn accounts_cards_view(accounts: &[AccountTableRow], create_account_page_url: &str) -> Markup {
    html!(
        ul class="lg:hidden space-y-4"
        {
            @for account in accounts {
                li class="rounded border border-gray-200 bg-white px-4 py-3 shadow-sm dark:border-gray-700 dark:bg-gray-800"
                    data-account-card="true"
                {
                    div class="flex items-start justify-between gap-3"
                    {
                        div class="text-sm font-semibold text-gray-900 dark:text-white"
                        { (account.name) }
                        div class="text-sm tabular-nums text-right text-gray-900 dark:text-white"
                        { (format_currency(account.balance)) }
                    }

                    div class="mt-1 text-xs text-gray-500 dark:text-gray-400"
                    { time datetime=(date_datetime_attr(account.date)) { (account.date) } }

                    div class="mt-2 flex items-center gap-4 text-sm"
                    {
                        (edit_delete_action_links(
                            &account.edit_url,
                            &account.delete_url,
                            &format!(
                                "Are you sure you want to delete the account '{}'? This cannot be undone.",
                                account.name
                            ),
                            "closest [data-account-card='true']",
                            "outerHTML",
                        ))
                    }
                }
            }

            @if accounts.is_empty() {
                li class="rounded border border-dashed border-gray-300 bg-white px-4 py-6 text-center text-sm text-gray-500 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-400"
                {
                    "No accounts found. Create an account "
                    a href=(create_account_page_url) class=(LINK_STYLE)
                    {
                        "here"
                    }
                    "."
                }
            }
        }
    )
}

const DATE_ATTRIBUTE_FORMAT: &[BorrowedFormatItem] =
    format_description!("[year]-[month repr:numerical padding:zero]-[day padding:zero]");

fn date_datetime_attr(date: Date) -> String {
    date.format(DATE_ATTRIBUTE_FORMAT)
        .unwrap_or_else(|_| date.to_string())
}

/// Renders the accounts page showing all accounts.
pub async fn get_accounts_page(State(state): State<AccountState>) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let accounts: Vec<AccountTableRow> = get_all_accounts(&connection)
        .inspect_err(|error| tracing::error!("could not get all accounts: {error}"))?;

    Ok(accounts_view(&accounts).into_response())
}

fn get_all_accounts(connection: &Connection) -> Result<Vec<AccountTableRow>, Error> {
    connection
        .prepare("SELECT id, name, balance, date FROM account ORDER BY name ASC;")?
        .query_map([], |row| {
            let id = row.get(0)?;

            Ok(AccountTableRow {
                name: row.get(1)?,
                balance: row.get(2)?,
                date: row.get(3)?,
                edit_url: format_endpoint(endpoints::EDIT_ACCOUNT_VIEW, id),
                delete_url: format_endpoint(endpoints::DELETE_ACCOUNT, id),
            })
        })?
        .map(|account_result| account_result.map_err(Error::from))
        .collect()
}

#[cfg(test)]
mod get_all_accounts_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        account::{
            Account,
            accounts_page::{AccountTableRow, get_all_accounts},
            create_account_table,
        },
        endpoints::{self, format_endpoint},
    };

    #[test]
    fn returns_all_accounts() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_account_table(&connection).expect("Could not create accounts table");
        let accounts = vec![
            Account {
                id: 2,
                name: "bar".to_owned(),
                balance: 1.0,
                date: date!(2025 - 07 - 20),
            },
            Account {
                id: 1,
                name: "foo".to_owned(),
                balance: 1.0,
                date: date!(2025 - 07 - 20),
            },
        ];
        let want_accounts = accounts
            .clone()
            .into_iter()
            .map(
                |Account {
                     id,
                     name: account,
                     balance,
                     date,
                 }| AccountTableRow {
                    name: account,
                    balance,
                    date,
                    edit_url: format_endpoint(endpoints::EDIT_ACCOUNT_VIEW, id),
                    delete_url: format_endpoint(endpoints::DELETE_ACCOUNT, id),
                },
            )
            .collect();
        accounts.iter().for_each(|account| {
            connection
                .execute(
                    "INSERT INTO account (id, name, balance, date) VALUES (?1, ?2, ?3, ?4)",
                    (
                        account.id,
                        &account.name,
                        account.balance,
                        account.date.to_string(),
                    ),
                )
                .unwrap_or_else(|_| {
                    panic!("Could not insert account {account:?} into the database")
                });
        });

        let accounts = get_all_accounts(&connection);

        assert_eq!(Ok(want_accounts), accounts);
    }

    #[test]
    fn returns_error_on_no_accounts() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_account_table(&connection).expect("Could not create accounts table");

        let accounts = get_all_accounts(&connection);

        assert_eq!(Ok(vec![]), accounts);
    }
}

#[cfg(test)]
mod accounts_template_tests {
    use std::iter::zip;

    use scraper::{ElementRef, Html, Selector};
    use time::macros::date;

    use crate::{
        account::{
            Account,
            accounts_page::{AccountTableRow, accounts_view},
        },
        endpoints::{self, format_endpoint},
        html::format_currency,
        test_utils::assert_valid_html,
    };

    #[test]
    fn test_get_accounts_view() {
        let want_account = Account {
            id: 1,
            name: "1234-5678-9101-12".to_string(),
            balance: 1234.56,
            date: date!(2025 - 05 - 31),
        };
        let accounts = vec![AccountTableRow {
            name: want_account.name,
            balance: want_account.balance,
            date: want_account.date,
            edit_url: format_endpoint(endpoints::EDIT_ACCOUNT_VIEW, want_account.id),
            delete_url: format_endpoint(endpoints::DELETE_ACCOUNT, want_account.id),
        }];

        let rendered_template = accounts_view(&accounts).into_string();

        let html = scraper::Html::parse_document(&rendered_template);
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_contains_accounts(table, &accounts);
    }

    #[test]
    fn test_no_data() {
        let accounts = vec![];

        let rendered_template = accounts_view(&accounts).into_string();

        let html = Html::parse_document(&rendered_template);
        assert_valid_html(&html);
        let paragraph = must_get_no_data_paragraph(&html);
        assert_paragraph_contains_link(paragraph, endpoints::NEW_ACCOUNT_VIEW);
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
    fn assert_table_contains_accounts(table: ElementRef<'_>, accounts: &[AccountTableRow]) {
        let table_rows = must_get_table_rows(table, accounts.len());
        let row_header_selector = Selector::parse("th").unwrap();
        let row_cell_selector = Selector::parse("td").unwrap();
        let button_selector = Selector::parse("button").unwrap();

        for (row, (table_row, want)) in zip(table_rows, accounts).enumerate() {
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
            let got_acconunt: String = columns[0].text().collect::<String>().trim().to_string();
            let got_date: String = columns[1].text().collect::<String>().trim().to_string();

            assert_eq!(
                want.name, got_account,
                "want account '{}', got '{got_account}'.",
                want.name
            );
            let want_balance = format_currency(want.balance);
            assert_eq!(
                want_balance, got_acconunt,
                "want balance {want_balance}, got {got_acconunt}."
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
        let paragraph_selector = Selector::parse("td[colspan='4']").unwrap();
        html.select(&paragraph_selector)
            .next()
            .expect("Could not find table cell with colspan='4' in HTML")
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

    // Shared helpers live in crate::test_utils.
}

#[cfg(test)]
mod get_accounts_page_tests {
    use std::{
        iter::zip,
        sync::{Arc, Mutex},
    };

    use axum::{extract::State, http::StatusCode};
    use rusqlite::Connection;
    use scraper::{ElementRef, Html, Selector};
    use time::macros::date;

    use crate::{
        account::{
            Account,
            accounts_page::{AccountState, AccountTableRow},
            create_account_table, get_accounts_page,
        },
        endpoints::{self, format_endpoint},
        html::format_currency,
        test_utils::{assert_content_type, assert_valid_html, parse_html_document},
    };

    #[tokio::test]
    async fn test_get_accounts_view() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_account_table(&connection).expect("Could not create accounts table");
        let want_account = Account {
            id: 1,
            name: "1234-5678-9101-12".to_string(),
            balance: 1234.56,
            date: date!(2025 - 05 - 31),
        };
        connection
            .execute(
                "INSERT INTO account (id, name, balance, date) VALUES (?1, ?2, ?3, ?4);",
                (
                    want_account.id,
                    &want_account.name,
                    want_account.balance,
                    want_account.date,
                ),
            )
            .expect("Could not insert test data into database");
        let accounts = vec![AccountTableRow {
            name: want_account.name,
            balance: want_account.balance,
            date: want_account.date,
            edit_url: format_endpoint(endpoints::EDIT_ACCOUNT_VIEW, want_account.id),
            delete_url: format_endpoint(endpoints::DELETE_ACCOUNT, want_account.id),
        }];

        let state = AccountState {
            db_connection: Arc::new(Mutex::new(connection)),
        };

        let response = get_accounts_page(State(state)).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let html = parse_html_document(response).await;
        assert_valid_html(&html);
        let table = must_get_table(&html);
        assert_table_contains_accounts(table, &accounts);
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
    fn assert_table_contains_accounts(table: ElementRef<'_>, accounts: &[AccountTableRow]) {
        let table_rows = must_get_table_rows(table, accounts.len());
        let row_header_selector = Selector::parse("th").unwrap();
        let row_cell_selector = Selector::parse("td").unwrap();
        let button_selector = Selector::parse("button").unwrap();

        for (row, (table_row, want)) in zip(table_rows, accounts).enumerate() {
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
                want.name, got_account,
                "want account '{}', got '{got_account}'.",
                want.name
            );
            let want_balance = format_currency(want.balance);
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

    // Shared helpers live in crate::test_utils.
}
