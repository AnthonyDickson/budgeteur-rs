//! The API endpoints URIs.
//!
//! For endpoints that take a parameter, e.g., '/users/:user_id', use [format_endpoint].

/// The root route which redirects to the dashboard or log in page.
pub const ROOT: &str = "/";
/// The landing page for logged in users.
pub const DASHBOARD_VIEW: &str = "/dashboard";
/// The page for displaying a user's transactions.
pub const TRANSACTIONS_VIEW: &str = "/transactions";
/// The route for getting the registration page.
pub const REGISTER_VIEW: &str = "/register";
/// The route for getting the log in page.
pub const LOG_IN_VIEW: &str = "/log_in";
/// The page to display when an internal server error occurs.
pub const INTERNAL_ERROR_VIEW: &str = "/error";

/// The route to request a cup of coffee (experimental).
pub const COFFEE: &str = "/api/coffee";
/// The route for logging in a user.
pub const LOG_IN_API: &str = "/api/log_in";
/// The route for the client to log out the current user.
pub const LOG_OUT: &str = "/api/log_out";
/// The route to access users.
pub const USERS: &str = "/api/users";
/// The route to access the categories for a given user.
pub const USER_CATEGORIES: &str = "/api/users/:user_id/categories";
/// The route to access the transactions for a given user.
pub const USER_TRANSACTIONS: &str = "/api/users/:user_id/transactions";
/// The route to access categories.
pub const CATEGORIES: &str = "/api/categories";
/// The route to access a single category.
pub const CATEGORY: &str = "/api/categories/:category_id";
/// The route to access transactions.
pub const TRANSACTIONS_API: &str = "/api/transactions";
/// The route to access a single transaction.
pub const TRANSACTION: &str = "/api/transactions/:transaction_id";

/// Replace the parameter in `endpoint_path` with `id`.
///
/// A parameter is a string that starts with a colon and is followed by
/// lowercase letters or underscores. For example, in the endpoint path
/// '/users/:user_id', ':user_id' is the parameter.
///
/// This function assumes that an endpoint path only contains ASCII characters
/// and a single parameter.
///
/// If no parameter is found in `endpoint_path`, the function returns the
/// the original `endpoint_path`.
///
/// # Examples
///
/// ```no_run
/// use budgeteur_rs::routes::endpoints::format_endpoint;
///
/// assert_eq!(format_endpoint("/users/:user_id", 42), "/users/42");
/// ```
///
pub fn format_endpoint(endpoint_path: &str, id: i64) -> String {
    let mut param_start = None;
    let mut param_end = None;

    for (i, c) in endpoint_path.chars().enumerate() {
        if c == ':' {
            param_start = Some(i);
        } else if param_start.is_some() && !c.is_ascii_lowercase() && c != '_' {
            param_end = Some(i);
            break;
        }
    }

    let param_start = match param_start {
        Some(start) => start,
        None => return endpoint_path.to_string(),
    };

    let param_end = param_end.unwrap_or(endpoint_path.len());

    format!(
        "{}{}{}",
        &endpoint_path[..param_start],
        id,
        &endpoint_path[param_end..]
    )
}

// These tests are here so that we know when we call `Uri::from_shared` it will not panic.
#[cfg(test)]
mod endpoints_tests {
    use axum::http::Uri;

    use crate::routes::endpoints;

    use super::format_endpoint;

    fn assert_endpoint_is_valid_uri(uri: &str) {
        assert!(uri.parse::<Uri>().is_ok());
    }

    #[test]
    fn endpoints_are_valid_uris() {
        assert_endpoint_is_valid_uri(endpoints::REGISTER_VIEW);
        assert_endpoint_is_valid_uri(endpoints::LOG_IN_VIEW);
        assert_endpoint_is_valid_uri(endpoints::DASHBOARD_VIEW);
        assert_endpoint_is_valid_uri(endpoints::TRANSACTIONS_VIEW);
        assert_endpoint_is_valid_uri(endpoints::INTERNAL_ERROR_VIEW);

        assert_endpoint_is_valid_uri(endpoints::COFFEE);
        assert_endpoint_is_valid_uri(endpoints::CATEGORIES);
        assert_endpoint_is_valid_uri(endpoints::CATEGORY);
        assert_endpoint_is_valid_uri(endpoints::LOG_IN_API);
        assert_endpoint_is_valid_uri(endpoints::LOG_OUT);
        assert_endpoint_is_valid_uri(endpoints::ROOT);
        assert_endpoint_is_valid_uri(endpoints::TRANSACTIONS_API);
        assert_endpoint_is_valid_uri(endpoints::USERS);
        assert_endpoint_is_valid_uri(endpoints::USER_CATEGORIES);
        assert_endpoint_is_valid_uri(endpoints::USER_TRANSACTIONS);
    }

    #[test]
    fn produces_valid_uri() {
        let formatted_path = format_endpoint("/hello/:world_id", 1);

        assert_eq!(formatted_path, "/hello/1");
        assert!(formatted_path.parse::<Uri>().is_ok());

        // Parameter with single word should also work.
        let formatted_path = format_endpoint("/hello/:world", 1);

        assert_eq!(formatted_path, "/hello/1");
        assert!(formatted_path.parse::<Uri>().is_ok());
    }

    #[test]
    fn returns_original_path_with_no_parameter() {
        let formatted_path = format_endpoint("/hello/world", 1);

        assert_eq!(formatted_path, "/hello/world");
        assert!(formatted_path.parse::<Uri>().is_ok());
    }

    #[test]
    fn parameter_in_middle() {
        let formatted_path = format_endpoint("/hello/:world/bye", 1);

        assert_eq!(formatted_path, "/hello/1/bye");
        assert!(formatted_path.parse::<Uri>().is_ok());
    }
}
