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
#[template(path = "partials/navbar.html")]
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
            url: endpoints::DASHBOARD,
            title: "Dashboard",
            is_current: active_endpoint == endpoints::DASHBOARD,
        },
        Link {
            url: endpoints::TRANSACTIONS,
            title: "Transactions",
            is_current: active_endpoint == endpoints::TRANSACTIONS,
        },
        Link {
            url: endpoints::LOG_OUT,
            title: "Log out",
            is_current: false,
        },
    ];

    NavbarTemplate { links }
}
