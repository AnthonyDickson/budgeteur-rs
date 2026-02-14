//! This file defines the templates and a convenience function for creating the navigation bar.

use maud::{Markup, html};

use crate::endpoints;

/// Template for a link in the navigation bar.
///
/// It will change appearance if `is_current` is set to
/// `true`. Only one link should be set as active at any one time.
#[derive(Clone)]
struct Link<'a> {
    url: &'a str,
    title: &'a str,
    is_current: bool,
}

impl Link<'_> {
    fn into_desktop_html(self) -> Markup {
        let style = if self.is_current {
            "block py-2 px-3 text-white bg-blue-700 rounded-sm lg:bg-transparent
        lg:text-blue-700 lg:p-0 dark:text-white lg:dark:text-blue-500"
        } else {
            "block py-2 px-3 text-gray-900 rounded-sm hover:bg-gray-100
        lg:hover:bg-transparent lg:border-0 lg:hover:text-blue-700 lg:p-0
        dark:text-white lg:dark:hover:text-blue-500 dark:hover:bg-gray-700
        dark:hover:text-white lg:dark:hover:bg-transparent"
        };

        html!( a href=(self.url) class=(style) { (self.title) } )
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
        let more_is_active = links.iter().any(|link| {
            (link.url == endpoints::TAGS_VIEW || link.url == endpoints::RULES_VIEW)
                && link.is_current
        });
        let bottom_link_class = |is_current: bool| -> &'static str {
            if is_current {
                "flex w-full min-w-0 items-center justify-center rounded-lg \
                bg-blue-50 px-2.5 py-2 text-xs font-semibold leading-tight \
                text-blue-700 shadow-sm sm:px-4 sm:text-sm \
                dark:bg-blue-900/30 dark:text-blue-200"
            } else {
                "flex w-full min-w-0 items-center justify-center rounded-lg \
                px-2.5 py-2 text-xs font-semibold leading-tight text-gray-600 \
                sm:px-4 sm:text-sm \
                hover:bg-blue-50/70 hover:text-blue-700 dark:text-gray-300 \
                dark:hover:bg-blue-900/20 dark:hover:text-blue-200"
            }
        };
        let more_summary_class = |is_active: bool| -> &'static str {
            if is_active {
                "list-none [&::-webkit-details-marker]:hidden flex w-full min-w-0 \
                items-center justify-center rounded-lg bg-blue-50 px-2.5 py-2 \
                text-xs font-semibold leading-tight sm:px-4 sm:text-sm \
                text-blue-700 shadow-sm cursor-pointer \
                dark:bg-blue-900/30 dark:text-blue-200"
            } else {
                "list-none [&::-webkit-details-marker]:hidden flex w-full min-w-0 \
                items-center justify-center rounded-lg px-2.5 py-2 text-xs \
                font-semibold leading-tight sm:px-4 sm:text-sm \
                text-gray-600 cursor-pointer hover:bg-blue-50/70 hover:text-blue-700 \
                dark:text-gray-300 dark:hover:bg-blue-900/20 \
                dark:hover:text-blue-200"
            }
        };
        let more_item_class = |is_current: bool| -> &'static str {
            if is_current {
                "block rounded-lg bg-blue-50 px-3 py-2 text-blue-700 \
                dark:bg-blue-900/30 dark:text-blue-200"
            } else {
                "block rounded-lg px-3 py-2 text-gray-700 hover:bg-gray-100 \
                hover:text-blue-700 dark:text-gray-200 dark:hover:bg-gray-800/80 \
                dark:hover:text-blue-200"
            }
        };

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

                    div class="hidden w-full lg:block lg:w-auto"
                    {
                        ul
                            class="font-medium flex flex-col p-4 lg:p-0 mt-4
                            border border-gray-100 rounded bg-gray-50
                            lg:flex-row lg:space-x-8 rtl:space-x-reverse lg:mt-0
                            lg:border-0 lg:bg-white dark:bg-gray-800
                            lg:dark:bg-gray-900 dark:border-gray-700"
                        {
                            @for link in links.clone().into_iter() {
                                li { (link.into_desktop_html()) }
                            }
                        }
                    }
                }
            }

            nav class="fixed inset-x-0 bottom-0 z-40 lg:hidden"
            {
                div class="mx-auto max-w-screen-xl px-4 pb-4"
                {
                    div
                        class="rounded-xl border border-gray-200 bg-white/95
                        shadow-lg backdrop-blur dark:border-gray-700 dark:bg-gray-900/95"
                    {
                        ul
                            class="grid grid-cols-4 gap-2 px-4 py-3 text-xs font-semibold
                            text-gray-600 dark:text-gray-300"
                            aria-label="Primary"
                        {
                            @for link in links.iter() {
                                @if link.url == endpoints::DASHBOARD_VIEW
                                    || link.url == endpoints::TRANSACTIONS_VIEW
                                    || link.url == endpoints::ACCOUNTS
                                {
                                    li class="min-w-0" {
                                        a
                                            href=(link.url)
                                            class=(bottom_link_class(link.is_current))
                                            aria-current=[link.is_current.then_some("page")]
                                        {
                                            span class="truncate" { (link.title) }
                                        }
                                    }
                                }
                            }

                            li class="min-w-0" {
                                details
                                    class="group relative"
                                {
                                    summary
                                        class=(more_summary_class(more_is_active))
                                        aria-current=[more_is_active.then_some("page")]
                                    {
                                        span class="truncate" { "More" }
                                    }

                                    div
                                        class="absolute bottom-full right-0 mb-3 w-40 rounded-xl
                                        border border-gray-200 bg-white/95 p-2 shadow-xl
                                        backdrop-blur dark:border-gray-700 dark:bg-gray-900/95"
                                    {
                                        ul class="flex flex-col gap-1 text-sm font-medium"
                                        {
                                            @for link in links.iter() {
                                                @if link.url == endpoints::TAGS_VIEW
                                                    || link.url == endpoints::RULES_VIEW
                                                    || link.url == endpoints::LOG_OUT
                                                {
                                                    li {
                                                        a
                                                            href=(link.url)
                                                            class=(more_item_class(link.is_current))
                                                            aria-current=[link.is_current.then_some("page")]
                                                        {
                                                            (link.title)
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        )
    }
}

#[cfg(test)]
mod nav_bar_tests {
    use std::collections::HashMap;

    use crate::{endpoints, navigation::NavBar};

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
}
