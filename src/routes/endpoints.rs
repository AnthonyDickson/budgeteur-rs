/*! The API endpoints URIs. */

pub const COFFEE: &str = "/coffee";
pub const DASHBOARD: &str = "/dashboard";
pub const ROOT: &str = "/";
pub const LOG_IN: &str = "/log_in";
pub const LOG_OUT: &str = "/log_out";
pub const REGISTER: &str = "/register";
pub const USERS: &str = "/users";
pub const USER_CATEGORIES: &str = "/users/:user_id/categories";
pub const USER_TRANSACTIONS: &str = "/users/:user_id/transactions";
pub const CATEGORIES: &str = "/categories";
pub const CATEGORY: &str = "/categories/:category_id";
pub const TRANSACTIONS: &str = "/transactions";
pub const TRANSACTION: &str = "/transactions/:transaction_id";
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
