//! This file defines the templates and a convenience function for creating the navigation bar.
use askama::Template;

use crate::routes::endpoints;

/// Template for a link in the navigation bar.
///
/// It will change appearance if `is_current` is set to
/// `true`. Only one link should be set as active at any one time.
#[derive(Template)]
#[template(path = "components/nav_link.html")]
struct Link<'a> {
    url: &'a str,
    title: &'a str,
    is_current: bool,
}

/// Template for the navigation bar which includes links to pages and a log out button.
#[derive(Template)]
#[template(path = "partials/nav_bar.html")]
pub struct NavbarTemplate<'a> {
    links: Vec<Link<'a>>,
}

/// Get the navigation bar.
///
/// If a link matches `active_endpoint`, then that link will be
/// marked as active and displayed differently in the HTML.
pub fn get_nav_bar(active_endpoint: &str) -> NavbarTemplate {
    let links = vec![
        Link {
            url: endpoints::DASHBOARD_VIEW,
            title: "Dashboard",
            is_current: active_endpoint == endpoints::DASHBOARD_VIEW,
        },
        Link {
            url: endpoints::TRANSACTIONS_VIEW,
            title: "Transactions",
            is_current: active_endpoint == endpoints::TRANSACTIONS_VIEW,
        },
        Link {
            url: endpoints::LOG_OUT,
            title: "Log out",
            is_current: false,
        },
    ];

    NavbarTemplate { links }
}

#[cfg(test)]
mod nav_bar_tests {
    use std::collections::HashMap;

    use crate::routes::endpoints;

    use super::get_nav_bar;

    #[test]
    fn set_active_endpoint() {
        let mut cases = HashMap::new();
        cases.insert(endpoints::DASHBOARD_VIEW, true);
        cases.insert(endpoints::TRANSACTIONS_VIEW, true);

        cases.insert(endpoints::ROOT, false);
        cases.insert(endpoints::COFFEE, false);
        cases.insert(endpoints::CATEGORY, false);
        cases.insert(endpoints::CATEGORIES, false);
        cases.insert(endpoints::INTERNAL_ERROR_VIEW, false);
        cases.insert(endpoints::LOG_IN_API, false);
        cases.insert(endpoints::LOG_IN_VIEW, false);
        cases.insert(endpoints::LOG_OUT, false);
        cases.insert(endpoints::REGISTER_VIEW, false);
        cases.insert(endpoints::TRANSACTION, false);
        cases.insert(endpoints::TRANSACTIONS_API, false);
        cases.insert(endpoints::USER_CATEGORIES, false);
        cases.insert(endpoints::USERS, false);

        let get_active_string = |is_active: bool| -> &str {
            if is_active {
                "active (true)"
            } else {
                "inactive (false)"
            }
        };

        for (endpoint, should_be_active) in cases {
            let nav_bar = get_nav_bar(endpoint);

            for link in nav_bar.links {
                if link.url == endpoint {
                    assert_eq!(
                        link.is_current,
                        should_be_active,
                        "Link for current page should be {} but got {}",
                        get_active_string(should_be_active),
                        get_active_string(link.is_current),
                    )
                } else {
                    assert!(
                        !link.is_current,
                        "Link for inactive page should {} but got {}",
                        get_active_string(false),
                        get_active_string(link.is_current)
                    )
                }
            }
        }
    }
}
