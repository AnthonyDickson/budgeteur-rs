//! Defines the route handler for the page for creating a new transaction.

use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, State},
    response::{IntoResponse, Response},
};
use maud::{Markup, html};
use rusqlite::Connection;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error, endpoints,
    navigation::NavBar,
    tag::{Tag, get_all_tags},
    timezone::get_local_offset,
    view_templates::{
        BUTTON_PRIMARY_STYLE, FORM_CONTAINER_STYLE, FORM_LABEL_STYLE, FORM_TEXT_INPUT_STYLE, base,
        dollar_input_styles, loading_spinner,
    },
};

fn create_transaction_view(max_date: Date, available_tags: &[Tag]) -> Markup {
    let create_transaction_route = endpoints::TRANSACTIONS_API;
    let nav_bar = NavBar::new(endpoints::NEW_TRANSACTION_VIEW).into_html();
    let spinner = loading_spinner();

    let content = html! {
        (nav_bar)

        div class=(FORM_CONTAINER_STYLE)
        {
            form
                hx-post=(create_transaction_route)
                hx-target-error="#alert-container"
                class="w-full space-y-4 md:space-y-6"
            {
                h2 class="text-xl font-bold" { "New Transaction" }

                div
                {
                    label
                        for="amount"
                        class=(FORM_LABEL_STYLE)
                    {
                        "Amount"
                    }

                    // w-full needed to ensure input takes the full width when prefilled with a value
                    div class="input-wrapper w-full"
                    {
                        input
                            name="amount"
                            id="amount"
                            type="number"
                            step="0.01"
                            placeholder="0.00"
                            required
                            autofocus
                            class=(FORM_TEXT_INPUT_STYLE);
                    }
                }

                div
                {
                    label
                        for="date"
                        class=(FORM_LABEL_STYLE)
                    {
                        "Date"
                    }

                    input
                        name="date"
                        id="date"
                        type="date"
                        max=(max_date)
                        required
                        value=(max_date)
                        class=(FORM_TEXT_INPUT_STYLE);
                }

                div
                {
                    label
                        for="description"
                        class=(FORM_LABEL_STYLE)
                    {
                        "Description"
                    }

                    input
                        name="description"
                        id="description"
                        type="text"
                        placeholder="Description"
                        class=(FORM_TEXT_INPUT_STYLE);
                }

                @if !available_tags.is_empty() {
                    div
                    {
                        label
                            for="tag_id"
                            class=(FORM_LABEL_STYLE)
                        {
                            "Tag"
                        }

                        select
                            name="tag_id"
                            id="tag_id"
                            class=(FORM_TEXT_INPUT_STYLE)
                        {
                            option value="" { "Select a tag" }

                            @for tag in available_tags {
                                option value=(tag.id) { (tag.name) }
                            }
                        }
                    }
                }

                button type="submit" id="submit-button" tabindex="0" class=(BUTTON_PRIMARY_STYLE)
                {
                    span
                        id="indicator"
                        class="inline htmx-indicator"
                    {
                        (spinner)
                    }
                    " Create Transaction"
                }
            }
        }
    };

    base("Create Transaction", &[dollar_input_styles()], &content)
}

/// The state needed for create new transaction page.
#[derive(Debug, Clone)]
pub struct CreateTransactionPageState {
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
    /// The database connection for accessing tags.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for CreateTransactionPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone.clone(),
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Renders the page for creating a transaction.
pub async fn get_create_transaction_page(
    State(state): State<CreateTransactionPageState>,
) -> Result<Response, Error> {
    let available_tags = {
        let connection = state
            .db_connection
            .lock()
            .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
            .map_err(|_| Error::DatabaseLockError)?;

        get_all_tags(&connection).inspect_err(|error| {
            tracing::error!("Failed to retrieve tags for new transaction page: {error}")
        })?
    };

    let local_timezone = get_local_offset(&state.local_timezone).ok_or_else(|| {
        tracing::error!("Invalid timezone {}", state.local_timezone);
        Error::InvalidTimezoneError(state.local_timezone)
    })?;

    let max_date = OffsetDateTime::now_utc().to_offset(local_timezone).date();

    Ok(create_transaction_view(max_date, &available_tags).into_response())
}

#[cfg(test)]
mod view_tests {
    use std::sync::{Arc, Mutex};

