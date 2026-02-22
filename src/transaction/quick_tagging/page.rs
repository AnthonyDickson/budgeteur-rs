use std::sync::{Arc, Mutex};

use axum::{
    extract::{FromRef, State},
    response::{IntoResponse, Response},
};
use maud::{Markup, PreEscaped, html};
use rusqlite::Connection;

use super::queue::{
    QUICK_TAGGING_QUEUE_PAGE_SIZE, UntaggedTransactionRow, get_untagged_transactions,
};
use crate::{
    AppState, Error, endpoints,
    html::{
        BUTTON_PRIMARY_STYLE, FORM_DISMISS_LABEL_STYLE, FORM_RADIO_LABEL_STYLE, HeadElement,
        LINK_STYLE, PAGE_CONTAINER_STYLE, base, format_currency,
    },
    navigation::NavBar,
    tag::{Tag, get_all_tags},
};

const PAGE_TITLE: &str = "Quick Tagging";

/// The state needed for the untagged queue page.
#[derive(Debug, Clone)]
pub struct QuickTaggingQueueState {
    /// The database connection for managing transactions.
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for QuickTaggingQueueState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

pub async fn get_quick_tagging_page(
    State(state): State<QuickTaggingQueueState>,
) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let queue_rows = get_untagged_transactions(QUICK_TAGGING_QUEUE_PAGE_SIZE, &connection)
        .inspect_err(|error| tracing::error!("could not fetch untagged queue: {error}"))?;
    let tags = get_all_tags(&connection)
        .inspect_err(|error| tracing::error!("could not get tags: {error}"))?;

    Ok(quick_tagging_view(&queue_rows, &tags).into_response())
}

