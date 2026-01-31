use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use maud::{Markup, html};

use crate::{
    Error, endpoints,
    html::{FORM_LABEL_STYLE, FORM_TEXT_INPUT_STYLE, base},
    navigation::NavBar,
    rule::{
        db::create_rule,
        models::{RuleFormData, RuleState},
    },
    tag::{Tag, get_all_tags},
};

/// Route handler for the new rule page.
pub async fn get_new_rule_page(State(state): State<RuleState>) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let available_tags = get_all_tags(&connection).inspect_err(|error| {
        tracing::error!("Failed to retrieve tags for new rule page: {error}")
    })?;

    Ok(new_rule_view(&available_tags, "").into_response())
}

/// A route handler for creating a new rule.
pub async fn create_rule_endpoint(
    State(state): State<RuleState>,
    Form(new_rule): Form<RuleFormData>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    let available_tags = get_all_tags(&connection).unwrap_or_default();

    if new_rule.pattern.trim().is_empty() {
        return new_rule_view(&available_tags, "Error: Pattern cannot be empty").into_response();
    }

    let rule_result = create_rule(new_rule.pattern.trim(), new_rule.tag_id, &connection);

    match rule_result {
        Ok(_) => (
            HxRedirect(endpoints::RULES_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while creating a rule: {error}");

            error.into_alert_response()
        }
    }
}

fn new_rule_view(available_tags: &[Tag], error_message: &str) -> Markup {
    let nav_bar = NavBar::new(endpoints::NEW_RULE_VIEW).into_html();
    let form = new_rule_form_view(available_tags, error_message);

    let content = html! {
        (nav_bar)

        div
            class="flex flex-col items-center px-6 py-8 mx-auto lg:py-0 max-w-md
            text-gray-900 dark:text-white"
        {
            (form)
        }
    };

    base("Create Rule", &[], &content)
}

fn new_rule_form_view(available_tags: &[Tag], error_message: &str) -> Markup {
    let create_rule_endpoint = endpoints::POST_RULE;

    html! {
        form
            hx-post=(create_rule_endpoint)
            hx-target-error="#alert-container"
            class="w-full space-y-4 md:space-y-6"
        {
            div
            {
                label
                    for="pattern"
                    class=(FORM_LABEL_STYLE)
                {
                    "Pattern"
                }

                input
                    id="pattern"
                    type="text"
                    name="pattern"
                    placeholder="e.g., starbucks"
                    required
                    autofocus
                    class=(FORM_TEXT_INPUT_STYLE);

                p class="mt-1 text-xs text-gray-500 dark:text-gray-400"
                {
                    "Transaction descriptions starting with this pattern will be tagged (case-insensitive)"
                }
            }

            div
            {
                label
                    for="tag_id"
                    class=(FORM_LABEL_STYLE)
                {
                    "Tag"
                }

                select
                    id="tag_id"
                    name="tag_id"
                    required
                    class=(FORM_TEXT_INPUT_STYLE)
                {
                    option value="" { "Select a tag" }

                    @for tag in available_tags {
                        option value=(tag.id) { (tag.name) }
                    }
                }
            }

            @if !error_message.is_empty() {
                p class="text-red-600 dark:text-red-400"
                {
                    (error_message)
                }
            }

            button
                type="submit"
                class="w-full px-4 py-2 bg-blue-500 dark:bg-blue-600 disabled:bg-blue-700
                    hover:enabled:bg-blue-600 hover:enabled:dark:bg-blue-700 text-white rounded"
            {
                "Create Rule"
            }
        }
    }
}
