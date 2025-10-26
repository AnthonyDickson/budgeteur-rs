use std::collections::HashMap;

use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rusqlite::Connection;

use crate::{
    Error, endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    shared_templates::render,
    tag::{Tag, TagId, TagsPageState, get_all_tags},
};

/// A tag with its formatted edit URL for template rendering.
#[derive(Debug, Clone)]
struct TagWithEditUrl {
    pub tag: Tag,
    pub edit_url: String,
    pub transaction_count: u64,
}

/// Renders the tags listing page.
#[derive(Template)]
#[template(path = "views/tags.html")]
struct TagsTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    tags: Vec<TagWithEditUrl>,
    new_tag_route: &'a str,
}

/// Route handler for the tags listing page.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_tags_page(State(state): State<TagsPageState>) -> Response {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let tags = match get_all_tags(&connection) {
        Ok(tags) => tags,

        Err(error) => {
            tracing::error!("Failed to retrieve tags: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load tags").into_response();
        }
    };

    let transactions_per_tag = match count_transactions_per_tag(&connection) {
        Ok(transactions_per_tag) => transactions_per_tag,
        Err(error) => {
            tracing::error!("Could not count transactions per tag: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load tags").into_response();
        }
    };

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
        .collect();

    render(
        StatusCode::OK,
        TagsTemplate {
            nav_bar: get_nav_bar(endpoints::TAGS_VIEW),
            tags: tags_with_edit_urls,
            new_tag_route: endpoints::NEW_TAG_VIEW,
        },
    )
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
