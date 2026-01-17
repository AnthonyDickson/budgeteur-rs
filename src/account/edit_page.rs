use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, Path, State},
    response::{IntoResponse, Response},
};
use maud::{Markup, html};
use rusqlite::Connection;
use time::{Date, OffsetDateTime};

use crate::{
    AppState, Error,
    account::{Account, map_row_to_account},
    database_id::DatabaseId,
    endpoints::{self, format_endpoint},
    html::{
        BUTTON_PRIMARY_STYLE, BUTTON_SECONDARY_STYLE, FORM_CONTAINER_STYLE, FORM_LABEL_STYLE,
        FORM_TEXT_INPUT_STYLE, base, dollar_input_styles, loading_spinner,
    },
    navigation::NavBar,
    timezone::get_local_offset,
};

fn edit_account_view(edit_url: &str, max_date: Date, account: &Account) -> Markup {
    let nav_bar = NavBar::new(endpoints::EDIT_ACCOUNT_VIEW).into_html();
    let spinner = loading_spinner();
    let balance_str = format!("{:.2}", account.balance);

    let content = html! {
        (nav_bar)

        div class=(FORM_CONTAINER_STYLE)
        {
            form
                hx-put=(edit_url)
                hx-target-error="#alert-container"
                class="w-full space-y-4 md:space-y-6"
            {
                h2 class="text-xl font-bold" { "Edit Account" }

                div
                {
                    label
                        for="name"
                        class=(FORM_LABEL_STYLE)
                    {
                        "Name"
                    }

                    input
                        name="name"
                        id="name"
                        type="text"
                        placeholder=(account.name)
                        value=(account.name)
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
                            placeholder=(balance_str)
                            value=(balance_str)
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
                        value=(account.date)
                        required
                        class=(FORM_TEXT_INPUT_STYLE);
                }

                button onclick="history.back()" type="button" class=(BUTTON_SECONDARY_STYLE) { "Cancel" }

                button type="submit" id="submit-button" tabindex="0" class=(BUTTON_PRIMARY_STYLE)
                {
                    span id="indicator" class="inline htmx-indicator" { (spinner) }
                    " Edit Account"
                }
            }
        }
    };

    base(
        &format!("Edit Account #{}", account.id),
        &[dollar_input_styles()],
        &content,
    )
}

/// The state needed for the edit account page.
#[derive(Debug, Clone)]
pub struct EditAccountPageState {
    /// The local timezone as a canonical timezone name, e.g. "Pacific/Auckland".
    pub local_timezone: String,
    /// The database connection for accessing tags.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for EditAccountPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            local_timezone: state.local_timezone.clone(),
            db_connection: state.db_connection.clone(),
        }
    }
}

/// Renders the page for editing an account.
pub async fn get_edit_account_page(
    State(state): State<EditAccountPageState>,
    Path(account_id): Path<DatabaseId>,
) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let account = get_account(account_id, &connection).inspect_err(|error| match error {
        Error::NotFound => {}
        error => tracing::error!("Failed to retrieve transaction {account_id}: {error}"),
    })?;

    let local_timezone = get_local_offset(&state.local_timezone).ok_or_else(|| {
        tracing::error!("Invalid timezone {}", state.local_timezone);
        Error::InvalidTimezoneError(state.local_timezone)
    })?;

    let edit_url = format_endpoint(endpoints::EDIT_ACCOUNT, account_id);
    let max_date = OffsetDateTime::now_utc().to_offset(local_timezone).date();

    Ok(edit_account_view(&edit_url, max_date, &account).into_response())
}

/// Retrieve an account from the database by its `id`.
///
/// # Errors
/// This function will return a:
/// - [Error::NotFound] if `id` does not refer to a valid account,
/// - or [Error::SqlError] there is some other SQL error.
fn get_account(id: DatabaseId, connection: &Connection) -> Result<Account, Error> {
    let account = connection
        .prepare("SELECT id, name, balance, date FROM account WHERE id = :id")?
        .query_one(&[(":id", &id)], map_row_to_account)?;

    Ok(account)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        account::{
            create_endpoint::{AccountForm, create_account},
            edit_page::get_account,
        },
        initialize_db,
    };

    #[test]
    fn test_get_account() {
        let connection = must_create_test_connection();
        let want_account = create_account(
            &AccountForm {
                name: "foo".to_owned(),
                balance: 1.23,
                date: date!(2025 - 11 - 02),
            },
            &connection,
        );

        let got_account = get_account(1, &connection);

        assert_eq!(want_account, got_account);
    }

    #[track_caller]
    fn must_create_test_connection() -> Connection {
        let connection =
            Connection::open_in_memory().expect("could not create in-memory SQLite database");
        initialize_db(&connection).expect("could not initialize test DB");

        connection
    }
}
