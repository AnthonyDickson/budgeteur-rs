use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Multipart, State, multipart::Field},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rusqlite::Connection;

use crate::{
    AppState, Error,
    alert::Alert,
    csv_import::{
        account::upsert_account,
        alert::{error_with_partial_success, success_with_tagging},
        csv::parse_csv,
    },
    rule::{TaggingMode, TaggingResult, apply_rules_to_transactions},
    transaction::{Transaction, TransactionBuilder, map_transaction_row},
};

/// The state needed for importing transactions.
#[derive(Debug, Clone)]
pub struct ImportState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for ImportState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Route handler for importing transactions from CSV files.
///
/// This function processes uploaded CSV files, imports transactions and accounts,
/// and applies auto-tagging rules to the newly imported transactions.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn import_transactions(
    State(state): State<ImportState>,
    mut multipart: Multipart,
) -> Result<Response, Response> {
    let start_time = std::time::Instant::now();
    let mut transactions = Vec::new();
    let mut accounts = Vec::new();

    while let Some(field) = multipart.next_field().await.unwrap() {
        let csv_data = parse_multipart_field(field)
            .await
            .map_err(|error| match error {
                Error::NotCSV => (
                    StatusCode::BAD_REQUEST,
                    Alert::ErrorSimple {
                        message: "File type must be CSV.".to_owned(),
                    }
                    .into_html(),
                )
                    .into_response(),
                error => {
                    tracing::error!("Failed to parse multipart field: {}", error);
                    error.into_alert_response()
                }
            })?;

        parse_csv(&csv_data)
            .map(|parse_result| {
                transactions.extend(parse_result.transactions);

                if let Some(account) = parse_result.account {
                    accounts.push(account);
                }
            })
            .inspect_err(|error| tracing::debug!("Failed to parse CSV: {}", error))
            .map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    Alert::Error {
                        message: "Failed to parse CSV".to_owned(),
                        details:
                            "Check that the provided file is a valid CSV from ASB or Kiwibank."
                                .to_owned(),
                    }
                    .into_html(),
                )
                    .into_response()
            })?;
    }

    let connection = state.db_connection.lock().map_err(|error| {
        tracing::error!("could not acquire database lock: {error}");
        Error::DatabaseLockError.into_alert_response()
    })?;

    let tx = connection
        .unchecked_transaction()
        .inspect_err(|error| tracing::error!("could not start transaction: {error}"))
        .map_err(|_| {
            Alert::ErrorSimple {
                message: "Could not import transactions".to_owned(),
            }
            .into_response()
        })?;

    let imported_transactions = import_transaction_list(transactions, &tx)
        .inspect_err(|error| tracing::error!("Failed to import transactions: {}", error))
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Alert::Error {
                    message: "Import failed".to_owned(),
                    details: "An unexpected error occurred, please try again later".to_owned(),
                }
                .into_html(),
            )
                .into_response()
        })?;

    // Apply auto-tagging to the imported transactions
    let auto_tagging_result = if !imported_transactions.is_empty() {
        apply_rules_to_transactions(TaggingMode::FromArgs(&imported_transactions), &tx)
    } else {
        Ok(TaggingResult::empty())
    };

    for account in accounts {
        upsert_account(&account, &tx).map_err(|error| {
            let duration = start_time.elapsed();
            tracing::error!(
                "Failed to import accounts after {:.1}ms: {error:#?}",
                duration.as_millis()
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Alert::Error {
                    message: "Account import failed".to_owned(),
                    details: format!(
                        "Transactions were imported but accounts could not be updated after {:.1}ms.",
                        duration.as_millis() ),
                }
                .into_html(),
            )
                .into_response()
        })?;
    }

    tx.commit()
        .inspect_err(|error| tracing::error!("could not commit transaction: {error}"))
        .map_err(|_| {
            Alert::ErrorSimple {
                message: "Could not import transactions".to_owned(),
            }
            .into_response()
        })?;

    let duration = start_time.elapsed();

    // Generate success/error message based on auto-tagging result
    auto_tagging_result
        .map(|tagging_result| {
            let alert =
                success_with_tagging(&tagging_result, imported_transactions.len(), duration);
            (StatusCode::CREATED, alert.into_html()).into_response()
        })
        .map_err(|error| {
            let alert = error_with_partial_success(
                &error.to_string(),
                imported_transactions.len(),
                duration,
            );
            (StatusCode::CREATED, alert.into_html()).into_response()
        })
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