    use axum::{body::Body, extract::State, http::StatusCode, response::Response};
    use rusqlite::Connection;
    use scraper::{ElementRef, Html};
    use time::OffsetDateTime;

    use crate::{
        db::initialize,
        endpoints,
        transaction::{create_page::CreateTransactionPageState, get_create_transaction_page},
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn new_transaction_returns_form() {
        let conn = get_test_connection();
        let state = CreateTransactionPageState {
            local_timezone: "Etc/UTC".to_owned(),
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = get_create_transaction_page(State(state)).await.unwrap();

        assert_status_ok(&response);
        assert_html_content_type(&response);
        let document = parse_html(response).await;
        assert_valid_html(&document);
        assert_correct_form(&document);
    }

    #[track_caller]
    fn assert_status_ok(response: &Response<Body>) {
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[track_caller]
    fn assert_html_content_type(response: &Response<Body>) {
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/html; charset=utf-8"
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

    #[track_caller]
    fn assert_correct_form(document: &Html) {
        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());

        let form = forms.first().unwrap();
        let hx_post = form.value().attr("hx-post");
        assert_eq!(
            hx_post,
            Some(endpoints::TRANSACTIONS_API),
            "want form with attribute hx-post=\"{}\", got {:?}",
            endpoints::TRANSACTIONS_API,
            hx_post
        );

        assert_correct_inputs(form);
        assert_has_submit_button(form);
    }

    #[track_caller]
    fn assert_correct_inputs(form: &ElementRef) {
        let expected_input_types = vec![
            ("amount", "number"),
            ("date", "date"),
            ("description", "text"),
        ];

        for (name, element_type) in expected_input_types {
            let selector_string = format!("input[type={element_type}]");
            let input_selector = scraper::Selector::parse(&selector_string).unwrap();
            let inputs = form.select(&input_selector).collect::<Vec<_>>();
            assert_eq!(
                inputs.len(),
                1,
                "want 1 {element_type} input, got {}",
                inputs.len()
            );

            let input = inputs.first().unwrap();

            let input_name = input.value().attr("name");
            assert_eq!(
                input_name,
                Some(name),
                "want {element_type} with name=\"{name}\", got {input_name:?}"
            );

            match input_name {
                Some("amount") => {
                    assert_required(input);
                    assert_amount_step(input);
                }
                Some("date") => {
                    assert_required(input);
                    assert_max_date(input);
                    assert_value(input, &OffsetDateTime::now_utc().date().to_string());
                }
                _ => {}
            }
        }
    }

    #[track_caller]
    fn assert_value(input: &ElementRef, expected_value: &str) {
        let value = input.value().attr("value");
        assert_eq!(
            value,
            Some(expected_value),
            "want input with value=\"{expected_value}\", got {value:?}"
        );
    }

    #[track_caller]
    fn assert_required(input: &ElementRef) {
        let required = input.value().attr("required");
        let input_name = input.value().attr("name").unwrap();
        assert!(
            required.is_some(),
            "want {input_name} input to be required, got {required:?}"
        );
    }

    #[track_caller]
    fn assert_max_date(input: &ElementRef) {
        let today = OffsetDateTime::now_utc().date();
        let max_date = input.value().attr("max");

        assert_eq!(
            Some(today.to_string().as_str()),
            max_date,
            "the date for a new transaction should be limited to the current date {today}, but got {max_date:?}"
        );
    }

    #[track_caller]
    fn assert_amount_step(input: &ElementRef) {
        let step = input
            .value()
            .attr("step")
            .expect("amount input should have the attribute 'step'");
        let step: f64 = step
            .parse()
            .expect("the attribute 'step' for the amount input should be a float");
        assert_eq!(
            0.01, step,
            "the amount for a new transaction should increment in steps of 0.01, but got {step}"
        );
    }

    #[track_caller]
    fn assert_has_submit_button(form: &ElementRef) {
        let button_selector = scraper::Selector::parse("button").unwrap();
        let buttons = form.select(&button_selector).collect::<Vec<_>>();
        assert_eq!(buttons.len(), 1, "want 1 button, got {}", buttons.len());
        let button_type = buttons.first().unwrap().value().attr("type");
        assert_eq!(
            button_type,
            Some("submit"),
            "want button with type=\"submit\", got {button_type:?}"
        );
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
