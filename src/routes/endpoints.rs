/*! The API endpoints URIs. */

/// The route to request a cup of coffee (experimental).
pub const COFFEE: &str = "/coffee";
/// The landing page for logged in users.
pub const DASHBOARD: &str = "/dashboard";
/// The root route which redirects to the dashboard or log in page.
pub const ROOT: &str = "/";
/// The route for getting log in page and logging in a user.
pub const LOG_IN: &str = "/log_in";
/// The route for the client to log out the current user.
pub const LOG_OUT: &str = "/log_out";
/// The route for getting the registration page and registering new users.
pub const REGISTER: &str = "/register";
/// The route to access users.
pub const USERS: &str = "/users";
/// The route to access the categories for a given user.
pub const USER_CATEGORIES: &str = "/users/:user_id/categories";
/// The route to access the transactions for a given user.
pub const USER_TRANSACTIONS: &str = "/users/:user_id/transactions";
/// The route to access categories.
pub const CATEGORIES: &str = "/categories";
/// The route to access a single category.
pub const CATEGORY: &str = "/categories/:category_id";
/// The route to access transactions.
pub const TRANSACTIONS: &str = "/transactions";
/// The route to access a single transaction.
pub const TRANSACTION: &str = "/transactions/:transaction_id";
/// The page to display when an internal server error occurs.
pub const INTERNAL_ERROR: &str = "/error";

// These tests are here so that we know when we call `Uri::from_shared` it will not panic.
#[cfg(test)]
mod endpoints_tests {
    use axum::http::Uri;

    use crate::routes::endpoints;

    fn assert_endpoint_is_valid_uri(uri: &str) {
        assert!(uri.parse::<Uri>().is_ok());
    }

    #[test]
    fn endpoints_are_valid_uris() {
        assert_endpoint_is_valid_uri(endpoints::CATEGORIES);
        assert_endpoint_is_valid_uri(endpoints::CATEGORY);
        assert_endpoint_is_valid_uri(endpoints::COFFEE);
        assert_endpoint_is_valid_uri(endpoints::DASHBOARD);
        assert_endpoint_is_valid_uri(endpoints::LOG_IN);
        assert_endpoint_is_valid_uri(endpoints::LOG_OUT);
        assert_endpoint_is_valid_uri(endpoints::REGISTER);
        assert_endpoint_is_valid_uri(endpoints::ROOT);
        assert_endpoint_is_valid_uri(endpoints::USERS);
        assert_endpoint_is_valid_uri(endpoints::USER_CATEGORIES);
        assert_endpoint_is_valid_uri(endpoints::USER_TRANSACTIONS);
    }
}
