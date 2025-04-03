use askama_axum::Template;
use axum::response::{IntoResponse, Response};

use crate::routes::{
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
};

/// Renders the new Category page.
#[derive(Template)]
#[template(path = "views/new_category.html")]
struct NewCategoryTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
}

pub async fn get_new_category_page() -> Response {
    NewCategoryTemplate {
        nav_bar: get_nav_bar(endpoints::NEW_CATEGORY_VIEW),
    }
    .into_response()
}

#[cfg(test)]
mod new_category_tests {
    use axum::{http::StatusCode, response::Response};
    use scraper::Html;

    use crate::routes::views::new_category::get_new_category_page;

    #[tokio::test]
    async fn render_page() {
        let response = get_new_category_page().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .expect("content-type header missing"),
            "text/html; charset=utf-8"
        );

        let html = parse_html(response).await;
        assert_valid_html(&html);

        // TODO: check that there is a form
        // TODO: check that the form has a hx-post attribute to the correct endpoint
        // TODO: check that the form has a text input called 'name'
        // TODO: check that the form has a submit button
    }

    #[tokio::test]
    async fn error_on_invalid_name() {
        // TODO: check that submitting an empty name results in an error being displayed
    }

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_document(&text)
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }
}