/// Import many transactions from a CSV file.
///
/// Ignores transactions with import IDs that already exist in the database.
///
/// **Note**: If you want transactional integrity (all or nothing), pass in a
/// transaction for `connection`.
///
/// # Errors
/// Returns an [Error::SqlError] if there is an unexpected SQL error.
fn import_transaction_list(
    builders: Vec<TransactionBuilder>,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
    let mut imported_transactions = Vec::new();

    // Prepare the insert statement once for reuse
    let mut stmt = connection.prepare(
        "INSERT INTO \"transaction\" (amount, date, description, import_id, tag_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(import_id) DO NOTHING
         RETURNING id, amount, date, description, import_id, tag_id",
    )?;

    for builder in builders {
        // Try to insert and get the result
        let transaction_result = stmt.query_row(
            (
                builder.amount,
                builder.date,
                builder.description,
                builder.import_id,
                builder.tag_id,
            ),
            map_transaction_row,
        );

        // Only collect successfully inserted transactions (not conflicts)
        if let Ok(transaction) = transaction_result {
            imported_transactions.push(transaction);
        }
    }

    Ok(imported_transactions)
}

#[cfg(test)]
mod import_transactions_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        body,
        extract::{FromRequest, Multipart, State},
        http::{Request, StatusCode},
        response::Response,
    };
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        Error,
        account::{Account, map_row_to_account},
        csv_import::import_transactions::{ImportState, import_transactions},
        db::initialize,
        endpoints,
        rule::create_rule,
        tag::{TagId, TagName, create_tag},
        test_utils::{assert_content_type, assert_valid_html, parse_html_fragment},
        transaction::count_transactions,
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

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

    const KIWIBANK_BANK_STATEMENT_CSV: &str = "\
        Account number,Effective Date,Transaction Date,Description,Transaction Code,\
        Particulars,Code,Reference,Other Party Name,Other Party Account Number,\
        Other Party Particulars,Other Party Code,Other Party Reference,Amount,Balance\n\
        38-8106-0601663-00,2025-08-21,2025-08-22,Sushi,EFTPOS PURCHASE,,,,,,,,,-9.00,895.69\n\
        38-8106-0601663-00,2025-08-22,2025-08-22,PAY Alice The Bar Drinks Bob,\
        DIRECT DEBIT,The Bar,Drinks,Bob,Alice,01-2345-1080543-00,The Bar,Drinks,Bob,-15.00,880.69\n\
        38-8106-0601663-00,2025-08-23,2025-08-23,PAY Alice Pool Bob,\
        DIRECT DEBIT,Pool,,Bob,Alice,01-2345-1080543-00,Pool,,Bob,-3.15,877.54\n\
        38-8106-0601663-00,2025-08-23,2025-08-23,PAK N SAVE SYLVIA PARK AUCKLAND,EFTPOS PURCHASE,,,,,,,,,-42.02,835.52";

    #[tokio::test]
    async fn post_multiple_bank_csv() {
        let conn = get_test_connection();
        let state = ImportState {
            db_connection: Arc::new(Mutex::new(conn)),
        };
        let want_transaction_count = 15;

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[
                ASB_BANK_STATEMENT_CSV,
                ASB_CC_STATEMENT_CSV,
                KIWIBANK_BANK_STATEMENT_CSV,
            ])
            .await,
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        // Check the number of transactions imported by querying the database
        let transaction_count = {
            let connection = state.db_connection.lock().unwrap();
            count_transactions(&connection).expect("Could not count transactions")
        };
        assert_eq!(
            want_transaction_count, transaction_count,
            "want {want_transaction_count} transactions imported, got {transaction_count}"
        );

        // Validate success alert message
        assert_alert_success_message(response, "Import completed successfully!").await;
    }

    #[tokio::test]
    async fn extracts_accounts_and_balances_asb_bank_account() {
        let conn = get_test_connection();
        let connection = Arc::new(Mutex::new(conn));
        let state = ImportState {
            db_connection: connection.clone(),
        };
        let want_account = "12-3405-0123456-50 (Streamline)";
        let want_balance = 20.00;
        let want_date = date!(2025 - 04 - 12);

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[ASB_BANK_STATEMENT_CSV]).await,
        )
        .await
        .unwrap();

        let accounts =
            get_all_accounts(&connection.lock().unwrap()).expect("Could not get balances");
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            accounts.len(),
            1,
            "want 1 balance, but got {}",
            accounts.len()
        );
        let got = &accounts[0];
        assert_eq!(want_account, got.name);
        assert_eq!(want_balance, got.balance);
        assert_eq!(want_date, got.date);

        // Validate success alert message
        assert_alert_success_message(response, "Import completed successfully!").await;
    }

    fn get_all_accounts(connection: &Connection) -> Result<Vec<Account>, Error> {
        connection
            .prepare("SELECT id, name, balance, date FROM account;")?
            .query_map([], map_row_to_account)?
            .map(|maybe_account| maybe_account.map_err(|error| error.into()))
            .collect()
    }

    #[tokio::test]
    async fn does_not_extract_accounts_and_balances_asb_cc_account() {
        let conn = get_test_connection();
        let connection = Arc::new(Mutex::new(conn));
        let state = ImportState {
            db_connection: connection.clone(),
        };

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[ASB_CC_STATEMENT_CSV]).await,
        )
        .await
        .unwrap();

        let accounts =
            get_all_accounts(&connection.lock().unwrap()).expect("Could not get accounts");
        assert_eq!(response.status(), StatusCode::CREATED);
        assert_eq!(
            accounts.len(),
            0,
            "want 0 accounts, but got {}",
            accounts.len()
        );

        // Validate success alert message
        assert_alert_success_message(response, "Import completed successfully!").await;
    }

    #[tokio::test]
    async fn invalid_csv_renders_error_message() {
        let conn = get_test_connection();
        let connection = Arc::new(Mutex::new(conn));
        let state = ImportState {
            db_connection: connection.clone(),
        };
        let response =
            import_transactions(State(state.clone()), must_make_multipart_csv(&[""]).await)
                .await
                .unwrap_err();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_content_type(&response, "text/html; charset=utf-8");

        // Check that no transactions were created
        let transaction_count = {
            let connection = state.db_connection.lock().unwrap();
            count_transactions(&connection).expect("Could not count transactions")
        };
        assert_eq!(
            transaction_count, 0,
            "want 0 transactions created, got {transaction_count}"
        );

        // Validate alert message
        assert_alert_error_message(response, "Failed to parse CSV").await;
    }

    #[tokio::test]
    async fn invalid_file_type_renders_error_message() {
        let conn = get_test_connection();
        let state = ImportState {
            db_connection: Arc::new(Mutex::new(conn)),
        };
        let response = import_transactions(
            State(state.clone()),
            must_make_multipart(&["text/plain"]).await,
        )
        .await
        .unwrap_err();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_content_type(&response, "text/html; charset=utf-8");

        // Check that no transactions were created
        let transaction_count = {
            let connection = state.db_connection.lock().unwrap();
            count_transactions(&connection).expect("Could not count transactions")
        };
        assert_eq!(
            transaction_count, 0,
            "want 0 transactions created, got {transaction_count}"
        );

        // Validate alert message
        assert_alert_error_message(response, "File type must be CSV.").await;
    }

    #[tokio::test]
    async fn sql_error_renders_error_message() {
        // Create a connection without initializing the database tables to trigger SQL errors
        let conn =
            Connection::open_in_memory().expect("Could not initialise in-memory SQLite database");
        let state = ImportState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[ASB_BANK_STATEMENT_CSV]).await,
        )
        .await
        .unwrap_err();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_content_type(&response, "text/html; charset=utf-8");

        // Validate alert message
        assert_alert_error_message(response, "Import failed").await;
    }

    async fn assert_alert_error_message(response: Response, expected_message: &str) {
        let html = parse_html_fragment(response).await;
        assert_valid_html(&html);

        let alert_container = html
            .select(&scraper::Selector::parse("#alert-container").unwrap())
            .next()
            .expect("No alert container found");

        let message_p = alert_container
            .select(&scraper::Selector::parse("p.text-sm.font-medium").unwrap())
            .next()
            .expect("No alert message found");

        let message = message_p.text().collect::<String>();
        assert_eq!(message.trim(), expected_message);
    }

    async fn assert_alert_success_message(response: Response, expected_message: &str) {
        let html = parse_html_fragment(response).await;
        assert_valid_html(&html);

        let alert_container = html
            .select(&scraper::Selector::parse("#alert-container").unwrap())
            .next()
            .expect("No alert container found");

        let message_p = alert_container
            .select(&scraper::Selector::parse("p.text-sm.font-medium").unwrap())
            .next()
            .expect("No alert message found");

        let message = message_p.text().collect::<String>();
        assert_eq!(message.trim(), expected_message);
    }

    async fn assert_alert_success_with_details(
        response: Response,
        expected_message: &str,
        expected_details_contains: &str,
    ) {
        let html = parse_html_fragment(response).await;
        assert_valid_html(&html);

        let alert_container = html
            .select(&scraper::Selector::parse("#alert-container").unwrap())
            .next()
            .expect("No alert container found");

        let message_p = alert_container
            .select(&scraper::Selector::parse("p.text-sm.font-medium").unwrap())
            .next()
            .expect("No alert message found");

        let message = message_p.text().collect::<String>();
        assert_eq!(message.trim(), expected_message);

        let details_p = alert_container
            .select(&scraper::Selector::parse("p.mt-1.text-sm.opacity-80").unwrap())
            .next()
            .expect("No alert details found");

        let details = details_p.text().collect::<String>();
        assert!(
            details.contains(expected_details_contains),
            "Expected details to contain '{}', but got: '{}'",
            expected_details_contains,
            details.trim()
        );
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

    // Auto-tagging integration tests

    // CSV with transactions that will match auto-tagging rules
    const AUTO_TAG_TEST_CSV: &str = "\
            Account number,Effective Date,Transaction Date,Description,Transaction Code,\
        Particulars,Code,Reference,Other Party Name,Other Party Account Number,\
        Other Party Particulars,Other Party Code,Other Party Reference,Amount,Balance\n\
        38-8106-0601663-00,2025-01-15,2025-01-15,Buckstars Coffee Shop,EFTPOS PURCHASE,,,,,,,,,-5.50,100.00\n\
        38-8106-0601663-00,2025-01-15,2025-01-15,Supermarket Groceries,EFTPOS PURCHASE,,,,,,,,,-45.20,54.80\n\
        38-8106-0601663-00,2025-01-15,2025-01-15,Amazon Prime Subscription,EFTPOS PURCHASE,,,,,,,,,-12.99,41.81\n\
        38-8106-0601663-00,2025-01-15,2025-01-15,Shell Gas Station,EFTPOS PURCHASE,,,,,,,,,-35.00,6.81\n\
        38-8106-0601663-00,2025-01-15,2025-01-15,Random Transaction,EFTPOS PURCHASE,,,,,,,,,-25.00,-18.19";

    #[tokio::test]
    async fn import_with_auto_tagging_success() {
        let conn = get_test_connection();
        let connection = Arc::new(Mutex::new(conn));
        let state = ImportState {
            db_connection: connection.clone(),
        };

        // Create tags and rules for auto-tagging
        {
            let conn = connection.lock().unwrap();
            let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &conn).unwrap();
            let grocery_tag = create_tag(TagName::new_unchecked("Groceries"), &conn).unwrap();

            create_rule("Buckstars", coffee_tag.id, &conn).unwrap();
            create_rule("Supermarket", grocery_tag.id, &conn).unwrap();
        }

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[AUTO_TAG_TEST_CSV]).await,
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        // Check that transactions were imported
        let transaction_count = {
            let connection_guard = state.db_connection.lock().unwrap();
            count_transactions(&connection_guard).expect("Could not count transactions")
        };
        assert_eq!(
            transaction_count, 5,
            "Expected 5 transactions to be imported"
        );

        // Validate that auto-tagging was applied
        assert_alert_success_with_details(
            response,
            "Import completed successfully!",
            "applied 2 tags automatically",
        )
        .await;
    }

    #[tokio::test]
    async fn import_with_no_matching_rules() {
        let conn = get_test_connection();
        let connection = Arc::new(Mutex::new(conn));
        let state = ImportState {
            db_connection: connection.clone(),
        };

        // Create tags but no rules that match the imported transactions
        {
            let conn = connection.lock().unwrap();
            let _unmatched_tag = create_tag(TagName::new_unchecked("Unmatched"), &conn).unwrap();
            create_rule("NoMatch", _unmatched_tag.id, &conn).unwrap();
        }

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[AUTO_TAG_TEST_CSV]).await,
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        // Check that transactions were imported
        let transaction_count = {
            let connection_guard = state.db_connection.lock().unwrap();

            count_transactions(&connection_guard).expect("Could not count transactions")
        };
        assert_eq!(
            transaction_count, 5,
            "Expected 5 transactions to be imported"
        );

        // Validate that no auto-tags were applied
        assert_alert_success_with_details(
            response,
            "Import completed successfully!",
            "No automatic tags were applied",
        )
        .await;
    }

    #[tokio::test]
    async fn import_with_partial_auto_tagging() {
        let conn = get_test_connection();
        let connection = Arc::new(Mutex::new(conn));
        let state = ImportState {
            db_connection: connection.clone(),
        };

        // Create only one rule that matches some transactions
        {
            let conn = connection.lock().unwrap();
            let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &conn).unwrap();
            create_rule("Buckstars", coffee_tag.id, &conn).unwrap();
            // No rule for "Supermarket", so only coffee transactions will be tagged
        }

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[AUTO_TAG_TEST_CSV]).await,
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        // Check that transactions were imported
        let transaction_count = {
            let connection_guard = state.db_connection.lock().unwrap();
            count_transactions(&connection_guard).expect("Could not count transactions")
        };
        assert_eq!(
            transaction_count, 5,
            "Expected 5 transactions to be imported"
        );

        // Validate that only 1 auto-tag was applied (for Starbucks)
        assert_alert_success_with_details(
            response,
            "Import completed successfully!",
            "applied 1 tags automatically",
        )
        .await;
    }

    #[tokio::test]
    async fn import_verifies_auto_tags_applied_to_correct_transactions() {
        let conn = get_test_connection();
        let connection = Arc::new(Mutex::new(conn));
        let state = ImportState {
            db_connection: connection.clone(),
        };

        // Create tags and rules
        let (coffee_tag_id, grocery_tag_id) = {
            let conn = connection.lock().unwrap();
            let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &conn).unwrap();
            let grocery_tag = create_tag(TagName::new_unchecked("Groceries"), &conn).unwrap();

            create_rule("Buckstars", coffee_tag.id, &conn).unwrap();
            create_rule("Supermarket", grocery_tag.id, &conn).unwrap();

            (coffee_tag.id, grocery_tag.id)
        };

        let response = import_transactions(
            State(state.clone()),
            must_make_multipart_csv(&[AUTO_TAG_TEST_CSV]).await,
        )
        .await
        .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::CREATED,
            "Unexpected status code {}, body: {}",
            response.status(),
            String::from_utf8(
                body::to_bytes(response.into_body(), usize::MAX)
                    .await
                    .unwrap()
                    .to_vec()
            )
            .unwrap()
        );

        // Verify specific transactions have correct tags
        let conn = connection.lock().unwrap();

        // Get all transactions and find the ones we expect to be tagged
        let all_transactions: Vec<_> = conn
            .prepare("SELECT id, description, tag_id FROM \"transaction\" ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<TagId>>(2)?,
                ))
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        // Find Starbucks transaction and verify it has coffee tag
        let starbucks_tx = all_transactions
            .iter()
            .find(|(_, desc, _)| desc.contains("Buckstars"))
            .expect("Should find Buckstars transaction");

        assert_eq!(
            starbucks_tx.2,
            Some(coffee_tag_id),
            "Buckstars should have coffee tag"
        );

        // Find Supermarket transaction and verify it has grocery tag
        let supermarket_tx = all_transactions
            .iter()
            .find(|(_, desc, _)| desc.contains("Supermarket"))
            .expect("Should find Supermarket transaction");

        assert_eq!(
            supermarket_tx.2,
            Some(grocery_tag_id),
            "Supermarket should have grocery tag"
        );

        // Verify untagged transactions have no tags
        let random_tx = all_transactions
            .iter()
            .find(|(_, desc, _)| desc.contains("Random Transaction"))
            .expect("Should find Random Transaction");

        assert_eq!(random_tx.2, None, "Random transaction should have no tags");

        // Clean up response to avoid issues
        drop(response);
    }
}