pub fn quick_tagging_queue_content(queue_rows: &[UntaggedTransactionRow], tags: &[Tag]) -> Markup {
    let transactions_route = endpoints::TRANSACTIONS_VIEW;
    let import_route = endpoints::IMPORT_VIEW;

    if queue_rows.is_empty() {
        return html! {
            section class="rounded bg-white dark:bg-gray-800 px-6 py-8 text-center space-y-4"
            {
                h1 class="text-2xl font-bold" { (PAGE_TITLE) }
                p class="text-gray-600 dark:text-gray-300"
                {
                    "All done, no untagged transactions left"
                }
                div class="flex flex-wrap justify-center gap-4"
                {
                    a href=(transactions_route) class=(LINK_STYLE)
                    {
                        "Back to Transactions"
                    }
                    a href=(import_route) class=(LINK_STYLE)
                    {
                        "Import Transactions"
                    }
                }
            }
        };
    }

    html! {
        section class="space-y-4"
        {
            header class="flex flex-wrap items-end justify-between gap-2"
            {
                h1 class="text-xl font-bold" { (PAGE_TITLE) }
            }

            form
                hx-post=(endpoints::QUICK_TAGGING_APPLY)
                hx-target="#untagged-queue-content"
                hx-target-error="#alert-container"
                hx-swap="innerHTML"
                class="space-y-4"
            {
                div class="space-y-4"
                {
                    @for row in queue_rows {
                        section class="rounded bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 p-4 space-y-3"
                        {
                            div class="space-y-1"
                            {
                                p class="text-base font-semibold text-gray-900 dark:text-white"
                                {
                                    (row.description)
                                }
                                div class="flex items-center justify-between gap-3 text-sm text-gray-600 dark:text-gray-300"
                                {
                                    span class="font-medium"
                                    {
                                        (row.date)
                                    }
                                    div class="text-sm font-semibold text-right"
                                    {
                                        span class=(amount_class(row.amount))
                                        {
                                            (format_currency(row.amount))
                                        }
                                    }
                                }
                            }

                            div class="space-y-2"
                            {
                                p class="text-xs uppercase tracking-wide text-gray-500"
                                {
                                    "Tags"
                                }

                                div class="flex flex-wrap gap-2"
                                {
                                    @if tags.is_empty() {
                                        span class="text-xs text-gray-500" { "No tags available" }
                                    } @else {
                                        @for tag in tags {
                                            @let input_id = format!("tag-{}-{}", row.id, tag.id);
                                            div class="flex items-center gap-2"
                                            {
                                                input
                                                    id=(input_id.clone())
                                                    type="radio"
                                                    name={(format!("tag_id_{}", row.id))}
                                                    value=(tag.id)
                                                    data-transaction-id=(row.id)
                                                    class="peer sr-only";
                                                label
                                                    for=(input_id)
                                                    class=(FORM_RADIO_LABEL_STYLE)
                                                {
                                                    (tag.name)
                                                }
                                            }
                                        }
                                    }

                                    @let dismiss_id = format!("dismiss-{}", row.id);
                                    div class="flex items-center gap-2"
                                    {
                                        input
                                            id=(dismiss_id.clone())
                                            type="checkbox"
                                            name="dismiss"
                                            value=(row.id)
                                            data-transaction-id=(row.id)
                                            class="peer sr-only";
                                        label
                                            for=(dismiss_id)
                                            class=(FORM_DISMISS_LABEL_STYLE)
                                        {
                                            "Dismiss"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                div class="flex justify-end"
                {
                    button
                        id="untagged-apply-button"
                        type="submit"
                        class=(BUTTON_PRIMARY_STYLE)
                    {
                        "Apply changes"
                    }
                }
            }
        }
    }
}

pub fn quick_tagging_view(queue_rows: &[UntaggedTransactionRow], tags: &[Tag]) -> Markup {
    let nav_bar = NavBar::new(endpoints::QUICK_TAGGING_VIEW).into_html();
    let head_elements = [HeadElement::ScriptSource(PreEscaped(
        include_str!("input_guard.js").to_owned(),
    ))];

    let content = html! {
        (nav_bar)

        main class=(PAGE_CONTAINER_STYLE)
        {
            section id="untagged-queue-content" class="w-full max-w-5xl mx-auto"
            {
                (quick_tagging_queue_content(queue_rows, tags))
            }
        }
    };

    base(PAGE_TITLE, &head_elements, &content)
}

fn amount_class(amount: f64) -> &'static str {
    if amount < 0.0 {
        "text-red-700 dark:text-red-300"
    } else {
        "text-green-700 dark:text-green-300"
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::extract::State;
    use rusqlite::Connection;
    use time::{OffsetDateTime, macros::date};

    use crate::{
        db::initialize,
        endpoints,
        tag::{TagName, create_tag},
        test_utils::{assert_valid_html, form::assert_hx_endpoint, parse_html_document},
        transaction::{Transaction, create_transaction, insert_untagged_transactions_for_import},
    };

    use super::{QuickTaggingQueueState, get_quick_tagging_page};

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[tokio::test]
    async fn page_shows_empty_state_when_queue_empty() {
        let conn = get_test_connection();
        let state = QuickTaggingQueueState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = get_quick_tagging_page(State(state)).await.unwrap();
        let html = parse_html_document(response).await;

        assert_valid_html(&html);
        let content = html.root_element().text().collect::<Vec<_>>().join("");
        assert!(
            content.contains("All done, no untagged transactions left"),
            "expected empty-state message"
        );
    }

    #[tokio::test]
    async fn page_renders_apply_form_when_queue_has_rows() {
        let conn = get_test_connection();
        let tag = create_tag(TagName::new_unchecked("Groceries"), &conn).unwrap();
        let tx = create_transaction(
            Transaction::build(12.34, date!(2025 - 10 - 05), "queue"),
            &conn,
        )
        .unwrap();
        let created_at = OffsetDateTime::now_utc();
        insert_untagged_transactions_for_import(&[tx.id], created_at, &conn).unwrap();

        let state = QuickTaggingQueueState {
            db_connection: Arc::new(Mutex::new(conn)),
        };

        let response = get_quick_tagging_page(State(state)).await.unwrap();
        let html = parse_html_document(response).await;

        assert_valid_html(&html);
        let form = crate::test_utils::form::must_get_form(&html);
        assert_hx_endpoint(&form, endpoints::QUICK_TAGGING_APPLY, "hx-post");

        let tag_id = tag.id.to_string();
        let inputs = html
            .select(&scraper::Selector::parse("input[type=radio]").unwrap())
            .collect::<Vec<_>>();
        assert!(
            inputs
                .iter()
                .any(|input| input.value().attr("value") == Some(tag_id.as_str())),
            "expected tag chip input"
        );
    }
}
