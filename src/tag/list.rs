//! Tags listing page.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{FromRef, State},
    response::{IntoResponse, Response},
};
use maud::{Markup, html};
use rusqlite::Connection;

use crate::{
    AppState, Error, endpoints,
    html::{
        LINK_STYLE, PAGE_CONTAINER_STYLE, TABLE_CELL_STYLE, TABLE_HEADER_STYLE, TABLE_ROW_STYLE,
        TAG_BADGE_STYLE, base, edit_delete_action_links,
    },
    navigation::NavBar,
    tag::{Tag, TagId, get_all_tags},
};

/// The state needed for the tags listing page.
#[derive(Debug, Clone)]
pub struct TagsPageState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for TagsPageState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

/// A tag with its formatted edit URL for template rendering.
#[derive(Debug, Clone)]
struct TagWithEditUrl {
    pub tag: Tag,
    pub edit_url: String,
    pub transaction_count: u32,
}

/// Render the tags listing page with transaction counts.
pub async fn get_tags_page(State(state): State<TagsPageState>) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let tags = get_all_tags(&connection)
        .inspect_err(|error| tracing::error!("Failed to retrieve tags: {error}"))?;

    let transactions_per_tag = count_transactions_per_tag(&connection)
        .inspect_err(|error| tracing::error!("Could not count transactions per tag: {error}"))?;

    let tags_with_edit_urls = tags
        .into_iter()
        .map(|tag| {
            let transaction_count = *transactions_per_tag.get(&tag.id).unwrap_or(&0);

            TagWithEditUrl {
                edit_url: endpoints::format_endpoint(endpoints::EDIT_TAG_VIEW, tag.id),
                tag,
                transaction_count,
            }
        })
        .collect::<Vec<_>>();

    Ok(tags_view(&tags_with_edit_urls).into_response())
}

fn count_transactions_per_tag(connection: &Connection) -> Result<HashMap<TagId, u32>, Error> {
    let result: Result<HashMap<TagId, u32>, rusqlite::Error> = connection
        .prepare(
            "SELECT tag_id, COUNT(1) FROM \"transaction\" WHERE tag_id IS NOT NULL GROUP BY tag_id",
        )?
        .query_map((), |row| {
            let tag_id = row.get(0)?;
            let count = row.get(1)?;

            Ok((tag_id, count))
        })?
        .collect();

    result.map_err(Error::from)
}

