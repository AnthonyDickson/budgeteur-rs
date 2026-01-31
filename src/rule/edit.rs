use axum::{
    Form,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use maud::{Markup, html};

use crate::{
    Error,
    database_id::DatabaseId,
    endpoints,
    html::{FORM_LABEL_STYLE, FORM_TEXT_INPUT_STYLE, base},
    navigation::NavBar,
    rule::{
        db::{get_rule, update_rule},
        models::{RuleFormData, RuleState},
    },
    tag::{Tag, get_all_tags},
};

/// Route handler for the edit rule page.
pub async fn get_edit_rule_page(
    Path(rule_id): Path<DatabaseId>,
    State(state): State<RuleState>,
) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let available_tags = get_all_tags(&connection).inspect_err(|error| {
        tracing::error!("Failed to retrieve tags for edit rule page: {error}")
    })?;

    let edit_endpoint = endpoints::format_endpoint(endpoints::EDIT_RULE_VIEW, rule_id);
    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_RULE, rule_id);

    let rule = get_rule(rule_id, &connection).inspect_err(|error| match error {
        Error::NotFound => {}
        error => {
            tracing::error!("An unexpected error ocurred when fetching rule #{rule_id}: {error}");
        }
    })?;

    Ok(edit_rule_view(
        &edit_endpoint,
        &update_endpoint,
        &available_tags,
        &rule.pattern,
        rule.tag_id,
        "",
    )
    .into_response())
}

/// A route handler for updating a rule.
pub async fn update_rule_endpoint(
    Path(rule_id): Path<DatabaseId>,
    State(state): State<RuleState>,
    Form(form_data): Form<RuleFormData>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    let edit_endpoint = endpoints::format_endpoint(endpoints::EDIT_RULE_VIEW, rule_id);
    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_RULE, rule_id);

    if form_data.pattern.trim().is_empty() {
        let available_tags = get_all_tags(&connection).unwrap_or_default();

        return edit_rule_view(
            &edit_endpoint,
            &update_endpoint,
            &available_tags,
            &form_data.pattern,
            form_data.tag_id,
            "Error: Pattern cannot be empty",
        )
        .into_response();
    }

    let result = update_rule(
        rule_id,
        form_data.pattern.trim(),
        form_data.tag_id,
        &connection,
    );

    match result {
        Ok(_) => (
            HxRedirect(endpoints::RULES_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response(),
        Err(Error::UpdateMissingRule) => Error::UpdateMissingRule.into_alert_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while updating rule {rule_id}: {error}");
            error.into_alert_response()
        }
    }
}

fn edit_rule_view(
    edit_endpoint: &str,
    update_endpoint: &str,
    available_tags: &[Tag],
    rule_pattern: &str,
    selected_tag_id: DatabaseId,
    error_message: &str,
) -> Markup {
    let nav_bar = NavBar::new(edit_endpoint).into_html();
    let form = edit_rule_form_view(
        update_endpoint,
        available_tags,
        rule_pattern,
        selected_tag_id,
        error_message,
    );

    let content = html! {
        (nav_bar)

        div
            class="flex flex-col items-center px-6 py-8 mx-auto lg:py-0 max-w-md
            text-gray-900 dark:text-white"
        {
            (form)
        }
    };

    base("Edit Rule", &[], &content)
}

fn edit_rule_form_view(
    update_rule_endpoint: &str,
    available_tags: &[Tag],
    rule_pattern: &str,
    selected_tag_id: DatabaseId,
    error_message: &str,
) -> Markup {
    html! {
        form
            hx-put=(update_rule_endpoint)
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
                    value=(rule_pattern)
                    required
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
                    @for tag in available_tags {
                        option
                            value=(tag.id)
                            selected[tag.id == selected_tag_id]
                        {
                            (tag.name)
                        }
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
                "Update Rule"
            }
        }
    }
}
