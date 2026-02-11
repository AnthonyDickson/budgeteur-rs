use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Path, Query, State},
    response::{IntoResponse, Response},
};
use maud::{Markup, html};
use rusqlite::Connection;
use serde::Deserialize;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error,
    endpoints::{self, format_endpoint},
    html::{
        BUTTON_PRIMARY_STYLE, BUTTON_SECONDARY_STYLE, FORM_CONTAINER_STYLE, base,
        dollar_input_styles, loading_spinner,
    },
    navigation::NavBar,
    tag::{Tag, get_all_tags},
    timezone::get_local_offset,
    transaction::{
        Transaction, TransactionId,
        form::{TransactionFormDefaults, transaction_form_fields},
        get_transaction,
    },
};

fn edit_transaction_view(
    edit_transaction_url: &str,
    max_date: Date,
    transaction: &Transaction,
    available_tags: &[Tag],
) -> Markup {
    let nav_bar = NavBar::new(endpoints::EDIT_TRANSACTION_VIEW).into_html();
    let spinner = loading_spinner();
    let form_defaults = TransactionFormDefaults {
        transaction_type: transaction.type_(),
        amount: Some(transaction.amount.abs()),
        date: transaction.date,
        description: Some(&transaction.description),
        tag_id: transaction.tag_id,
        max_date,
        autofocus_amount: false,
    };

    let content = html! {
        (nav_bar)

        div class=(FORM_CONTAINER_STYLE)
        {
            form
                hx-put=(edit_transaction_url)
                class="w-full space-y-4 md:space-y-6"
            {
                h2 class="text-xl font-bold" { "Edit Transaction" }

                (transaction_form_fields(&form_defaults, available_tags))

                button onclick="history.back()" type="button" class=(BUTTON_SECONDARY_STYLE) { "Cancel" }

                button type="submit" id="submit-button" tabindex="0" class=(BUTTON_PRIMARY_STYLE)
                {
                    span id="indicator" class="inline htmx-indicator" { (spinner) }
                    " Update Transaction"
                }
            }
        }
    };

    base(
        &format!("Edit Transaction #{}", transaction.id),
        &[dollar_input_styles()],
        &content,
    )
}