fn tags_view(tags: &[TagWithEditUrl]) -> Markup {
    let new_tag_route = endpoints::NEW_TAG_VIEW;
    let nav_bar = NavBar::new(endpoints::TAGS_VIEW).into_html();

    let table_row = |tag_with_url: &TagWithEditUrl| {
        let delete_url = endpoints::format_endpoint(endpoints::DELETE_TAG, tag_with_url.tag.id);
        let confirm_message = format!(
            "Are you sure you want to delete '{}'? This will remove it from {} transaction(s).",
            tag_with_url.tag.name, tag_with_url.transaction_count
        );

        html!(
            tr class=(TABLE_ROW_STYLE)
            {
                td class=(TABLE_CELL_STYLE)
                {
                    span class=(TAG_BADGE_STYLE)
                    {
                        (tag_with_url.tag.name)
                    }
                }

                td class=(TABLE_CELL_STYLE)
                {
                    (tag_with_url.transaction_count)
                }

                td class=(TABLE_CELL_STYLE)
                {
                    div class="flex gap-4"
                    {
                        (edit_delete_action_links(
                            &tag_with_url.edit_url,
                            &delete_url,
                            &confirm_message,
                            "closest tr",
                            "delete",
                        ))
                    }
                }
            }
        )
    };

    let content = html!(
        (nav_bar)

        main class=(PAGE_CONTAINER_STYLE)
        {
            section class="space-y-4"
            {
                header class="flex justify-between flex-wrap items-end"
                {
                    h1 class="text-xl font-bold" { "Tags" }

                    a href=(new_tag_route) class=(LINK_STYLE)
                    {
                        "Create Tag"
                    }
                }

                (tags_cards_view(tags, new_tag_route))

                section class="hidden lg:block dark:bg-gray-800 lg:max-w-5xl lg:w-full lg:mx-auto"
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
                                    "Name"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Transactions"
                                }
                                th scope="col" class=(TABLE_CELL_STYLE)
                                {
                                    "Actions"
                                }
                            }
                        }

                        tbody
                        {
                            @for tag_with_url in tags {
                                (table_row(tag_with_url))
                            }

                            @if tags.is_empty() {
                                tr
                                {
                                    td
                                        colspan="3"
                                        class="px-6 py-4 text-center
                                            text-gray-500 dark:text-gray-400"
                                    {
                                        "No tags created yet. "
                                        a href=(new_tag_route) class=(LINK_STYLE)
                                        {
                                            "Create your first tag"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    );

    base("Tags", &[], &content)
}

fn tags_cards_view(tags: &[TagWithEditUrl], new_tag_route: &str) -> Markup {
    struct TagCardView<'a> {
        tag_name: &'a str,
        transaction_count: u32,
        edit_url: &'a str,
        delete_url: String,
        confirm_message: String,
    }

    let cards = tags
        .iter()
        .map(|tag_with_url| TagCardView {
            tag_name: tag_with_url.tag.name.as_ref(),
            transaction_count: tag_with_url.transaction_count,
            edit_url: &tag_with_url.edit_url,
            delete_url: endpoints::format_endpoint(endpoints::DELETE_TAG, tag_with_url.tag.id),
            confirm_message: format!(
                "Are you sure you want to delete '{}'? This will remove it from {} transaction(s).",
                tag_with_url.tag.name, tag_with_url.transaction_count
            ),
        })
        .collect::<Vec<_>>();

    html!(
        ul class="lg:hidden space-y-4"
        {
            @for card in &cards {
                li class="rounded border border-gray-200 bg-white px-4 py-3 shadow-sm dark:border-gray-700 dark:bg-gray-800"
                    data-tag-card="true"
                {
                    div class="flex items-start justify-between gap-3"
                    {
                        span class=(TAG_BADGE_STYLE) { (card.tag_name) }
                        span class="text-sm tabular-nums text-gray-900 dark:text-white"
                        { (card.transaction_count) }
                    }

                    div class="mt-2 flex items-center gap-4 text-sm"
                    {
                        (edit_delete_action_links(
                            card.edit_url,
                            &card.delete_url,
                            &card.confirm_message,
                            "closest [data-tag-card='true']",
                            "outerHTML",
                        ))
                    }
                }
            }

            @if cards.is_empty() {
                li class="rounded border border-dashed border-gray-300 bg-white px-4 py-6 text-center text-sm text-gray-500 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-400"
                {
                    "No tags created yet. "
                    a href=(new_tag_route) class=(LINK_STYLE)
                    {
                        "Create your first tag"
                    }
                }
            }
        }
    )
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use time::OffsetDateTime;

    use crate::{
        tag::{TagName, create_tag, create_tag_table, list::count_transactions_per_tag},
        transaction::{TransactionBuilder, create_transaction, create_transaction_table},
    };

    fn get_test_db_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        create_transaction_table(&connection).expect("Could not create transaction table");
        create_tag_table(&connection).expect("Could not create tag table");
        connection
    }

    #[test]
    fn test_counts_transactions_per_tag() {
        let connection = get_test_db_connection();
        let tag1 = create_tag(TagName::new_unchecked("foo"), &connection)
            .expect("Could not create test tag");
        let tag2 = create_tag(TagName::new_unchecked("bar"), &connection)
            .expect("Could not create test tag");
        let want_untagged_count = 10;
        let want_tag1_count = 20;
        let want_tag2_count = 30;
        for i in 0..want_untagged_count {
            create_transaction(
                TransactionBuilder {
                    amount: i as f64,
                    date: OffsetDateTime::now_utc().date(),
                    description: i.to_string(),
                    import_id: None,
                    tag_id: None,
                },
                &connection,
            )
            .unwrap();
        }
        for i in 0..want_tag1_count {
            create_transaction(
                TransactionBuilder {
                    amount: i as f64,
                    date: OffsetDateTime::now_utc().date(),
                    description: i.to_string(),
                    import_id: None,
                    tag_id: Some(tag1.id),
                },
                &connection,
            )
            .unwrap();
        }
        for i in 0..want_tag2_count {
            create_transaction(
                TransactionBuilder {
                    amount: i as f64,
                    date: OffsetDateTime::now_utc().date(),
                    description: i.to_string(),
                    import_id: None,
                    tag_id: Some(tag2.id),
                },
                &connection,
            )
            .unwrap();
        }

        let counts = count_transactions_per_tag(&connection).unwrap();

        assert_eq!(want_tag1_count, counts[&tag1.id]);
        assert_eq!(want_tag2_count, counts[&tag2.id]);
    }
}
