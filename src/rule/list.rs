use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use maud::{Markup, html};

use crate::{
    Error, endpoints,
    html::{
        BUTTON_DELETE_STYLE, LINK_STYLE, PAGE_CONTAINER_STYLE, TABLE_CELL_STYLE,
        TABLE_HEADER_STYLE, TABLE_ROW_STYLE, TAG_BADGE_STYLE, base, dollar_input_styles,
        loading_spinner,
    },
    navigation::NavBar,
    rule::{
        db::get_all_rules_with_tags,
        models::{RuleState, RuleWithTag},
    },
};

/// Route handler for the rules listing page.
pub async fn get_rules_page(State(state): State<RuleState>) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let rules = get_all_rules_with_tags(&connection)
        .inspect_err(|error| tracing::error!("Failed to retrieve rules: {error}"))?;

    Ok(rules_view(&rules).into_response())
}

fn rules_view(rules: &[RuleWithTag]) -> Markup {
    let auto_tag_all_route = endpoints::AUTO_TAG_ALL;
    let auto_tag_untagged_route = endpoints::AUTO_TAG_UNTAGGED;
    let new_rule_route = endpoints::NEW_RULE_VIEW;

    let nav_bar = NavBar::new(endpoints::RULES_VIEW).into_html();
    let spinner = loading_spinner();

    let table_row = |rule: &RuleWithTag| {
        html!(
            tr class=(TABLE_ROW_STYLE)
            {
                td class=(TABLE_CELL_STYLE)
                {
                    code class="bg-gray-100 dark:bg-gray-700 px-2.5 py-0.5 rounded-sm text-xs"
                    {
                        (rule.rule.pattern)
                    }
                }

                td class=(TABLE_CELL_STYLE)
                {
                    span class=(TAG_BADGE_STYLE)
                    {
                        (rule.tag.name)
                    }
                }

                td class=(TABLE_CELL_STYLE)
                {
                    div class="flex gap-4"
                    {
                        a href=(rule.edit_url) class=(LINK_STYLE)
                        {
                            "Edit"
                        }

                        button
                            hx-delete=(rule.delete_url)
                            hx-confirm={
                                "Are you sure you want to delete the rule '"
                                (rule.rule.pattern) "' → '" (rule.tag.name) "'?"
                            }
                            hx-target="closest tr"
                            hx-target-error="#alert-container"
                            hx-swap="delete"
                            class=(BUTTON_DELETE_STYLE)
                        {
                           "Delete"
                        }
                    }
                }
            }
        )
    };

    let content = html!(
        (nav_bar)

        div class=(PAGE_CONTAINER_STYLE)
        {
            div class="relative space-y-4"
            {
                h1 class="text-xl font-bold" { "Auto-Tagging Rules" }

                @if !rules.is_empty() {
                    div class="flex gap-4"
                    {
                        button
                            hx-post=(auto_tag_all_route)
                            hx-confirm="Apply all rules to every transaction? This may take a moment for large datasets."
                            hx-indicator="#loading-all-indicator"
                            hx-swap="none"
                            class="px-4 py-2 bg-blue-600 hover:bg-blue-700
                                disabled:opacity-50 text-white text-sm font-medium
                                rounded transition-colors focus:outline-hidden
                                focus:ring-2 focus:ring-blue-500 focus:ring-offset-2
                                flex items-center"
                        {
                            span
                                id="loading-all-indicator"
                                class="htmx-indicator"
                                style="display: none;"
                            {
                                (spinner)
                            }

                            span class="button-text" { "Tag All Transactions" }
                        }

                        button
                            hx-post=(auto_tag_untagged_route)
                            hx-confirm="Apply rules only to transactions that currently have no tags?"
                            hx-indicator="#loading-untagged-indicator"
                            hx-swap="none"
                            class="px-4 py-2 bg-green-600 hover:bg-green-700
                                disabled:opacity-50 text-white text-sm
                                font-medium rounded transition-colors
                                focus:outline-hidden focus:ring-2
                                focus:ring-green-500 focus:ring-offset-2 flex
                                items-center"
                        {
                            span
                                id="loading-untagged-indicator"
                                class="htmx-indicator"
                                style="display: none;"
                            {
                                (spinner)
                            }

                            span class="button-text" { "Tag Untagged Transactions" }
                        }
                    }
                }

                div class="p-4 bg-blue-50 dark:bg-blue-900/20 rounded"
                {
                    h3 class="text-sm font-medium text-blue-800 dark:text-blue-200 mb-2"
                    {
                        "How Rules Work"
                    }

                    p class="text-xs text-blue-700 dark:text-blue-300"
                    {
                        r#"Rules automatically tag transactions whose descriptions start with the
                        specified pattern (case-insensitive) when importing from a CSV. 
                        For example, a rule with pattern "starbucks" will match
                        "Starbucks Downtown" or "STARBUCKS #1234".
                        Use the buttons above to manually apply all rules to your transactions."#
                    }
                }

                div class="flex justify-between flex-wrap items-end"
                {
                    a href=(new_rule_route) class=(LINK_STYLE)
                    {
                        "Create Rule"
                    }
                }

                div class="dark:bg-gray-800"
                {
                    table class="w-full text-sm text-left rtl:text-right
                        text-gray-500 dark:text-gray-400"
                    {
                        thead class=(TABLE_HEADER_STYLE)
                        {
                            tr
                            {
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Pattern"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Tag"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Actions"
                                }
                            }
                        }

                        tbody
                        {
                            @for rule_with_tag in rules {
                                (table_row(rule_with_tag))
                            }

                            @if rules.is_empty() {
                                tr
                                {
                                    td
                                        colspan="4"
                                        class="px-6 py-4 text-center
                                            text-gray-500 dark:text-gray-400"
                                    {
                                        "No rules created yet. "
                                        a href=(new_rule_route) class=(LINK_STYLE)
                                        {
                                            "Create your first rule"
                                        }
                                        " to automatically tag transactions."
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    );

    base("Rules", &[dollar_input_styles()], &content)
}
