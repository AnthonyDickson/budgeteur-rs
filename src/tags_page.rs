use std::collections::HashMap;

use axum::{
    extract::State,
    response::{IntoResponse, Response},
};
use maud::{Markup, html};
use rusqlite::Connection;

use crate::{
    Error, endpoints,
    navigation::NavBar,
    tag::{Tag, TagId, TagsPageState, get_all_tags},
    view_templates::base,
};

/// A tag with its formatted edit URL for template rendering.
#[derive(Debug, Clone)]
struct TagWithEditUrl {
    pub tag: Tag,
    pub edit_url: String,
    pub transaction_count: u64,
}

fn tags_view(tags: &[TagWithEditUrl]) -> Markup {
    let new_tag_route = endpoints::NEW_TAG_VIEW;
    let delete_tag_route =
        |tag: &TagWithEditUrl| endpoints::format_endpoint(endpoints::DELETE_TAG, tag.tag.id);
    let nav_bar = NavBar::new(endpoints::TAGS_VIEW).into_html();

    let table_row = |tag_with_url: &TagWithEditUrl| {
        html!(
            tr class="bg-white border-b dark:bg-gray-800 dark:border-gray-700"
            {
                td class="px-6 py-4"
                {
                    span
                        class="inline-flex items-center px-2.5 py-0.5 text-xs font-semibold text-blue-800
                        bg-blue-100 rounded-full dark:bg-blue-900 dark:text-blue-300"
                    {
                        (tag_with_url.tag.name)
                    }
                }

                td class="px-6 py-4"
                {
                    (tag_with_url.transaction_count)
                }

                td class="px-6 py-4"
                {
                    div class="flex gap-4"
                    {
                        a
                            href=(tag_with_url.edit_url)
                            class="text-blue-600 hover:text-blue-500 dark:text-blue-500 dark:hover:text-blue-400 underline"
                        {
                            "Edit"
                        }

                        button
                            hx-delete=(delete_tag_route(tag_with_url))
                            hx-confirm={
                                "Are you sure you want to delete '"
                                (tag_with_url.tag.name) "'? This will remove it from "
                                (tag_with_url.transaction_count) " transaction(s)."
                            }
                            hx-target="closest tr"
                            hx-target-error="#alert-container"
                            hx-swap="delete"
                            class="text-red-600 hover:text-red-500 dark:text-red-500 dark:hover:text-red-400
                               underline bg-transparent border-none cursor-pointer"
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

        div
            class="flex flex-col items-center px-6 py-8 mx-auto lg:py-5
            text-gray-900 dark:text-white"
        {
            div class="relative"
            {
                div class="flex justify-between flex-wrap items-end"
                {
                    h1 class="text-xl font-bold" { "Tags" }

                    a
                        href=(new_tag_route)
                        class="text-blue-600 hover:text-blue-500
                            dark:text-blue-500 dark:hover:text-blue-400 underline"
                    {
                        "Create Tag"
                    }
                }

                div class="dark:bg-gray-800"
                {
                    table class="w-full text-sm text-left rtl:text-right
                        text-gray-500 dark:text-gray-400"
                    {
                        thead
                            class="text-xs text-gray-700 uppercase bg-gray-50
                            dark:bg-gray-700 dark:text-gray-400"
                        {
                            tr
                            {
                                th scope="col" class="px-6 py-3"
                                {
                                    "Name"
                                }
                                th scope="col" class="px-6 py-3"
                                {
                                    "Transactions"
                                }
                                th scope="col" class="px-6 py-3"
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
                                        colspan="4"
                                        class="px-6 py-4 text-center
                                            text-gray-500 dark:text-gray-400"
                                    {
                                        "No tags created yet. "
                                        a
                                            href=(new_tag_route)
                                            class="text-blue-600
                                                hover:text-blue-500
                                                dark:text-blue-500
                                                dark:hover:text-blue-400
                                                underline"
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

/// Route handler for the tags listing page.
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

fn count_transactions_per_tag(connection: &Connection) -> Result<HashMap<TagId, u64>, Error> {
    let result: Result<HashMap<TagId, u64>, rusqlite::Error> = connection
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

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use time::OffsetDateTime;

    use crate::{
        tag::{TagName, create_tag, create_tag_table},
        tags_page::count_transactions_per_tag,
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
