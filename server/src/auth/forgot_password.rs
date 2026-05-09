use axum::{response::IntoResponse, response::Response};
use maud::{Markup, html};

use crate::html::base;

fn forgot_password_template() -> Markup {
    let content = html! {
        // Template adapted from https://flowbite.com/blocks/marketing/register/
        div
            class="flex flex-col items-center justify-center px-6 py-8 mx-auto text-gray-900 dark:text-white"
        {
            a
                href="#"
                class="flex items-center mb-6 text-2xl font-semibold"
            {
                img
                    src="/static/favicon-128x128.png"
                    alt="logo"
                    class="w-8 h-8 mr-2";
                "Budgeteur"
            }
            div
                class="w-full bg-white rounded shadow dark:border md:mt-0 sm:max-w-md xl:p-0 dark:bg-gray-800 dark:border-gray-700"
            {
                div class="p-6 space-y-4 md:space-y-6 sm:p-8"
                {
                    h1
                        class="text-xl font-bold md:text-2xl"
                    {
                        "Forgot your password?"
                    }
                    p class="text-justify"
                    {
                        "To reset your password, go to directory where this server is
                    running from and run the program 'reset_password' and point it to
                    your database file. For more details, see "
                        a
                            href="https://github.com/AnthonyDickson/budgeteur-rs?tab=readme-ov-file#resetting-your-password"
                            class="py-2 px-3 text-white bg-blue-700 rounded-sm md:bg-transparent md:text-blue-700 md:p-0 dark:text-white md:dark:text-blue-500"
                        {
                            span class="flex items-center"
                            {
                                "README.md"
                                svg
                                    width="1em"
                                    height="1em"
                                    viewBox="0 0 24 24"
                                    stroke ="#3671D3"
                                    fill="none" xmlns="http://www.w3.org/2000/svg"
                                {
                                    g id="Interface / External_Link"
                                    {
                                        path
                                            id="Vector"
                                            d="M10.0002 5H8.2002C7.08009 5 6.51962 5 6.0918 5.21799C5.71547 5.40973 5.40973 5.71547 5.21799 6.0918C5 6.51962 5 7.08009 5 8.2002V15.8002C5 16.9203 5 17.4801 5.21799 17.9079C5.40973 18.2842 5.71547 18.5905 6.0918 18.7822C6.5192 19 7.07899 19 8.19691 19H15.8031C16.921 19 17.48 19 17.9074 18.7822C18.2837 18.5905 18.5905 18.2839 18.7822 17.9076C19 17.4802 19 16.921 19 15.8031V14M20 9V4M20 4H15M20 4L13 11" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
                                        {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    base("Forgot Password", &[], &content)
}

/// Renders a page describing how the user's password can be reset.
pub async fn get_forgot_password_page() -> Response {
    forgot_password_template().into_response()
}
