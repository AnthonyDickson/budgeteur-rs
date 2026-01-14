//! This file defines the templates and a convenience function for creating the navigation bar.

use askama::Template;
use axum::response::{IntoResponse, Response};
use maud::{Markup, html};

use crate::endpoints;

// TODO: OLD CODE: Remove after refactor complete

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
pub fn get_nav_bar(active_endpoint: &str) -> NavbarTemplate<'_> {
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
            url: endpoints::ACCOUNTS,
            title: "Accounts",
            is_current: active_endpoint == endpoints::ACCOUNTS,
        },
        Link {
            url: endpoints::TAGS_VIEW,
            title: "Tags",
            is_current: active_endpoint == endpoints::TAGS_VIEW,
        },
        Link {
            url: endpoints::RULES_VIEW,
            title: "Rules",
            is_current: active_endpoint == endpoints::RULES_VIEW,
        },
        Link {
            url: endpoints::LOG_OUT,
            title: "Log out",
            is_current: false,
        },
    ];

    NavbarTemplate { links }
}

// END: OLD CODE

impl Link<'_> {
    fn into_html(self) -> Markup {
        let Link {
            url,
            title,
            is_current,
        } = self;

        let style = if is_current {
            "block py-2 px-3 text-white bg-blue-700 rounded-sm md:bg-transparent
        md:text-blue-700 md:p-0 dark:text-white md:dark:text-blue-500"
        } else {
            "block py-2 px-3 text-gray-900 rounded-sm hover:bg-gray-100
        md:hover:bg-transparent md:border-0 md:hover:text-blue-700 md:p-0
        dark:text-white md:dark:hover:text-blue-500 dark:hover:bg-gray-700
        dark:hover:text-white md:dark:hover:bg-transparent"
        };

        html!( a href=(url) class=(style) { (title) } )
    }
}

pub struct NavBar<'a> {
    links: Vec<Link<'a>>,
}

impl NavBar<'_> {
    /// Get the navigation bar.
    ///
    /// If a link matches `active_endpoint`, then that link will be
    /// marked as active and displayed differently in the HTML.
    pub fn new(active_endpoint: &str) -> NavBar<'_> {
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
                url: endpoints::ACCOUNTS,
                title: "Accounts",
                is_current: active_endpoint == endpoints::ACCOUNTS,
            },
            Link {
                url: endpoints::TAGS_VIEW,
                title: "Tags",
                is_current: active_endpoint == endpoints::TAGS_VIEW,
            },
            Link {
                url: endpoints::RULES_VIEW,
                title: "Rules",
                is_current: active_endpoint == endpoints::RULES_VIEW,
            },
            Link {
                url: endpoints::LOG_OUT,
                title: "Log out",
                is_current: false,
            },
        ];

        NavBar { links }
    }

    pub fn into_html(self) -> Markup {
        let links = self.links;

        // Template adapted from https://flowbite.com/docs/components/navbar/#default-navbar
        html!(
            nav class="bg-white border-gray-200 dark:bg-gray-900"
            {
                div
                    class="max-w-screen-xl flex flex-wrap items-center justify-between mx-auto p-4"
                {
                    a
                        href="/"
                        class="flex items-center space-x-3 rtl:space-x-reverse"
                    {
                        img
                            src="/static/favicon-128x128.png"
                            alt="Budgeteur Logo"
                            class="h-8"
                        ;

                        span
                            class="self-center text-2xl font-semibold whitespace-nowrap dark:text-white"
                        {
                            "Budgeteur"
                        }
                    }

                    button
                        data-collapse-toggle="nav-bar-default"
                        type="button"
                        aria-controls="nav-bar-default"
                        aria-expanded="false"
                        class="inline-flex items-center p-2 w-10 h-10 justify-center
                        text-sm text-gray-500 rounded md:hidden
                        hover:bg-gray-100 focus:outline-hidden focus:ring-2
                        focus:ring-gray-200 dark:text-gray-400
                        dark:hover:bg-gray-700 dark:focus:ring-gray-600"
                    {
                        span class="sr-only" { "Open main menu" }
                        svg
                            viewBox="0 0 17 14"
                            fill="none"
                            class="w-5 h-5"
                            aria-hidden="true"
                            xmlns="http://www.w3.org/2000/svg"
                        {
                            path
                                stroke="currentColor"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                stroke-width="2"
                                d="M1 1h15M1 7h15M1 13h15"
                            {}
                        }
                    }
                    div class="hidden w-full md:block md:w-auto" id="nav-bar-default"
                    {
                        ul
                            class="font-medium flex flex-col p-4 md:p-0 mt-4
                            border border-gray-100 rounded bg-gray-50
                            md:flex-row md:space-x-8 rtl:space-x-reverse md:mt-0
                            md:border-0 md:bg-white dark:bg-gray-800
                            md:dark:bg-gray-900 dark:border-gray-700"
                        {
                            @for link in links {
                                li { (link.into_html()) }
                            }
                        }
                    }
                }
            }
        )
    }
}

impl IntoResponse for NavBar<'_> {
    fn into_response(self) -> Response {
        self.into_html().into_response()
    }
}

#[cfg(test)]
mod nav_bar_tests {
    use std::collections::HashMap;

    use crate::{
        endpoints,
        navigation::{NavBar, NavbarTemplate},
    };

    use super::get_nav_bar;

    #[test]
    fn set_active_endpoint() {
        let mut cases = HashMap::new();
        cases.insert(endpoints::DASHBOARD_VIEW, true);
        cases.insert(endpoints::TRANSACTIONS_VIEW, true);
        cases.insert(endpoints::ACCOUNTS, true);
        cases.insert(endpoints::TAGS_VIEW, true);
        cases.insert(endpoints::RULES_VIEW, true);

        cases.insert(endpoints::ROOT, false);
        cases.insert(endpoints::COFFEE, false);
        cases.insert(endpoints::POST_TAG, false);
        cases.insert(endpoints::INTERNAL_ERROR_VIEW, false);
        cases.insert(endpoints::LOG_IN_API, false);
        cases.insert(endpoints::LOG_IN_VIEW, false);
        cases.insert(endpoints::LOG_OUT, false);
        cases.insert(endpoints::REGISTER_VIEW, false);
        cases.insert(endpoints::TRANSACTIONS_API, false);
        cases.insert(endpoints::USERS, false);

        for (endpoint, should_be_active) in cases {
            let nav_bar = NavBar::new(endpoint);

            assert_link_active(nav_bar, endpoint, should_be_active);

            // TODO: OLD CODE: Remove after refactor complete
            let nav_bar = get_nav_bar(endpoint);

            assert_link_active_deprecated(nav_bar, endpoint, should_be_active);
            // END: OLD CODE
        }
    }

    #[track_caller]
    fn assert_link_active(nav_bar: NavBar<'_>, endpoint: &str, should_be_active: bool) {
        let get_active_string = |is_active: bool| -> &str {
            if is_active {
                "active (true)"
            } else {
                "inactive (false)"
            }
        };

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

    // TODO: OLD CODE: Remove after refactor complete
    #[track_caller]
    fn assert_link_active_deprecated(
        nav_bar: NavbarTemplate<'_>,
        endpoint: &str,
        should_be_active: bool,
    ) {
        let get_active_string = |is_active: bool| -> &str {
            if is_active {
                "active (true)"
            } else {
                "inactive (false)"
            }
        };

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
    // END: OLD CODE
}
