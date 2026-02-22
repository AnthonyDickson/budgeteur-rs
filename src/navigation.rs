//! This file defines the templates and a convenience function for creating the navigation bar.

use maud::{Markup, PreEscaped, html};

use crate::endpoints;

/// Template for a submenu link in the navigation bar.
#[derive(Clone)]
struct MenuLink<'a> {
    url: &'a str,
    title: &'a str,
    is_current: bool,
}

/// Template for a top-level navigation item.
#[derive(Clone)]
struct MenuItem<'a> {
    key: &'a str,
    url: &'a str,
    title: &'a str,
    is_current: bool,
    submenu: Vec<MenuLink<'a>>,
}

impl MenuItem<'_> {
    fn has_submenu(&self) -> bool {
        !self.submenu.is_empty()
    }
}

pub struct NavBar<'a> {
    items: Vec<MenuItem<'a>>,
}

impl NavBar<'_> {
    /// Get the navigation bar.
    ///
    /// If a link matches `active_endpoint`, then that link will be
    /// marked as active and displayed differently in the HTML.
    pub fn new(active_endpoint: &str) -> NavBar<'_> {
        let is_transactions = active_endpoint.starts_with(endpoints::TRANSACTIONS_VIEW);
        let is_accounts = active_endpoint.starts_with(endpoints::ACCOUNTS);
        let is_tags = active_endpoint.starts_with("/tag") || active_endpoint.starts_with("/tags");
        let is_rules = active_endpoint.starts_with(endpoints::RULES_VIEW);

        let transactions_submenu = vec![
            MenuLink {
                url: endpoints::TRANSACTIONS_VIEW,
                title: "Transactions",
                is_current: active_endpoint == endpoints::TRANSACTIONS_VIEW,
            },
            MenuLink {
                url: endpoints::NEW_TRANSACTION_VIEW,
                title: "Create Transaction",
                is_current: active_endpoint == endpoints::NEW_TRANSACTION_VIEW,
            },
            MenuLink {
                url: endpoints::IMPORT_VIEW,
                title: "Import Transactions",
                is_current: active_endpoint == endpoints::IMPORT_VIEW,
            },
            MenuLink {
                url: endpoints::QUICK_TAGGING_VIEW,
                title: "Quick Tagging",
                is_current: active_endpoint == endpoints::QUICK_TAGGING_VIEW,
            },
        ];

        let accounts_submenu = vec![
            MenuLink {
                url: endpoints::ACCOUNTS,
                title: "Accounts",
                is_current: active_endpoint == endpoints::ACCOUNTS,
            },
            MenuLink {
                url: endpoints::NEW_ACCOUNT_VIEW,
                title: "Add Account",
                is_current: active_endpoint == endpoints::NEW_ACCOUNT_VIEW,
            },
        ];

        let tags_submenu = vec![
            MenuLink {
                url: endpoints::TAGS_VIEW,
                title: "Tags",
                is_current: active_endpoint == endpoints::TAGS_VIEW,
            },
            MenuLink {
                url: endpoints::NEW_TAG_VIEW,
                title: "Create Tag",
                is_current: active_endpoint == endpoints::NEW_TAG_VIEW,
            },
        ];

        let rules_submenu = vec![
            MenuLink {
                url: endpoints::RULES_VIEW,
                title: "Rules",
                is_current: active_endpoint == endpoints::RULES_VIEW,
            },
            MenuLink {
                url: endpoints::NEW_RULE_VIEW,
                title: "Create Rule",
                is_current: active_endpoint == endpoints::NEW_RULE_VIEW,
            },
        ];

        let items = vec![
            MenuItem {
                key: "dashboard",
                url: endpoints::DASHBOARD_VIEW,
                title: "Dashboard",
                is_current: active_endpoint == endpoints::DASHBOARD_VIEW,
                submenu: Vec::new(),
            },
            MenuItem {
                key: "transactions",
                url: endpoints::TRANSACTIONS_VIEW,
                title: "Transactions",
                is_current: is_transactions,
                submenu: transactions_submenu,
            },
            MenuItem {
                key: "accounts",
                url: endpoints::ACCOUNTS,
                title: "Accounts",
                is_current: is_accounts,
                submenu: accounts_submenu,
            },
            MenuItem {
                key: "tags",
                url: endpoints::TAGS_VIEW,
                title: "Tags",
                is_current: is_tags,
                submenu: tags_submenu,
            },
            MenuItem {
                key: "rules",
                url: endpoints::RULES_VIEW,
                title: "Rules",
                is_current: is_rules,
                submenu: rules_submenu,
            },
            MenuItem {
                key: "logout",
                url: endpoints::LOG_OUT,
                title: "Log out",
                is_current: false,
                submenu: Vec::new(),
            },
        ];

        NavBar { items }
    }

    pub fn into_html(self) -> Markup {
        let items = self.items;
        let more_is_active = items.iter().any(|item| {
            (item.url == endpoints::TAGS_VIEW || item.url == endpoints::RULES_VIEW)
                && item.is_current
        });
        let desktop_item_class = |is_current: bool| -> &'static str {
            if is_current {
                "inline-flex items-center py-3 px-3 rounded-lg bg-blue-50 text-blue-700\
        lg:px-3 lg:py-2 lg:leading-none dark:bg-blue-900/30 dark:text-blue-200"
            } else {
                "inline-flex items-center py-2 px-3 text-gray-900 rounded-sm hover:bg-gray-100\
        lg:hover:bg-transparent lg:border-0 lg:hover:text-blue-700 lg:p-0 lg:leading-none\
        dark:text-gray-100 lg:dark:text-gray-100 lg:dark:hover:text-blue-300 dark:hover:bg-gray-700\
        dark:hover:text-white lg:dark:hover:bg-transparent"
            }
        };
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
        let submenu_item_class = |is_current: bool| -> &'static str {
            if is_current {
                "block rounded-lg bg-blue-50 px-3 py-2 text-blue-700 \
                dark:bg-blue-900/30 dark:text-blue-200"
            } else {
                "block rounded-lg px-3 py-2 text-gray-700 hover:bg-gray-100 \
                hover:text-blue-700 dark:text-gray-200 dark:hover:bg-gray-800/80 \
                dark:hover:text-blue-200"
            }
        };
        let bottom_toggle_class = |is_active: bool| -> &'static str {
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

        // Template adapted from https://flowbite.com/docs/components/navbar/#default-navbar
        html!(
            nav class="bg-white border-gray-200 dark:bg-gray-900" data-nav-scope="desktop"
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
                            @for item in items.clone().into_iter() {
                                li class="group relative flex items-center" data-nav-item=(item.key)
                                {
                                    @if item.has_submenu() {
                                        button
                                            type="button"
                                            class=(desktop_item_class(item.is_current))
                                            data-nav-toggle=(item.key)
                                            aria-expanded="false"
                                        {
                                            (item.title)
                                        }

                                        div
                                            class="absolute left-0 top-full hidden w-40 z-50
                                            rounded-xl border border-gray-200 bg-white/95
                                            p-2 shadow-xl backdrop-blur dark:border-gray-700
                                            dark:bg-gray-900/95"
                                            data-nav-menu=(item.key)
                                        {
                                            ul class="flex flex-col gap-1 text-sm font-medium"
                                            {
                                                @for sublink in item.submenu.iter() {
                                                    li {
                                                        a
                                                            href=(sublink.url)
                                                            class=(submenu_item_class(sublink.is_current))
                                                            aria-current=[sublink.is_current.then_some("page")]
                                                        {
                                                            (sublink.title)
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } @else {
                                        a
                                            href=(item.url)
                                            class=(desktop_item_class(item.is_current))
                                            aria-current=[item.is_current.then_some("page")]
                                        {
                                            (item.title)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            nav class="fixed inset-x-0 bottom-0 z-40 lg:hidden" data-nav-scope="mobile"
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
                            @for item in items.iter() {
                                @if item.url == endpoints::DASHBOARD_VIEW
                                    || item.url == endpoints::TRANSACTIONS_VIEW
                                    || item.url == endpoints::ACCOUNTS
                                {
                                    li class="relative min-w-0" data-nav-item=(item.key)
                                    {
                                        @if item.has_submenu() {
                                            button
                                                type="button"
                                                class=(bottom_toggle_class(item.is_current))
                                                data-nav-toggle=(item.key)
                                                aria-expanded="false"
                                            {
                                                span class="truncate" { (item.title) }
                                            }

                                            div
                                                class="absolute bottom-full right-0 mb-3 hidden w-40 z-50
                                                rounded-xl border border-gray-200 bg-white/95 p-2
                                                shadow-xl backdrop-blur dark:border-gray-700
                                                dark:bg-gray-900/95"
                                                data-nav-menu=(item.key)
                                            {
                                                ul class="flex flex-col gap-1 text-sm font-medium"
                                                {
                                                    @for sublink in item.submenu.iter() {
                                                        li {
                                                            a
                                                                href=(sublink.url)
                                                                class=(submenu_item_class(sublink.is_current))
                                                                aria-current=[sublink.is_current.then_some("page")]
                                                            {
                                                                (sublink.title)
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } @else {
                                            a
                                                href=(item.url)
                                                class=(bottom_link_class(item.is_current))
                                                aria-current=[item.is_current.then_some("page")]
                                            {
                                                span class="truncate" { (item.title) }
                                            }
                                        }
                                    }
                                }
                            }

                            li class="relative min-w-0" data-nav-item="more"
                            {
                                button
                                    type="button"
                                    class=(bottom_toggle_class(more_is_active))
                                    data-nav-toggle="more"
                                    aria-expanded="false"
                                {
                                    span class="truncate" { "More" }
                                }

                                div
                                    class="absolute bottom-full right-0 mb-3 hidden w-40 z-50 rounded-xl
                                    border border-gray-200 bg-white/95 p-2 shadow-xl
                                    backdrop-blur dark:border-gray-700 dark:bg-gray-900/95"
                                    data-nav-menu="more"
                                {
                                    ul class="flex flex-col gap-1 text-sm font-medium"
                                    {
                                        @for item in items.iter() {
                                            @if item.url == endpoints::TAGS_VIEW
                                                || item.url == endpoints::RULES_VIEW
                                            {
                                                @for sublink in item.submenu.iter() {
                                                    li {
                                                        a
                                                            href=(sublink.url)
                                                            class=(submenu_item_class(sublink.is_current))
                                                            aria-current=[sublink.is_current.then_some("page")]
                                                        {
                                                            (sublink.title)
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        li {
                                            a
                                                href=(endpoints::LOG_OUT)
                                                class=(submenu_item_class(false))
                                            {
                                                "Log out"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            script {
                (PreEscaped(include_str!("navigation.js")))
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
        cases.insert(endpoints::DASHBOARD_VIEW, "Dashboard");
        cases.insert(endpoints::TRANSACTIONS_VIEW, "Transactions");
        cases.insert(endpoints::NEW_TRANSACTION_VIEW, "Transactions");
        cases.insert(endpoints::IMPORT_VIEW, "Transactions");
        cases.insert(endpoints::QUICK_TAGGING_VIEW, "Transactions");
        cases.insert(endpoints::ACCOUNTS, "Accounts");
        cases.insert(endpoints::NEW_ACCOUNT_VIEW, "Accounts");
        cases.insert(endpoints::TAGS_VIEW, "Tags");
        cases.insert(endpoints::NEW_TAG_VIEW, "Tags");
        cases.insert(endpoints::RULES_VIEW, "Rules");
        cases.insert(endpoints::NEW_RULE_VIEW, "Rules");

        for (endpoint, active_title) in cases {
            let nav_bar = NavBar::new(endpoint);

            assert_active_item(nav_bar, active_title);
        }
    }

    #[test]
    fn ignores_non_page_endpoints() {
        let cases = [
            endpoints::ROOT,
            endpoints::COFFEE,
            endpoints::POST_TAG,
            endpoints::INTERNAL_ERROR_VIEW,
            endpoints::LOG_IN_API,
            endpoints::LOG_IN_VIEW,
            endpoints::LOG_OUT,
            endpoints::REGISTER_VIEW,
            endpoints::TRANSACTIONS_API,
            endpoints::USERS,
        ];

        for endpoint in cases {
            let nav_bar = NavBar::new(endpoint);
            assert_no_active_items(nav_bar);
        }
    }

    #[track_caller]
    fn assert_active_item(nav_bar: NavBar<'_>, active_title: &str) {
        let mut got_active = false;

        for item in nav_bar.items {
            if item.title == active_title {
                assert!(item.is_current, "Expected {active_title} to be active.");
                got_active = true;
            } else {
                assert!(!item.is_current, "Expected {} to be inactive.", item.title);
            }
        }

        assert!(
            got_active,
            "Expected to find active item for {active_title}."
        );
    }

    #[track_caller]
    fn assert_no_active_items(nav_bar: NavBar<'_>) {
        for item in nav_bar.items {
            assert!(!item.is_current, "Expected {} to be inactive.", item.title);
        }
    }
}
