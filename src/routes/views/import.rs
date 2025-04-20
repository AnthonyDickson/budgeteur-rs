use askama_axum::Template;
use axum::{
    Extension,
    extract::{Multipart, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;

use crate::{
    AppState,
    csv::parse_csv,
    models::UserID,
    routes::{
        endpoints,
        navigation::{NavbarTemplate, get_nav_bar},
    },
    stores::{CategoryStore, TransactionStore, UserStore},
};

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

pub async fn import_transactions<C, T, U>(
    State(mut state): State<AppState<C, T, U>>,
    Extension(user_id): Extension<UserID>,
    mut multipart: Multipart,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    let mut transactions = Vec::new();

    while let Some(field) = multipart.next_field().await.unwrap() {
        if field.content_type() != Some("text/csv") {
            return (
                StatusCode::BAD_REQUEST,
                ImportTransactionFormTemplate {
                    import_route: endpoints::IMPORT,
                    error_message: "File type must be CSV.",
                },
            )
                .into_response();
        }

        let file_name = field.file_name().unwrap().to_string();
        let data = field.text().await.unwrap();

        tracing::debug!(
            "Received file '{}' that is {} bytes: {}",
            file_name,
            data.len(),
            data
        );

        match parse_csv(&data, user_id) {
            Ok(parsed_transactions) => {
                transactions.extend(parsed_transactions);
            }
            Err(e) => {
                tracing::debug!("Failed to parse CSV: {}", e);
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    ImportTransactionFormTemplate {
                        import_route: endpoints::IMPORT,
                        error_message: "Failed to parse CSV, check that the provided file is a valid CSV from ASB or Kiwibank.",
                    },
                )
                    .into_response();
            }
        }
    }

    // TODO: Add function to store to add many transactions at once.
    // TODO: Filter out transactions that have already been imported by
    // checking the import ID.
    for transaction in transactions {
        state
            .transaction_store
            .create_from_builder(transaction)
            .unwrap();
    }

    (
        HxRedirect(Uri::from_static(endpoints::TRANSACTIONS_VIEW)),
        StatusCode::SEE_OTHER,
    )
        .into_response()
}

#[cfg(test)]
mod import_transactions_tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        Extension,
        extract::{FromRequest, Multipart, State},
        http::{Request, StatusCode},
        response::Response,
    };
    use scraper::{ElementRef, Html};
    use time::macros::date;

    use crate::{
        AppState, Error,
        models::{Category, DatabaseID, PasswordHash, Transaction, TransactionBuilder, UserID},
        routes::{
            endpoints,
            views::import::{get_import_page, import_transactions},
        },
        stores::{CategoryStore, TransactionStore, UserStore, transaction::TransactionQuery},
    };

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
    async fn post_asb_bank_csv() {
        let state = AppState::new(
            "foo",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );
        let user_id = UserID::new(123);

        let want_transactions: Vec<Transaction> = vec![
            TransactionBuilder::new(1300.00, user_id)
                .date(date!(2025 - 01 - 18))
                .expect("Could not parse date")
                .description("Credit Card")
                .finalise(0),
            TransactionBuilder::new(-1300.00, user_id)
                .date(date!(2025 - 01 - 18))
                .expect("Could not parse date")
                .description("TO CARD 5023  Credit Card")
                .finalise(1),
            TransactionBuilder::new(4400.00, user_id)
                .date(date!(2025 - 02 - 18))
                .expect("Could not parse date")
                .description("Credit Card")
                .finalise(2),
            TransactionBuilder::new(-4400.00, user_id)
                .date(date!(2025 - 02 - 19))
                .expect("Could not parse date")
                .description("TO CARD 5023  THANK YOU")
                .finalise(3),
            TransactionBuilder::new(2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not parse date")
                .description("Credit Card")
                .finalise(4),
            TransactionBuilder::new(-2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not parse date")
                .description("TO CARD 5023  THANK YOU")
                .finalise(5),
        ];

        let response = import_transactions(
            State(state.clone()),
            Extension(user_id),
            must_make_multipart_csv(&[ASB_BANK_STATEMENT_CSV]).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let create_transaction_calls = state.transaction_store.create_calls.lock().unwrap().len();
        assert_eq!(
            create_transaction_calls,
            want_transactions.len(),
            "want {} transaction created, got {create_transaction_calls}",
            want_transactions.len()
        );
        assert_hx_redirect(&response, endpoints::TRANSACTIONS_VIEW);
    }

    #[tokio::test]
    async fn post_asb_cc_csv() {
        let state = AppState::new(
            "foo",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );
        let user_id = UserID::new(123);

        let want_transactions: Vec<Transaction> = vec![
            TransactionBuilder::new(2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not parse date")
                .description("PAYMENT RECEIVED THANK YOU")
                .finalise(0),
            TransactionBuilder::new(-8.50, user_id)
                .date(date!(2025 - 04 - 09))
                .expect("Could not parse date")
                .description("Birdy Bytes")
                .finalise(1),
            TransactionBuilder::new(-10.63, user_id)
                .date(date!(2025 - 04 - 10))
                .expect("Could not parse date")
                .description(
                    "AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)",
                )
                .finalise(2),
            TransactionBuilder::new(-0.22, user_id)
                .date(date!(2025 - 04 - 10))
                .expect("Could not parse date")
                .description("OFFSHORE SERVICE MARGINS")
                .finalise(3),
            TransactionBuilder::new(-11.50, user_id)
                .date(date!(2025 - 04 - 11))
                .expect("Could not parse date")
                .description("Buckstars")
                .finalise(4),
        ];

        let response = import_transactions(
            State(state.clone()),
            Extension(user_id),
            must_make_multipart_csv(&[ASB_CC_STATEMENT_CSV]).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let create_transaction_calls = state.transaction_store.create_calls.lock().unwrap().len();
        assert_eq!(
            create_transaction_calls,
            want_transactions.len(),
            "want {} transaction created, got {create_transaction_calls}",
            want_transactions.len()
        );
        assert_hx_redirect(&response, endpoints::TRANSACTIONS_VIEW);
    }

    #[tokio::test]
    async fn post_kiwibank_bank_csv() {
        let state = AppState::new(
            "foo",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );
        let user_id = UserID::new(999);

        let want_transactions: Vec<Transaction> = vec![
            TransactionBuilder::new(0.25, user_id)
                .date(date!(2025 - 01 - 31))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .finalise(0),
            TransactionBuilder::new(-0.03, user_id)
                .date(date!(2025 - 01 - 31))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .finalise(1),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 02 - 28))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .finalise(2),
            TransactionBuilder::new(-0.02, user_id)
                .date(date!(2025 - 02 - 28))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .finalise(3),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 03 - 31))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .finalise(4),
            TransactionBuilder::new(-0.02, user_id)
                .date(date!(2025 - 03 - 31))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .finalise(5),
        ];

        let response = import_transactions(
            State(state.clone()),
            Extension(user_id),
            must_make_multipart_csv(&[KIWIBANK_BANK_STATEMENT_CSV]).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let create_transaction_calls = state.transaction_store.create_calls.lock().unwrap().len();
        assert_eq!(
            create_transaction_calls,
            want_transactions.len(),
            "want {} transaction created, got {create_transaction_calls}",
            want_transactions.len()
        );
        assert_hx_redirect(&response, endpoints::TRANSACTIONS_VIEW);
    }

    #[tokio::test]
    async fn post_multiple_bank_csv() {
        let state = AppState::new(
            "foo",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );
        let user_id = UserID::new(123);

        let want_transactions: Vec<Transaction> = vec![
            TransactionBuilder::new(1300.00, user_id)
                .date(date!(2025 - 01 - 18))
                .expect("Could not parse date")
                .description("Credit Card")
                .finalise(0),
            TransactionBuilder::new(-1300.00, user_id)
                .date(date!(2025 - 01 - 18))
                .expect("Could not parse date")
                .description("TO CARD 5023  Credit Card")
                .finalise(1),
            TransactionBuilder::new(4400.00, user_id)
                .date(date!(2025 - 02 - 18))
                .expect("Could not parse date")
                .description("Credit Card")
                .finalise(2),
            TransactionBuilder::new(-4400.00, user_id)
                .date(date!(2025 - 02 - 19))
                .expect("Could not parse date")
                .description("TO CARD 5023  THANK YOU")
                .finalise(3),
            TransactionBuilder::new(2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not parse date")
                .description("Credit Card")
                .finalise(4),
            TransactionBuilder::new(-2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not parse date")
                .description("TO CARD 5023  THANK YOU")
                .finalise(5),
            TransactionBuilder::new(2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not parse date")
                .description("PAYMENT RECEIVED THANK YOU")
                .finalise(6),
            TransactionBuilder::new(-8.50, user_id)
                .date(date!(2025 - 04 - 09))
                .expect("Could not parse date")
                .description("Birdy Bytes")
                .finalise(7),
            TransactionBuilder::new(-10.63, user_id)
                .date(date!(2025 - 04 - 10))
                .expect("Could not parse date")
                .description(
                    "AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)",
                )
                .finalise(8),
            TransactionBuilder::new(-0.22, user_id)
                .date(date!(2025 - 04 - 10))
                .expect("Could not parse date")
                .description("OFFSHORE SERVICE MARGINS")
                .finalise(9),
            TransactionBuilder::new(-11.50, user_id)
                .date(date!(2025 - 04 - 11))
                .expect("Could not parse date")
                .description("Buckstars")
                .finalise(10),
            TransactionBuilder::new(0.25, user_id)
                .date(date!(2025 - 01 - 31))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .finalise(11),
            TransactionBuilder::new(-0.03, user_id)
                .date(date!(2025 - 01 - 31))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .finalise(12),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 02 - 28))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .finalise(13),
            TransactionBuilder::new(-0.02, user_id)
                .date(date!(2025 - 02 - 28))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .finalise(14),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 03 - 31))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .finalise(15),
            TransactionBuilder::new(-0.02, user_id)
                .date(date!(2025 - 03 - 31))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .finalise(16),
        ];

        let response = import_transactions(
            State(state.clone()),
            Extension(user_id),
            must_make_multipart_csv(&[
                ASB_BANK_STATEMENT_CSV,
                ASB_CC_STATEMENT_CSV,
                KIWIBANK_BANK_STATEMENT_CSV,
            ])
            .await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let create_transaction_calls = state.transaction_store.create_calls.lock().unwrap().len();
        assert_eq!(
            create_transaction_calls,
            want_transactions.len(),
            "want {} transaction created, got {create_transaction_calls}",
            want_transactions.len()
        );
        assert_hx_redirect(&response, endpoints::TRANSACTIONS_VIEW);
    }

    // TODO: Test post import extracts balance, account number and creates unique IDs for each
    // transaction.
    // TODO: Test post import rejects transactions that have already been imported.
    #[tokio::test]
    async fn invalid_csv_renders_error_message() {
        let state = AppState::new(
            "foo",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );
        let user_id = UserID::new(123);

        let response = import_transactions(
            State(state.clone()),
            Extension(user_id),
            must_make_multipart_csv(&[""]).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_content_type(&response, "text/html; charset=utf-8");
        let create_transaction_calls = state.transaction_store.create_calls.lock().unwrap().len();
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
        let state = AppState::new(
            "foo",
            DummyCategoryStore {},
            FakeTransactionStore::new(),
            DummyUserStore {},
        );
        let user_id = UserID::new(123);

        let response = import_transactions(
            State(state.clone()),
            Extension(user_id),
            must_make_multipart(&["text/plain"]).await,
        )
        .await;

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_content_type(&response, "text/html; charset=utf-8");
        let create_transaction_calls = state.transaction_store.create_calls.lock().unwrap().len();
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
    struct DummyUserStore {}

    impl UserStore for DummyUserStore {
        fn create(
            &mut self,
            _email: email_address::EmailAddress,
            _password_hash: PasswordHash,
        ) -> Result<crate::models::User, Error> {
            todo!()
        }

        fn get(&self, _id: UserID) -> Result<crate::models::User, Error> {
            todo!()
        }

        fn get_by_email(
            &self,
            _email: &email_address::EmailAddress,
        ) -> Result<crate::models::User, Error> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyCategoryStore {}

    impl CategoryStore for DummyCategoryStore {
        fn create(
            &self,
            _name: crate::models::CategoryName,
            _user_id: UserID,
        ) -> Result<Category, Error> {
            todo!()
        }

        fn get(&self, _category_id: DatabaseID) -> Result<Category, Error> {
            todo!()
        }

        fn get_by_user(&self, _user_id: UserID) -> Result<Vec<Category>, Error> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct FakeTransactionStore {
        transactions: Vec<Transaction>,
        create_calls: Arc<Mutex<Vec<Transaction>>>,
    }

    impl FakeTransactionStore {
        fn new() -> Self {
            Self {
                transactions: Vec::new(),
                create_calls: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl TransactionStore for FakeTransactionStore {
        fn create(&mut self, amount: f64, user_id: UserID) -> Result<Transaction, Error> {
            self.create_from_builder(TransactionBuilder::new(amount, user_id))
        }

        fn create_from_builder(
            &mut self,
            builder: TransactionBuilder,
        ) -> Result<Transaction, Error> {
            let next_id = match self.transactions.last() {
                Some(transaction) => transaction.id() + 1,
                None => 0,
            };

            let transaction = builder.finalise(next_id);

            self.transactions.push(transaction.clone());
            self.create_calls.lock().unwrap().push(transaction.clone());

            Ok(transaction)
        }

        fn get(&self, id: DatabaseID) -> Result<Transaction, Error> {
            self.transactions
                .iter()
                .find(|transaction| transaction.id() == id)
                .ok_or(Error::NotFound)
                .map(|transaction| transaction.to_owned())
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, Error> {
            todo!()
        }

        fn get_query(&self, _filter: TransactionQuery) -> Result<Vec<Transaction>, Error> {
            todo!()
        }
    }
}
