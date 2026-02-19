//! Helpers for redirect URLs during authentication flows.

use axum::{extract::Request, http::Uri};
use tracing::{error, warn};

use crate::endpoints;

fn is_safe_redirect_url(redirect_url: &str) -> bool {
    if !redirect_url.starts_with('/') || redirect_url.starts_with("//") {
        return false;
    }

    let path = redirect_url
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(redirect_url);

    path != endpoints::LOG_IN_VIEW
}

pub fn normalize_redirect_url(raw_url: &str) -> Option<String> {
    let uri = raw_url.parse::<Uri>().ok()?;
    if uri.scheme().is_some() || uri.authority().is_some() {
        return None;
    }
    let path_and_query = uri.path_and_query()?.as_str();

    is_safe_redirect_url(path_and_query).then(|| path_and_query.to_owned())
}

fn normalize_hx_current_url(raw_url: &str) -> Option<String> {
    let uri = raw_url.parse::<Uri>().ok()?;
    let path_and_query = uri.path_and_query()?.as_str();

    is_safe_redirect_url(path_and_query).then(|| path_and_query.to_owned())
}

pub fn build_log_in_redirect_url(request: &Request) -> Option<String> {
    let redirect_target = if request.uri().path().starts_with("/api") {
        redirect_target_from_hx_request(request)?
    } else {
        redirect_target_from_request_uri(request)?
    };

    build_log_in_redirect_url_from_target(&redirect_target)
}

pub(super) fn build_log_in_redirect_url_from_target(redirect_target: &str) -> Option<String> {
    match serde_urlencoded::to_string([("redirect_url", redirect_target)]) {
        Ok(param) => Some(format!("{}?{}", endpoints::LOG_IN_VIEW, param)),
        Err(error) => {
            error!("Could not encode redirect URL {redirect_target}: {error}");
            None
        }
    }
}

fn redirect_target_from_request_uri(request: &Request) -> Option<String> {
    let path_and_query = request.uri().path_and_query()?.as_str();
    normalize_redirect_url(path_and_query)
}

fn redirect_target_from_hx_request(request: &Request) -> Option<String> {
    let headers = request.headers();
    let hx_request = headers
        .get("hx-request")
        .and_then(|header| header.to_str().ok())
        .map(|header| header.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if !hx_request {
        warn!("Missing HX-Request header for /api request.");
        return None;
    }

    let current_url = match headers
        .get("hx-current-url")
        .and_then(|header| header.to_str().ok())
    {
        Some(value) => value,
        None => {
            warn!("Missing HX-Current-URL header for /api request.");
            return None;
        }
    };

    let redirect_url = normalize_hx_current_url(current_url);
    if redirect_url.is_none() {
        warn!("Invalid HX-Current-URL header value: {current_url}");
    }

    redirect_url
}
