//! Middleware for logging requests and responses.

use axum::{extract::Request, http::header::CONTENT_TYPE, middleware::Next, response::Response};

/// Log the request and response for each request.
///
/// Both the request and response are logged at the `info` level.
/// If the response body is longer than [LOG_BODY_LENGTH_LIMIT] bytes, it is
/// truncated and logged at the `debug` level.
pub async fn logging_middleware(request: Request, next: Next) -> Response {
    let (headers, body_text) = extract_header_and_body_text_from_request(request).await;

    if headers.method.eq(&axum::http::Method::POST)
        && headers.headers.get(CONTENT_TYPE)
            == Some(&"application/x-www-form-urlencoded".parse().unwrap())
    {
        let display_text = redact_password(&body_text, "password");
        let display_text = redact_password(&display_text, "confirm_password");
        log_request(&headers, &display_text);
    } else {
        log_request(&headers, &body_text);
    }

    let request = Request::from_parts(headers, body_text.into());
    let response = next.run(request).await;

    let (headers, body_text) = extract_header_and_body_text_from_response(response).await;
    log_response(&headers, &body_text);

    Response::from_parts(headers, body_text.into())
}

fn redact_password(form_text: &str, field_name: &str) -> String {
    let password_start = form_text.find(&format!("{}=", field_name));

    let start = match password_start {
        Some(password_pos) => password_pos,
        None => return form_text.to_string(),
    };

    let password_end = form_text[start..].find('&');
    let end = match password_end {
        Some(end) => start + end,
        None => form_text.len(),
    };
    let password = &form_text[start..end];

    form_text.replace(password, &format!("{}=********", field_name))
}

async fn extract_header_and_body_text_from_request(
    request: Request,
) -> (axum::http::request::Parts, String) {
    let (headers, body) = request.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();

    (headers, String::from_utf8_lossy(&body_bytes).to_string())
}

async fn extract_header_and_body_text_from_response(
    response: Response,
) -> (axum::http::response::Parts, String) {
    let (headers, body) = response.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX).await.unwrap();

    (headers, String::from_utf8_lossy(&body_bytes).to_string())
}

const LOG_BODY_LENGTH_LIMIT: usize = 64;

fn log_request(headers: &axum::http::request::Parts, body: &str) {
    if body.len() > LOG_BODY_LENGTH_LIMIT {
        tracing::info!(
            "Received request: {headers:#?}\nbody: {:}...",
            &body[..LOG_BODY_LENGTH_LIMIT]
        );
        tracing::debug!("Full request body: {body:?}");
    } else {
        tracing::info!("Received request: {headers:#?}\nbody: {body:?}");
    }
}

fn log_response(headers: &axum::http::response::Parts, body: &str) {
    if body.len() > LOG_BODY_LENGTH_LIMIT {
        tracing::info!(
            "Sending response: {headers:#?}\nbody: {:}...",
            &body[..LOG_BODY_LENGTH_LIMIT]
        );
        tracing::debug!("Full response body: {body:?}");
    } else {
        tracing::info!("Sending response: {headers:#?}\nbody: {body:?}");
    }
}
