use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    shared_templates::render,
    tag::{Tag, TagsPageState, get_all_tags},
    transaction_tag::get_tag_transaction_count,
};

/// A tag with its formatted edit URL for template rendering.
#[derive(Debug, Clone)]
struct TagWithEditUrl {
    pub tag: Tag,
    pub edit_url: String,
    pub transaction_count: i64,
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

    // TODO: Use single query to get transaction count by tag.
    let tags_with_edit_urls = tags
        .into_iter()
        .map(|tag| {
            let transaction_count = get_tag_transaction_count(tag.id, &connection).unwrap_or(0); // Default to 0 if count query fails
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
