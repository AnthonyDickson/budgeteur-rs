//! Defines the route handler for the page for creating an account.

use axum::{
    extract::{FromRef, State},
    response::{IntoResponse, Response},
};
use maud::{Markup, html};
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error, endpoints,
    navigation::NavBar,
    timezone::get_local_offset,
    view_templates::{
        FORM_LABEL_STYLE, FORM_TEXT_INPUT_STYLE, base, dollar_input_styles, loading_spinner,
    },
};

fn create_account_view(max_date: Date) -> Markup {
    let create_account_route = endpoints::ACCOUNTS;
    let nav_bar = NavBar::new(endpoints::NEW_ACCOUNT_VIEW).into_html();
    let spinner = loading_spinner();

    let content = html! {
        (nav_bar)

        div
            class="flex flex-col items-center px-6 py-8 mx-auto lg:py-0 max-w-md
            text-gray-900 dark:text-white"
        {
            form
                hx-post=(create_account_route)
                hx-target-error="#alert-container"
                class="w-full space-y-4 md:space-y-6"
            {
                h2 class="text-xl font-bold" { "Add Account" }

                div
                {
                    label
                        for="name"
                        class=(FORM_LABEL_STYLE)
                    {
                        "Account Name"
                    }

                    input
                        name="name"
                        id="name"
                        type="text"
                        placeholder="12-3456-7891011-12"
                        autofocus
                        class=(FORM_TEXT_INPUT_STYLE);
                }

                div
                {
                    label
                        for="balance"
                        class=(FORM_LABEL_STYLE)
                    {
                        "Balance"
                    }

                    // w-full needed to ensure input takes the full width when prefilled with a value
                    div class="input-wrapper w-full"
                    {
                        input
                            name="balance"
                            id="balance"
                            type="number"
                            step="0.01"
                            placeholder="0.00"
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
                        "Date "
                        span
                            title="The date for when the balance was last checked"
                            class="text-blue-500"
                        {
                            "ⓘ"
                        }
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
                    " Add Account"
                }
            }
        }
    };

    base("Add Account", &[dollar_input_styles()], &content)
}

/// The state needed for create page.
#[derive(Debug, Clone)]
pub struct CreateAccountPageState {
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
}

impl FromRef<AppState> for CreateAccountPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone.clone(),
        }
    }
}

/// Renders the page for creating an account.
pub async fn get_create_account_page(
    State(state): State<CreateAccountPageState>,
) -> Result<Response, Error> {
    let local_timezone = get_local_offset(&state.local_timezone).ok_or_else(|| {
        tracing::error!(
            "could not get local time offset from timezone {}",
            &state.local_timezone
        );
        Error::InvalidTimezoneError(state.local_timezone)
    })?;

    let max_date = OffsetDateTime::now_utc().to_offset(local_timezone).date();

    Ok(create_account_view(max_date).into_response())
}
