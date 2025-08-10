use std::sync::{Arc, Mutex};

use askama_axum::Template;
use axum::{
    extract::{FromRef, Multipart, State, multipart::Field},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use rusqlite::Connection;

use crate::{
    AppState, Error,
    balances::Balance,
    csv::{ImportBalance, parse_csv},
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
    },
    stores::TransactionStore,
};

/// The state needed for importing transactions.
#[derive(Debug, Clone)]
pub struct ImportState<T>
where
    T: TransactionStore + Send + Sync,
{
    /// The store for managing user [transactions](crate::models::Transaction).
    pub transaction_store: T,
    pub db_connection: Arc<Mutex<Connection>>,
}

impl<T> FromRef<AppState<T>> for ImportState<T>
where
    T: TransactionStore + Clone + Send + Sync,
{
    fn from_ref(state: &AppState<T>) -> Self {
        Self {
            transaction_store: state.transaction_store.clone(),
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Renders the form for creating a category.
#[derive(Template)]
#[template(path = "partials/import_form.html")]
pub struct ImportTransactionFormTemplate<'a> {
    pub import_route: &'a str,
    pub error_message: &'a str,
}

/// Renders the new Category page.
#[derive(Template)]
#[template(path = "views/import.html")]
struct ImportTransactionsTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    form: ImportTransactionFormTemplate<'a>,
}

pub async fn get_import_page() -> Response {
    ImportTransactionsTemplate {
        nav_bar: get_nav_bar(endpoints::IMPORT_VIEW),
        form: ImportTransactionFormTemplate {
            import_route: endpoints::IMPORT,
            error_message: "",
        },
    }
    .into_response()
}

pub async fn import_transactions<T>(
    State(mut state): State<ImportState<T>>,
    mut multipart: Multipart,
) -> Response
where
    T: TransactionStore + Send + Sync,
{
    let mut transactions = Vec::new();
    let mut balances = Vec::new();
    let unexpected_error_response = ImportTransactionFormTemplate {
        import_route: endpoints::IMPORT,
        error_message: "An unexpected error occurred, please try again later.",
    }
    .into_response();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let csv_data = match parse_multipart_field(field).await {
            Ok(data) => data,
            Err(Error::NotCSV) => {
                return ImportTransactionFormTemplate {
                    import_route: endpoints::IMPORT,
                    error_message: "File type must be CSV.",
                }
                .into_response();
            }
            Err(_) => {
                return unexpected_error_response;
            }
        };

        match parse_csv(&csv_data) {
            Ok(parse_result) => {
                transactions.extend(parse_result.transactions);

                if let Some(balance) = parse_result.balance {
                    balances.push(balance);
                }
            }
            Err(e) => {
                tracing::debug!("Failed to parse CSV: {}", e);
                return ImportTransactionFormTemplate {
                    import_route: endpoints::IMPORT,
                    error_message: "Failed to parse CSV, check that the provided file is a valid CSV from ASB or Kiwibank.",
                }
                .into_response();
            }
        }
    }

    if let Err(error) = state.transaction_store.import(transactions) {
        tracing::error!("Failed to import transactions: {}", error);
        return unexpected_error_response;
    }

    for balance in balances {
        if let Err(error) = upsert_balance(&balance, &state.db_connection.lock().unwrap()) {
            tracing::error!("Failed to import account balances: {error:#?}");
            return unexpected_error_response;
        }
    }

    (
        HxRedirect(Uri::from_static(endpoints::TRANSACTIONS_VIEW)),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

async fn parse_multipart_field(field: Field<'_>) -> Result<String, Error> {
    if field.content_type() != Some("text/csv") {
        return Err(Error::NotCSV);
    }

    let file_name = match field.file_name() {
        Some(file_name) => file_name.to_owned(),
        None => {
            tracing::error!("Could not get file name from multipart form field: {field:#?}");
            return Err(Error::MultipartError(
                "Could not get file name from multipart form field".to_owned(),
            ));
        }
    };
    let data = match field.text().await {
        Ok(data) => data.to_owned(),
        Err(error) => {
            tracing::error!("Could not read data from multipart form field: {error}");
            return Err(Error::MultipartError(
                "Could not read data from multipart form field.".to_owned(),
            ));
        }
    };

    tracing::debug!("Received file '{}' that is {} bytes", file_name, data.len());

    Ok(data)
}

fn upsert_balance(
    imported_balance: &ImportBalance,
    connection: &Connection,
) -> Result<Balance, Error> {
    // First, try the upsert with RETURNING
    let maybe_balance = connection
        .prepare(
            "INSERT INTO balance (account, balance, date)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(account) DO UPDATE SET
                 balance = excluded.balance,
                 date = excluded.date
             WHERE excluded.date > balance.date
             RETURNING id, account, balance, date",
        )?
        .query_row(
            (
                &imported_balance.account,
                imported_balance.balance,
                imported_balance.date,
            ),
            map_row_to_balance,
        );

    match maybe_balance {
        Ok(balance) => Ok(balance),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // No rows returned means the WHERE condition wasn't met
            // (trying to insert older data), so fetch the existing record
            connection
                .prepare("SELECT id, account, balance, date FROM balance WHERE account = ?1")?
                .query_row([&imported_balance.account], map_row_to_balance)
                .map_err(Error::from)
        }
        Err(error) => Err(Error::from(error)),
    }
}

fn map_row_to_balance(row: &rusqlite::Row) -> Result<Balance, rusqlite::Error> {
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

#[cfg(test)]
mod upsert_balance_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        balances::{Balance, create_balance_table},
        csv::ImportBalance,
        routes::views::import::upsert_balance,
    };

    #[tokio::test]
    async fn can_upsert_balance() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let want = Balance {
            id: 1,
            account: "1234-5678-9101-012".to_owned(),
            balance: 37_337_252_784.63,
            date: date!(2025 - 05 - 31),
        };

        let got = upsert_balance(
            &ImportBalance {
                account: want.account.clone(),
                balance: want.balance,
                date: want.date,
            },
            &connection,
        )
        .expect("Could not create account balance");

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    /// This test detects a bug with upsert when the same CSV, and therefore the
    /// same balances, are imported twice.
    ///
    /// When using the last inserted row id to fetch the balance row, if there
    /// was a conflict which resulted in no rows being inserted/updated the row
    /// id would either be zero if the database connection was reset, or the
    /// last successfully inserted row otherwise. In the first case, this
    /// resulted in an error 'NotFound: the requested resource could not be
    /// found', and in the second case it would result in an unrelated balance
    /// being returned.
    #[tokio::test]
    async fn upsert_balances_twice() {
        let want = vec![
            Balance {
                id: 1,
                account: "1234-5678-9101-012".to_owned(),
                balance: 123.45,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 2,
                account: "1234-5678-9101-013".to_owned(),
                balance: 234.56,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 3,
                account: "1234-5678-9101-014".to_owned(),
                balance: 345.67,
                date: date!(2025 - 05 - 27),
            },
            Balance {
                id: 4,
                account: "1234-5678-9101-015".to_owned(),
                balance: 567.89,
                date: date!(2025 - 06 - 06),
            },
        ];
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");

        for balance in &want {
            let balance = upsert_balance(
                &ImportBalance {
                    account: balance.account.clone(),
                    balance: balance.balance,
                    date: balance.date,
                },
                &connection,
            )
            .expect("Could not create account balance");
            println!("{balance:#?}");
        }

        let mut got = Vec::new();
        for balance in &want {
            let got_balance = upsert_balance(
                &ImportBalance {
                    account: balance.account.clone(),
                    balance: balance.balance,
                    date: balance.date,
                },
                &connection,
            )
            .expect("Could not create account balance");
            println!("{got_balance:#?}");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn upsert_balance_increments_id() {
        let want = vec![
            Balance {
                id: 1,
                account: "1234-5678-9101-012".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 2,
                account: "2345-6789-1011-123".to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
        ];
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");

        let mut got = Vec::new();

        for balance in &want {
            let got_balance = upsert_balance(
                &ImportBalance {
                    account: balance.account.clone(),
                    balance: balance.balance,
                    date: balance.date,
                },
                &connection,
            )
            .expect("Could not create account balance");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want balance {want:?}, got {got:?}");
    }

    #[tokio::test]
    async fn upsert_takes_balance_with_latest_date() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let account = "1234-5678-9101-112";
        let test_balances = vec![
            // This entry should be accepted in the first upsert
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 73_254.89,
                date: date!(2025 - 05 - 30),
            },
            // This entry should overwrite the balance from the first upsert
            // because it is newer
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            // This entry should be ignored because it is older.
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 2_727_843.43,
                date: date!(2025 - 05 - 29),
            },
        ];
        let want = vec![
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 73_254.89,
                date: date!(2025 - 05 - 30),
            },
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
            Balance {
                id: 1,
                account: account.to_owned(),
                balance: 37_337_252_784.63,
                date: date!(2025 - 05 - 31),
            },
        ];

        let mut got = Vec::new();

        for balance in test_balances {
            let got_balance = upsert_balance(
                &ImportBalance {
                    account: balance.account.clone(),
                    balance: balance.balance,
                    date: balance.date,
                },
                &connection,
            )
            .expect("Could not create account balance");
            got.push(got_balance);
        }

        assert_eq!(want, got, "want left");
    }
}