/// The state needed for the edit transaction page.
#[derive(Debug, Clone)]
pub struct EditTransactionPageState {
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
    /// The database connection for accessing tags.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for EditTransactionPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone.clone(),
            db_connection: state.db_connection.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct QueryParams {
    redirect_url: Option<String>,
}

/// Renders the page for editing a transaction.
pub async fn get_edit_transaction_page(
    State(state): State<EditTransactionPageState>,
    Path(transaction_id): Path<TransactionId>,
    Query(query_params): Query<QueryParams>,
) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("Could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let transaction =
        get_transaction(transaction_id, &connection).inspect_err(|error| match error {
            Error::NotFound => {}
            error => {
                tracing::error!("Failed to retrieve transaction {transaction_id}: {error}")
            }
        })?;

    let available_tags = get_all_tags(&connection).inspect_err(|error| {
        tracing::error!("Failed to retrieve tags for new transaction page: {error}")
    })?;

    let local_timezone = get_local_offset(&state.local_timezone).ok_or_else(|| {
        tracing::error!("Invalid timezone {}", state.local_timezone);
        Error::InvalidTimezoneError(state.local_timezone)
    })?;

    let base_url = format_endpoint(endpoints::EDIT_TRANSACTION_VIEW, transaction_id);
    let edit_transaction_url = match query_params.redirect_url {
        Some(redirect_url) => {
            let redirect_url_param = serde_urlencoded::to_string([(
                "redirect_url",
                redirect_url.as_str(),
            )])
            .inspect_err(|error| {
                tracing::error!(
                    "Could not set redirect URL {redirect_url} due to encoding error: {error}"
                );
            })
            .ok();

            redirect_url_param
                .map(|param| format!("{base_url}?{param}"))
                .unwrap_or(base_url)
        }
        None => base_url,
    };

    let max_date = OffsetDateTime::now_utc().to_offset(local_timezone).date();

    Ok(edit_transaction_view(
        &edit_transaction_url,
        max_date,
        &transaction,
        &available_tags,
    )
    .into_response())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::extract::{Path, Query, State};
    use rusqlite::Connection;
    use scraper::{ElementRef, Html};
    use time::OffsetDateTime;

    use crate::{
        db::initialize,
        endpoints,
        transaction::{
            Transaction, create_transaction,
            edit_page::{EditTransactionPageState, QueryParams, get_edit_transaction_page},
            test_utils::{
                assert_html_content_type, assert_status_ok, assert_transaction_type_inputs,
                assert_valid_html, parse_html,
            },
        },
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn edit_transaction_checks_type() {
        let cases = [(-12.34, "expense"), (12.34, "income")];

        for (amount, expected_type) in cases {
            let conn = get_test_connection();
            let transaction = create_transaction(
                Transaction::build(amount, OffsetDateTime::now_utc().date(), "test"),
                &conn,
            )
            .expect("could not create test transaction");
            let state = EditTransactionPageState {
                local_timezone: "Etc/UTC".to_owned(),
                db_connection: Arc::new(Mutex::new(conn)),
            };

            let response = get_edit_transaction_page(
                State(state),
                Path(transaction.id),
                Query(QueryParams { redirect_url: None }),
            )
            .await
            .unwrap();

            assert_status_ok(&response);
            assert_html_content_type(&response);
            let document = parse_html(response).await;
            assert_valid_html(&document);
            assert_correct_form(&document, transaction.id, expected_type);
        }
    }

    #[tokio::test]
    async fn edit_transaction_preserves_redirect_url_query() {
        let conn = get_test_connection();
        let transaction = create_transaction(
            Transaction::build(12.0, OffsetDateTime::now_utc().date(), "test"),
            &conn,
        )
        .expect("could not create test transaction");
        let state = EditTransactionPageState {
            local_timezone: "Etc/UTC".to_owned(),
            db_connection: Arc::new(Mutex::new(conn)),
        };
        let redirect_url = "/transactions?window=month&anchor=2025-10-05".to_owned();

        let response = get_edit_transaction_page(
            State(state),
            Path(transaction.id),
            Query(QueryParams {
                redirect_url: Some(redirect_url.clone()),
            }),
        )
        .await
        .unwrap();

        let document = parse_html(response).await;
        assert_valid_html(&document);

        let expected_query = serde_urlencoded::to_string([("redirect_url", redirect_url.as_str())])
            .expect("Could not encode redirect_url");
        let expected_endpoint =
            endpoints::format_endpoint(endpoints::EDIT_TRANSACTION_VIEW, transaction.id);
        let expected_hx_put = format!("{expected_endpoint}?{expected_query}");
        let form = document
            .select(&scraper::Selector::parse("form").unwrap())
            .next()
            .expect("No form found");
        let hx_put = form.value().attr("hx-put").expect("No hx-put found");

        assert_eq!(
            hx_put, expected_hx_put,
            "want form with attribute hx-put=\"{}\", got {}",
            expected_hx_put, hx_put
        );
    }

    #[track_caller]
    fn assert_correct_form(document: &Html, transaction_id: u32, checked_type: &str) {
        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());

        let form = forms.first().unwrap();
        let hx_put = form.value().attr("hx-put");
        let expected_endpoint =
            endpoints::format_endpoint(endpoints::EDIT_TRANSACTION_VIEW, transaction_id);
        assert_eq!(
            hx_put,
            Some(expected_endpoint.as_str()),
            "want form with attribute hx-put=\"{}\", got {:?}",
            expected_endpoint,
            hx_put
        );

        assert_transaction_type_inputs(form, Some(checked_type));
        assert_has_submit_button(form);
    }

    #[track_caller]
    fn assert_has_submit_button(form: &ElementRef) {
        let button_selector = scraper::Selector::parse("button").unwrap();
        let buttons = form.select(&button_selector).collect::<Vec<_>>();
        assert!(
            !buttons.is_empty(),
            "want at least 1 button, got {}",
            buttons.len()
        );
        let submit_buttons = buttons
            .iter()
            .filter(|button| button.value().attr("type") == Some("submit"))
            .count();
        assert_eq!(
            submit_buttons, 1,
            "want 1 submit button, got {submit_buttons}"
        );
    }
}
