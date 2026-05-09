use axum::{body::Body, response::Response};
use scraper::Html;

pub(crate) async fn parse_html_document(response: Response<Body>) -> Html {
    let body = response.into_body();
    let body = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("Could not get response body");
    let text = String::from_utf8_lossy(&body).to_string();

    Html::parse_document(&text)
}

pub(crate) async fn parse_html_fragment(response: Response<Body>) -> Html {
    let body = response.into_body();
    let body = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("Could not get response body");
    let text = String::from_utf8_lossy(&body).to_string();

    Html::parse_fragment(&text)
}

#[track_caller]
pub(crate) fn assert_valid_html(html: &Html) {
    assert!(
        html.errors.is_empty(),
        "Got HTML parsing errors: {:?}",
        html.errors
    );
}
