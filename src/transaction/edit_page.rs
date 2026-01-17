use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Path, Query, State},
    response::{IntoResponse, Response},
};
use maud::{Markup, PreEscaped, html};
use rusqlite::Connection;
use serde::Deserialize;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error,
    database_id::TransactionId,
    endpoints::{self, format_endpoint},
    navigation::NavBar,
    tag::{Tag, get_all_tags},
    timezone::get_local_offset,
    transaction::{Transaction, get_transaction},
    view_templates::{FORM_LABEL_STYLE, FORM_TEXT_INPUT_STYLE, HeadElement, base, loading_spinner},
};

fn edit_transaction_view(
    edit_transaction_url: &str,
    max_date: Date,
    transaction: &Transaction,
    available_tags: &[Tag],
) -> Markup {
    let nav_bar = NavBar::new(endpoints::EDIT_TRANSACTION_VIEW).into_html();
    let spinner = loading_spinner();
    let amount_str = format!("{:.2}", transaction.amount);

    let content = html! {
        (nav_bar)

        div
            class="flex flex-col items-center px-6 py-8 mx-auto lg:py-0 max-w-md
            text-gray-900 dark:text-white"
        {
            form
                hx-put=(edit_transaction_url)
                class="w-full space-y-4 md:space-y-6"
            {
                h2 class="text-xl font-bold" { "Edit Transaction" }

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
                            placeholder=(amount_str)
                            value=(amount_str)
                            required
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
                        value=(transaction.date)
                        required
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
                        placeholder=(transaction.description)
                        value=(transaction.description)
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
                                @if Some(tag.id) == transaction.tag_id {
                                    option value=(tag.id) selected { (tag.name) }
                                } @else {
                                    option value=(tag.id) { (tag.name) }
                                }
                            }
                        }
                    }
                }

                button
                    onclick="history.back()"
                    type="button"
                    class="w-full py-2.5 px-5 mb-2 text-sm font-medium text-gray-900
                        bg-white rounded border border-gray-200
                        hover:bg-gray-100 hover:text-blue-700 focus:z-10 dark:bg-gray-800
                        dark:text-gray-400 dark:border-gray-600 dark:hover:text-white
                        dark:hover:bg-gray-700"
                {
                    "Cancel"
                }

                button
                    type="submit"
                    id="submit-button"
                    tabindex="0"
                    class="w-full px-4 py-2 bg-blue-500 dark:bg-blue-600 disabled:bg-blue-700
                        hover:enabled:bg-blue-600 hover:enabled:dark:bg-blue-700 text-white rounded"
                {
                    span
                        id="indicator"
                        class="inline htmx-indicator"
                    {
                        (spinner)
                    }
                    " Edit Transaction"
                }
            }
        }
    };

    let style = HeadElement::Style(PreEscaped(
        r#"
        .input-wrapper {
            position: relative;
            display: inline-block;
        }
        .input-wrapper input[type="number"] {
            padding-left: 1.4rem;
        }
        .input-wrapper::before {
            content: '$';
            position: absolute;
            left: 0.6rem;
            top: 50%;
            transform: translateY(-52%);
            pointer-events: none;
        }
        "#
        .to_owned(),
    ));

    base(
        &format!("Edit Transaction #{}", transaction.id),
        &[style],
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
        Some(redirect_url) => format!("{base_url}?redirect_url={redirect_url}"),
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
