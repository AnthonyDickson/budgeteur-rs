use axum::{body::Body, http::StatusCode, response::Response};
use scraper::{ElementRef, Html};

#[track_caller]
pub fn assert_status_ok(response: &Response<Body>) {
    assert_eq!(response.status(), StatusCode::OK);
}

#[track_caller]
pub fn assert_html_content_type(response: &Response<Body>) {
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/html; charset=utf-8"
    );
}

#[track_caller]
pub fn assert_valid_html(html: &Html) {
    assert!(
        html.errors.is_empty(),
        "Got HTML parsing errors: {:?}",
        html.errors
    );
}

pub async fn parse_html(response: Response<Body>) -> Html {
    let body = response.into_body();
    let body = axum::body::to_bytes(body, usize::MAX)
        .await
        .expect("Could not get response body");
    let text = String::from_utf8_lossy(&body).to_string();

    Html::parse_document(&text)
}

#[track_caller]
pub fn assert_transaction_type_inputs(form: &ElementRef, checked_type: Option<&str>) {
    let selector = scraper::Selector::parse("input[type=radio][name=type_]").unwrap();
    let inputs = form.select(&selector).collect::<Vec<_>>();
    assert_eq!(
        inputs.len(),
        2,
        "want 2 transaction type inputs, got {}",
        inputs.len()
    );

    let mut values = inputs
        .iter()
        .filter_map(|input| input.value().attr("value"))
        .collect::<Vec<_>>();
    values.sort_unstable();
    assert_eq!(
        values,
        vec!["expense", "income"],
        "want transaction type values to be expense/income, got {values:?}"
    );

    let checked_count = inputs
        .iter()
        .filter(|input| input.value().attr("checked").is_some())
        .count();
    assert_eq!(
        checked_count, 1,
        "want exactly one transaction type input checked, got {checked_count}"
    );

    for input in &inputs {
        let required = input.value().attr("required");
        let input_name = input.value().attr("name").unwrap_or("type_");
        assert!(
            required.is_some(),
            "want {input_name} input to be required, got {required:?}"
        );
    }

    if let Some(checked_type) = checked_type {
        let expected_checked = inputs.iter().any(|input| {
            input.value().attr("value") == Some(checked_type)
                && input.value().attr("checked").is_some()
        });
        assert!(
            expected_checked,
            "want {checked_type} to be checked, but it was not"
        );
    }
}
