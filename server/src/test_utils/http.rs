use axum::{body::Body, http::StatusCode, response::Response};

#[track_caller]
pub(crate) fn assert_status_ok(response: &Response<Body>) {
    assert_eq!(response.status(), StatusCode::OK);
}

#[track_caller]
pub(crate) fn assert_content_type(response: &Response<Body>, content_type: &str) {
    let content_type_header = response
        .headers()
        .get("content-type")
        .expect("content-type header missing");
    assert_eq!(content_type_header, content_type);
}

#[track_caller]
pub(crate) fn get_header(response: &Response<Body>, header_name: &str) -> String {
    let header_error_message = format!("Headers missing {header_name}");

    response
        .headers()
        .get(header_name)
        .expect(&header_error_message)
        .to_str()
        .expect("Could not convert to str")
        .to_string()
}

#[track_caller]
pub(crate) fn assert_hx_redirect(response: &Response<Body>, endpoint: &str) {
    assert_eq!(get_header(response, "hx-redirect"), endpoint);
}
