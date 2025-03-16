use askama::Template;
use axum::response::{IntoResponse, Response};

use crate::routes::{
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
};

/// Renders the dashboard page.
#[derive(Template)]
#[template(path = "views/new_transaction.html")]
struct NewTransactionTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    create_transaction_route: &'a str,
}

/// Renders the page for creating a transaction.
pub async fn get_new_transaction_page() -> Response {
    let nav_bar = get_nav_bar(endpoints::NEW_TRANSACTION_VIEW);

    NewTransactionTemplate {
        nav_bar,
        create_transaction_route: endpoints::TRANSACTIONS_API,
    }
    .into_response()
}

#[cfg(test)]
mod new_transaction_route_tests {
    use axum::{
        body::Body,
        http::{StatusCode, response::Response},
    };

    use crate::routes::endpoints;

    use super::get_new_transaction_page;

    #[tokio::test]
    async fn returns_html() {
        let result = get_new_transaction_page().await;

        assert_eq!(result.status(), StatusCode::OK);
        assert_eq!(
            result
                .headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap(),
            "text/html; charset=utf-8"
        );

        let document = parse_html(result).await;
        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());

        let form = forms.first().unwrap();
        let hx_post = form.value().attr("hx-post");
        assert_eq!(
            hx_post,
            Some(endpoints::TRANSACTIONS_API),
            "want form with attribute hx-post=\"{}\", got {:?}",
            endpoints::TRANSACTIONS_API,
            hx_post
        );

        // TODO: check that form has all the fields for a transaction.
    }

    async fn parse_html(response: Response<Body>) -> scraper::Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        scraper::Html::parse_document(&text)
    }
}