#[cfg(test)]
mod import_transaction_list_tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        Error,
        csv_import::import_transactions::import_transaction_list,
        db::initialize,
        transaction::{Transaction, create_transaction, map_transaction_row},
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn create_succeeds() {
        let conn = get_test_connection();
        let amount = 12.3;

        let result =
            create_transaction(Transaction::build(amount, date!(2025 - 10 - 05), ""), &conn);

        match result {
            Ok(transaction) => assert_eq!(transaction.amount, amount),
            Err(error) => panic!("Unexpected error: {error}"),
        }
    }

    #[test]
    fn import_multiple() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 04);
        let want = vec![
            Transaction::build(123.45, today, "").import_id(Some(123456789)),
            Transaction::build(678.90, today, "").import_id(Some(101112131)),
        ];

        let imported_transactions =
            import_transaction_list(want.clone(), &conn).expect("Could not create transaction");

        assert_eq!(
            want.len(),
            imported_transactions.len(),
            "want {} transactions, got {}",
            want.len(),
            imported_transactions.len()
        );

        for (want, got) in want.iter().zip(imported_transactions) {
            assert_eq!(want.amount, got.amount);
            assert_eq!(want.date, got.date);
            assert_eq!(want.description, got.description);
            assert_eq!(want.import_id, got.import_id);
        }
    }

    #[test]
    fn import_ignores_duplicate_import_id() {
        let conn = get_test_connection();
        let import_id = Some(123456789);
        let today = date!(2025 - 10 - 04);
        let want = create_transaction(
            Transaction::build(123.45, today, "").import_id(import_id),
            &conn,
        )
        .expect("Could not create transaction");

        let duplicate_transactions = import_transaction_list(
            vec![Transaction::build(123.45, today, "").import_id(import_id)],
            &conn,
        )
        .expect("Could not import transactions");

        // The import should return 0 transactions since the import_id already exists
        assert_eq!(
            duplicate_transactions.len(),
            0,
            "import should ignore transactions with duplicate import IDs: want 0 transactions, got {}",
            duplicate_transactions.len()
        );

        // Verify that only the original transaction exists in the database
        let all_transactions = conn
            .prepare(
                "SELECT id, amount, date, description, import_id, tag_id  FROM \"transaction\"",
            )
            .unwrap()
            .query_map([], map_transaction_row)
            .unwrap()
            .map(|transaction_result| transaction_result.map_err(Error::SqlError))
            .collect::<Result<Vec<Transaction>, Error>>()
            .expect("Could not query transactions");

        assert_eq!(
            all_transactions.len(),
            1,
            "Expected exactly 1 transaction in database after duplicate import attempt, got {}",
            all_transactions.len()
        );

        // Verify the original transaction is unchanged
        let stored_transaction = &all_transactions[0];
        assert_eq!(stored_transaction.amount, want.amount);
        assert_eq!(stored_transaction.date, want.date);
        assert_eq!(stored_transaction.description, want.description);
        assert_eq!(stored_transaction.import_id, want.import_id);
    }

    #[tokio::test]
    async fn import_escapes_single_quotes() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let want =
            vec![Transaction::build(123.45, today, "Tom's Hardware").import_id(Some(123456789))];

        let imported_transactions =
            import_transaction_list(want.clone(), &conn).expect("Could not create transaction");

        assert_eq!(
            want.len(),
            imported_transactions.len(),
            "want {} transactions, got {}",
            want.len(),
            imported_transactions.len()
        );

        want.into_iter()
            .zip(imported_transactions)
            .for_each(|(want, got)| {
                assert_eq!(want.amount, got.amount);
                assert_eq!(want.date, got.date);
                assert_eq!(want.description, got.description);
                assert_eq!(want.import_id, got.import_id);
            });
    }
}
