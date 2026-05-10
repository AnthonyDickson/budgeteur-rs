//! Alert system for displaying success and error messages to users.
//!
//! This module provides a unified way to display alert messages across the application
//! with proper styling and dismissal functionality.
//!
//! ## Displaying Alerts
//!
//! To display an alert, just add the attribute `hx-swap="none"` and respond with
//! an [AlertTemplate]. The base template already has a `div` with the ID
//! `alert-container` to swap with HTMX.
//!
//! If you want to display an error alert for a 4XX-5XX status code, add the
//! attribute `hx-target-error="#alert-container"` to the element that is making
//! the HTMX request.
//! **Note**: This assumes that *all* errors are handled by responding with an
//! alert, otherwise the alert container element will be simply replaced with
//! the response body.

use axum::response::{Html, IntoResponse, Response};
use maud::{Markup, html};

/// Alert types
#[derive(Debug, Clone)]
pub enum Alert {
    Success { message: String, details: String },
    SuccessSimple { message: String },
    Error { message: String, details: String },
    ErrorSimple { message: String },
}

impl Alert {
    pub fn into_html(self) -> Html<String> {
        Html(alert_template(self).into_string())
    }
}

impl IntoResponse for Alert {
    fn into_response(self) -> Response {
        alert_template(self).into_response()
    }
}

fn alert_template(alert: Alert) -> Markup {
    let (is_success, message, details) = match alert {
        Alert::Success { message, details } => (true, message, Some(details)),
        Alert::SuccessSimple { message } => (true, message, None),
        Alert::Error { message, details } => (false, message, Some(details)),
        Alert::ErrorSimple { message } => (false, message, None),
    };

    let container_style = if is_success {
        "bg-green-50 border border-green-200 text-green-800 dark:bg-gray-800 dark:border-green-700 dark:text-green-200"
    } else {
        "bg-red-50 border border-red-200 text-red-800 dark:bg-gray-800 dark:border-red-700 dark:text-red-300"
    };

    let button_style = if is_success {
        "focus:ring-green-600"
    } else {
        "focus:ring-red-600"
    };

    html! {
        div
            id="alert-container"
            hx-swap-oob="true"
            class="w-full max-w-md px-4" style="position: fixed; bottom: 1rem; left: 50%; transform: translateX(-50%); z-index: 9999;"
        {
            div class={"p-4 rounded shadow-lg w-full" (container_style)}
            {
                div class="flex items-start"
                {
                    div class="shrink-0"
                    {
                        @if is_success {
                            svg
                                viewBox="0 0 20 20"
                                fill="currentColor"
                                class="w-5 h-5 text-green-400"
                            {
                                path
                                    fill-rule="evenodd"
                                    clip-rule="evenodd"
                                    d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.707-9.293a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z"
                                {}
                            }
                        } @else {
                            svg
                                viewBox="0 0 20 20"
                                fill="currentColor"
                                class="w-5 h-5 text-red-400"
                            {
                                path
                                    fill-rule="evenodd"
                                    clip-rule="evenodd"
                                    d="M10 18a8 8 0 100-16 8 8 0 000 16zM8.707 7.293a1 1 0 00-1.414 1.414L8.586 10l-1.293 1.293a1 1 0 101.414 1.414L10 11.414l1.293 1.293a1 1 0 001.414-1.414L11.414 10l1.293-1.293a1 1 0 00-1.414-1.414L10 8.586 8.707 7.293z"
                                {}
                            }
                        }
                    }
                    div class="ml-3 flex-1"
                    {
                        p class="text-sm font-medium"
                        {
                            (message)
                        }

                        @if let Some(details) = details {
                            p class="mt-1 text-sm opacity-80"
                            {
                                (details)
                            }
                        }
                    }
                    div class="ml-auto pl-3"
                    {
                        div class="-mx-1.5 -my-1.5"
                        {
                            button
                                type="button"
                                class={"inline-flex rounded-md p-1.5
                                    hover:bg-black/5 active:bg-black/10 focus:outline-hidden
                                    focus:ring-2 focus:ring-offset-2"
                                    (button_style)}
                                onclick="this.parentElement.parentElement.parentElement.parentElement.remove()"
                            {
                                span class="sr-only"
                                {
                                    "Dismiss"
                                }
                                svg
                                    viewBox="0 0 20 20"
                                    fill="currentColor"
                                    class="w-5 h-5"
                                {
                                    path
                                        fill-rule="evenodd"
                                        clip-rule="evenodd"
                                        d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
                                    {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