#[cfg(test)]
mod import_transactions_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        extract::{FromRequest, Multipart, State},
        http::{Request, StatusCode},
        response::Response,
    };
    use rusqlite::Connection;
    use scraper::{ElementRef, Html};
    use time::macros::date;

    use crate::{
        Error,
        balances::{Balance, create_balance_table},
        models::{DatabaseID, Transaction, TransactionBuilder},
        routes::{
            endpoints,
            views::import::{ImportState, get_import_page, import_transactions},
        },
        stores::{TransactionQuery, TransactionStore},
    };

    use super::map_row_to_balance;

    const ASB_BANK_STATEMENT_CSV: &str = "Created date / time : 12 April 2025 / 11:10:19\n\
        Bank 12; Branch 3405; Account 0123456-50 (Streamline)\n\
        From date 20250101\n\
        To date 20250412\n\
        Avail Bal : 1020.00 as of 20250320\n\
        Ledger Balance : 20.00 as of 20250412\n\
        Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount\n\
        \n\
        2025/01/18,2025011801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",1300.00\n\
        2025/01/18,2025011802,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  Credit Card\",-1300.00\n\
        2025/02/18,2025021801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",4400.00\n\
        2025/02/19,2025021901,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-4400.00\n\
        2025/03/20,2025032001,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",2750.00\n\
        2025/03/20,2025032002,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-2750.00";

    const ASB_CC_STATEMENT_CSV: &str = "Created date / time : 12 April 2025 / 11:09:26\n\
        Card Number XXXX-XXXX-XXXX-5023 (Visa Light)\n\
        From date 20250101\n\
        To date 20250412\n\
        Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount\n\
        \n\
        2025/03/20,2025/03/20,2025032002,CREDIT,5023,\"PAYMENT RECEIVED THANK YOU\",-2750.00\n\
        2025/04/09,2025/04/08,2025040902,DEBIT,5023,\"Birdy Bytes\",8.50\n\
        2025/04/10,2025/04/07,2025041001,DEBIT,5023,\"AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)\",10.63\n\
        2025/04/10,2025/04/07,2025041002,DEBIT,5023,\"OFFSHORE SERVICE MARGINS\",0.22\n\
        2025/04/11,2025/04/10,2025041101,DEBIT,5023,\"Buckstars\",11.50";

    const KIWIBANK_BANK_STATEMENT_CSV: &str = "Account number,Date,Memo/Description,Source Code (payment type),TP ref,TP part,TP code,OP ref,OP part,OP code,OP name,OP Bank Account Number,Amount (credit),Amount (debit),Amount,Balance\n\
        38-1234-0123456-01,31-01-2025,INTEREST EARNED ;,,,,,,,,,,0.25,,0.25,71.16\n\
        38-1234-0123456-01,31-01-2025,PIE TAX 10.500% ;,,,,,,,,,,,0.03,-0.03,71.13\n\
        38-1234-0123456-01,28-02-2025,INTEREST EARNED ;,,,,,,,,,,0.22,,0.22,71.35\n\
        38-1234-0123456-01,28-02-2025,PIE TAX 10.500% ;,,,,,,,,,,,0.02,-0.02,71.33\n\
        38-1234-0123456-01,31-03-2025,INTEREST EARNED ;,,,,,,,,,,0.22,,0.22,71.55\n\
        38-1234-0123456-01,31-03-2025,PIE TAX 10.500% ;,,,,,,,,,,,0.02,-0.02,71.53";

    const KIWIBANK_BANK_STATEMENT_SIMPLE_CSV: &str = "47-8115-1482616-00,,,,\n\
            22 Jan 2025,TRANSFER TO A R DICKSON - 01 ;,,-353.46,200.00\n\
            22 Jan 2025,POS W/D LOBSTER SEAFOO-19:47 ;,,-32.00,168.00\n\
            22 Jan 2025,TRANSFER FROM A R DICKSON - 01 ;,,32.00,200.00\n\
            26 Jan 2025,POS W/D BEAUTY CHINA -14:02 ;,,-18.00,182.00\n\
            26 Jan 2025,POS W/D LEE HONG BBQ -14:20 ;,,-60.00,122.00\n\
            26 Jan 2025,TRANSFER FROM A R DICKSON - 01 ;,,78.00,200.00";

    #[tokio::test]
    async fn render_page() {
        let response = get_import_page().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .expect("content-type header missing"),
            "text/html; charset=utf-8"
        );

        let html = parse_html(response, HTMLParsingMode::Document).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_hx_endpoint(&form, endpoints::IMPORT);
        assert_form_enctype(&form, "multipart/form-data");
        assert_form_input(&form, "files", "file");
        assert_form_submit_button(&form);
    }

    #[tokio::test]
    async fn post_multiple_bank_csv() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let state = ImportState {
            db_connection: Arc::new(Mutex::new(connection)),
            transaction_store: FakeTransactionStore::new(),
        };
        let want_transaction_count = 23;

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[
                ASB_BANK_STATEMENT_CSV,
                ASB_CC_STATEMENT_CSV,
                KIWIBANK_BANK_STATEMENT_CSV,
                KIWIBANK_BANK_STATEMENT_SIMPLE_CSV,
            ])
            .await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let create_transaction_calls = state.transaction_store.import_calls.lock().unwrap();
        assert_eq!(
            1,
            create_transaction_calls.len(),
            "want 1 call to import, got {}",
            create_transaction_calls.len()
        );

        // Only check number of transactions imported, the tests for the csv module
        // already checks the contents of imported transactions.
        let got = state.transaction_store.transactions.lock().unwrap().clone();
        assert_eq!(
            want_transaction_count,
            got.len(),
            "want {want_transaction_count} transactions imported, {}",
            got.len()
        );
        assert_hx_redirect(&response, endpoints::TRANSACTIONS_VIEW);
    }

    #[tokio::test]
    async fn extracts_accounts_and_balances_asb_bank_account() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let connection = Arc::new(Mutex::new(connection));
        let state = ImportState {
            db_connection: connection.clone(),
            transaction_store: FakeTransactionStore::new(),
        };
        let want_account = "12-3405-0123456-50 (Streamline)";
        let want_balance = 20.00;
        let want_date = date!(2025 - 04 - 12);

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[ASB_BANK_STATEMENT_CSV]).await,
        )
        .await;

        let balances =
            get_all_balances(&connection.lock().unwrap()).expect("Could not get balances");
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            balances.len(),
            1,
            "want 1 balance, but got {}",
            balances.len()
        );
        let got = &balances[0];
        assert_eq!(want_account, got.account);
        assert_eq!(want_balance, got.balance);
        assert_eq!(want_date, got.date);
    }

    fn get_all_balances(connection: &Connection) -> Result<Vec<Balance>, Error> {
        connection
            .prepare("SELECT id, account, balance, date FROM balance;")?
            .query_map([], map_row_to_balance)?
            .map(|maybe_balance| maybe_balance.map_err(|error| error.into()))
            .collect()
    }

    #[tokio::test]
    async fn does_not_extract_accounts_and_balances_asb_cc_account() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let connection = Arc::new(Mutex::new(connection));
        let state = ImportState {
            db_connection: connection.clone(),
            transaction_store: FakeTransactionStore::new(),
        };

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[ASB_CC_STATEMENT_CSV]).await,
        )
        .await;

        let balances =
            get_all_balances(&connection.lock().unwrap()).expect("Could not get balances");
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            balances.len(),
            0,
            "want 0 balance, but got {}",
            balances.len()
        );
    }
    #[tokio::test]
    async fn extracts_accounts_and_balances_kiwibank_bank_account() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let connection = Arc::new(Mutex::new(connection));
        let state = ImportState {
            db_connection: connection.clone(),
            transaction_store: FakeTransactionStore::new(),
        };
        let want_account = "38-1234-0123456-01";
        let want_balance = 71.53;
        let want_date = date!(2025 - 03 - 31);

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[KIWIBANK_BANK_STATEMENT_CSV]).await,
        )
        .await;

        let balances =
            get_all_balances(&connection.lock().unwrap()).expect("Could not get balances");
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            balances.len(),
            1,
            "want 1 balance, but got {}",
            balances.len()
        );
        let got = &balances[0];
        assert_eq!(want_account, got.account);
        assert_eq!(want_balance, got.balance);
        assert_eq!(want_date, got.date);
    }

    #[tokio::test]
    async fn invalid_csv_renders_error_message() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let connection = Arc::new(Mutex::new(connection));
        let state = ImportState {
            db_connection: connection.clone(),
            transaction_store: FakeTransactionStore::new(),
        };
        let response =
            import_transactions(State(state.clone()), must_make_multipart_csv(&[""]).await).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let create_transaction_calls = state.transaction_store.import_calls.lock().unwrap().len();
        assert_eq!(
            create_transaction_calls, 0,
            "want {} transaction created, got {create_transaction_calls}",
            0
        );

        let html = parse_html(response, HTMLParsingMode::Fragment).await;
        assert_valid_html(&html);
        let form = must_get_form(&html);
        assert_error_message(
            &form,
            "Failed to parse CSV, check that the provided file is a valid CSV from ASB or Kiwibank.",
        );
    }

    #[tokio::test]
    async fn invalid_file_type_renders_error_message() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        create_balance_table(&connection).expect("Could not create balances table");
        let state = ImportState {
            db_connection: Arc::new(Mutex::new(connection)),
            transaction_store: FakeTransactionStore::new(),
        };
        let response = import_transactions(
            State(state.clone()),
            must_make_multipart(&["text/plain"]).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let create_transaction_calls = state.transaction_store.import_calls.lock().unwrap().len();
        assert_eq!(
            create_transaction_calls, 0,
            "want {} transaction created, got {create_transaction_calls}",
            0
        );

        let html = parse_html(response, HTMLParsingMode::Fragment).await;
        assert_valid_html(&html);
        let form = must_get_form(&html);
        assert_error_message(&form, "File type must be CSV.");
    }

    #[tokio::test]
    async fn sql_error_renders_error_message() {
        let connection =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        // Not creating the balances table will create a SQLite error
        // create_balance_table(&connection).expect("Could not create balances table");
        let state = ImportState {
            db_connection: Arc::new(Mutex::new(connection)),
            transaction_store: FakeTransactionStore::new(),
        };

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[ASB_BANK_STATEMENT_CSV]).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_content_type(&response, "text/html; charset=utf-8");
        let html = parse_html(response, HTMLParsingMode::Fragment).await;
        assert_valid_html(&html);
        let form = must_get_form(&html);
        assert_error_message(
            &form,
            "An unexpected error occurred, please try again later.",
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

    #[track_caller]
    fn assert_error_message(form: &ElementRef, want_error_message: &str) {
        let p = form
            .select(&scraper::Selector::parse("p.text-red-500").unwrap())
            .next()
            .expect("No p tag found");
        let error_message = p.text().collect::<String>();
        assert_eq!(want_error_message, error_message.trim());
    }

    async fn must_make_multipart_csv(csv_strings: &[&str]) -> Multipart {
        let boundary = "MY_BOUNDARY123456789";
        let boundary_start = format!("--{boundary}");
        let boundary_end = format!("--{boundary}--");

        let mut lines: Vec<&str> = Vec::new();

        for csv_string in csv_strings {
            lines.push(&boundary_start);
            lines.push("Content-Disposition: form-data; name=\"files\"; filename=\"foobar.CSV\";");
            lines.push("Content-Type: text/csv");
            lines.push("");
            lines.push(csv_string);
        }

        lines.push(&boundary_end);

        let data = lines.join("\r\n").into_bytes();

        let request = Request::builder()
            .method("POST")
            .uri(endpoints::IMPORT)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(data.into())
            .unwrap();

        Multipart::from_request(request, &{}).await.unwrap()
    }
    #[track_caller]
    fn assert_hx_redirect(response: &Response, endpoint: &str) {
        assert_eq!(get_header(response, "hx-redirect"), endpoint,);
    }

    #[track_caller]
    fn get_header(response: &Response, header_name: &str) -> String {
        let header_error_message = format!("Headers missing {header_name}");

        response
            .headers()
            .get(header_name)
            .expect(&header_error_message)
            .to_str()
            .expect("Could not convert to str")
            .to_string()
    }

    async fn must_make_multipart(file_types: &[&str]) -> Multipart {
        let boundary = "MY_BOUNDARY123456789";
        let boundary_start = format!("--{boundary}");
        let boundary_end = format!("--{boundary}--");

        let mut lines: Vec<String> = Vec::new();

        for file_type in file_types {
            lines.push(boundary_start.clone());
            lines.push(
                "Content-Disposition: form-data; name=\"files\"; filename=\"foobar.CSV\";"
                    .to_owned(),
            );
            lines.push(format!("Content-Type: {file_type}"));
            lines.push("".to_owned());
            lines.push("foo".to_owned());
        }

        lines.push(boundary_end);

        let data = lines.join("\r\n").into_bytes();

        let request = Request::builder()
            .method("POST")
            .uri(endpoints::IMPORT)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(data.into())
            .unwrap();

        Multipart::from_request(request, &{}).await.unwrap()
    }

    enum HTMLParsingMode {
        Document,
        Fragment,
    }

    async fn parse_html(response: Response, mode: HTMLParsingMode) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        match mode {
            HTMLParsingMode::Document => Html::parse_document(&text),
            HTMLParsingMode::Fragment => Html::parse_fragment(&text),
        }
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
    fn must_get_form(html: &Html) -> ElementRef {
        html.select(&scraper::Selector::parse("form").unwrap())
            .next()
            .expect("No form found")
    }

    #[track_caller]
    fn assert_hx_endpoint(form: &ElementRef, endpoint: &str) {
        let hx_post = form
            .value()
            .attr("hx-post")
            .expect("hx-post attribute missing");

        assert_eq!(
            hx_post, endpoint,
            "want form with attribute hx-post=\"{endpoint}\", got {hx_post:?}"
        );
        assert_eq!(hx_post, endpoint);
    }

    #[track_caller]
    fn assert_form_enctype(form: &ElementRef, enctype: &str) {
        let form_enctype = form
            .value()
            .attr("enctype")
            .expect("enctype attribute missing");

        assert_eq!(
            form_enctype, enctype,
            "want form with attribute enctype=\"{enctype}\", got {form_enctype:?}"
        );
    }

    #[track_caller]
    fn assert_form_input(form: &ElementRef, name: &str, type_: &str) {
        for input in form.select(&scraper::Selector::parse("input").unwrap()) {
            let input_name = input.value().attr("name").unwrap_or_default();

            if input_name == name {
                let input_type = input.value().attr("type").unwrap_or_default();
                let input_required = input.value().attr("required");
                let input_multiple = input.value().attr("multiple");
                let input_accept = input.value().attr("accept").unwrap_or_default();

                assert_eq!(
                    input_type, type_,
                    "want input with type \"{type_}\", got {input_type:?}"
                );

                assert!(
                    input_required.is_some(),
                    "want input with name {name} to have the required attribute but got none"
                );

                assert!(
                    input_multiple.is_some(),
                    "want input with name {name} to have the multiple attribute but got none"
                );

                assert_eq!(
                    input_accept, "text/csv",
                    "want input with name {name} to have the accept attribute \"text/csv\" but got {input_accept:?}"
                );

                return;
            }
        }

        panic!("No input found with name \"{name}\" and type \"{type_}\"");
    }

    #[track_caller]
    fn assert_form_submit_button(form: &ElementRef) {
        let submit_button = form
            .select(&scraper::Selector::parse("button").unwrap())
            .next()
            .expect("No button found");

        assert_eq!(
            submit_button.value().attr("type").unwrap_or_default(),
            "submit",
            "want submit button with type=\"submit\""
        );
    }

    #[derive(Clone)]
    struct FakeTransactionStore {
        transactions: Arc<Mutex<Vec<Transaction>>>,
        import_calls: Arc<Mutex<Vec<Vec<TransactionBuilder>>>>,
    }

    impl FakeTransactionStore {
        fn new() -> Self {
            Self {
                transactions: Arc::new(Mutex::new(Vec::new())),
                import_calls: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl TransactionStore for FakeTransactionStore {
        fn create(&mut self, amount: f64) -> Result<Transaction, Error> {
            self.create_from_builder(TransactionBuilder::new(amount))
        }

        fn create_from_builder(
            &mut self,
            _builder: TransactionBuilder,
        ) -> Result<Transaction, Error> {
            todo!()
        }

        fn import(&mut self, builders: Vec<TransactionBuilder>) -> Result<Vec<Transaction>, Error> {
            self.import_calls.lock().unwrap().push(builders.clone());

            let next_id = match self.transactions.lock().unwrap().last() {
                Some(transaction) => transaction.id() + 1,
                None => 0,
            };

            let transactions: Vec<Transaction> = builders
                .into_iter()
                .enumerate()
                .map(|(i, builder)| builder.finalise(next_id + i as i64))
                .collect();

            self.transactions
                .lock()
                .unwrap()
                .extend(transactions.clone());

            Ok(transactions)
        }

        fn get(&self, _id: DatabaseID) -> Result<Transaction, Error> {
            todo!()
        }

        fn get_query(&self, _filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            todo!()
        }

        fn count(&self) -> Result<usize, Error> {
            todo!()
        }
    }
}
