use maud::{DOCTYPE, Markup, PreEscaped, html};

use std::sync::OnceLock;

use numfmt::{Formatter, Precision};

// In view_templates.rs

// Link styles
pub const LINK_STYLE: &str = "text-blue-600 hover:text-blue-500 \
    dark:text-blue-500 dark:hover:text-blue-400 underline";

// Button styles
pub const BUTTON_PRIMARY_STYLE: &str = "w-full px-4 py-2 bg-blue-500
    dark:bg-blue-600 disabled:bg-blue-700 hover:enabled:bg-blue-600 \
    hover:enabled:dark:bg-blue-700 text-white rounded";

pub const BUTTON_SECONDARY_STYLE: &str = "w-full py-2.5 px-5 mb-2 \
    text-sm font-medium text-gray-900 bg-white rounded border border-gray-200 \
    hover:bg-gray-100 hover:text-blue-700 focus:z-10 dark:bg-gray-800 \
    dark:text-gray-400 dark:border-gray-600 dark:hover:text-white \
    dark:hover:bg-gray-700";

pub const BUTTON_DELETE_STYLE: &str = "text-red-600 hover:text-red-500 \
    dark:text-red-500 dark:hover:text-red-400 underline bg-transparent \
    border-none cursor-pointer";

// Form styles
pub const FORM_CONTAINER_STYLE: &str = "flex flex-col items-center px-6 py-8 \
    mx-auto lg:py-0 max-w-md text-gray-900 dark:text-white";
pub const FORM_LABEL_STYLE: &str = "block mb-2 text-sm font-medium text-gray-900 dark:text-white";
pub const FORM_TEXT_INPUT_STYLE: &str = "block w-full p-2.5 rounded text-sm \
    text-gray-900 dark:text-white disabled:text-gray-500 bg-gray-50 \
    dark:bg-gray-700 border border-gray-300 dark:border-gray-600 \
    dark:placeholder-gray-400 focus:ring-blue-600 focus:border-blue-600 \
    focus:dark:border-blue-500 focus:dark:ring-blue-500";
pub const FORM_RADIO_GROUP_STYLE: &str = "flex flex-col gap-2";
pub const FORM_RADIO_INPUT_STYLE: &str = "peer h-4 w-4 shrink-0 cursor-pointer \
    text-blue-600 border-gray-300 dark:border-gray-600 focus-visible:ring-2 \
    focus-visible:ring-blue-500 focus-visible:ring-offset-2 \
    focus-visible:ring-offset-white focus-visible:dark:ring-offset-gray-900";
pub const FORM_RADIO_LABEL_STYLE: &str = "flex-1 rounded border border-gray-300 \
    dark:border-gray-600 bg-white dark:bg-gray-700 px-3 py-2 text-sm font-medium \
    text-gray-700 dark:text-white cursor-pointer transition \
    hover:border-gray-400 hover:bg-gray-50 hover:text-gray-900 \
    hover:dark:border-gray-500 hover:dark:bg-gray-600 active:scale-[0.99] \
    peer-checked:border-blue-600 peer-checked:bg-blue-50 peer-checked:text-blue-700 \
    peer-checked:shadow-sm peer-checked:dark:border-blue-500 \
    peer-checked:dark:bg-blue-600/20 peer-checked:dark:text-blue-200";

// Table styles
pub const TABLE_HEADER_STYLE: &str = "text-xs text-gray-700 uppercase \
    bg-gray-50 dark:bg-gray-700 dark:text-gray-400";

pub const TABLE_ROW_STYLE: &str = "bg-white border-b dark:bg-gray-800 dark:border-gray-700";

pub const TABLE_CELL_STYLE: &str = "px-6 py-4";

// Tag badge style
pub const TAG_BADGE_STYLE: &str = "inline-flex items-center px-2.5 py-0.5 \
    text-xs font-semibold text-blue-800 bg-blue-100 rounded-full \
    dark:bg-blue-900 dark:text-blue-300";

// Page container
pub const PAGE_CONTAINER_STYLE: &str =
    "flex flex-col items-center px-6 py-8 mx-auto lg:py-5 text-gray-900 dark:text-white";

pub enum HeadElement {
    /// The file path or URL to a JavaScript script.
    ScriptLink(String),
    #[allow(dead_code)]
    /// JavaScript source code.
    ScriptSource(PreEscaped<String>),
    Style(PreEscaped<String>),
}

pub fn base(title: &str, head_elements: &[HeadElement], content: &Markup) -> Markup {
    html! {
        (DOCTYPE)
        html lang="en"
        {
            head
            {
                meta charset="UTF-8";
                meta name="viewport" content="width=device-width, initial-scale=1.0";
                title { (title) " - Budgeteur" }
                link rel="icon" type="image/png" href="/static/favicon-32x32.png" sizes="32x32";
                link rel="icon" type="image/png" href="/static/favicon-128x128.png" sizes="128x128";
                link href="/static/main.css" rel="stylesheet";

                script src="/static/htmx-2.0.8-min.js" integrity="sha384-/TgkGk7p307TH7EXJDuUlgG3Ce1UVolAOFopFekQkkXihi5u/6OCvVKyz1W+idaz" {}
                script src="/static/htmx-ext-response-targets-2.0.4.js" integrity="sha384-T41oglUPvXLGBVyRdZsVRxNWnOOqCynaPubjUVjxhsjFTKrFJGEMm3/0KGmNQ+Pg" {}

                style
                {
                    r#"
                    #indicator.htmx-indicator {
                        display: none;
                    }

                    #indicator.htmx-request .htmx-indicator {
                        display: inline;
                    }

                    #indicator.htmx-request.htmx-indicator {
                        display: inline;
                    }

                    /* Keep chart tooltips below the fixed bottom nav, but above page content. */
                    .echarts-tooltip {
                        z-index: 30 !important;
                    }
                    "#
                }

                @for element in head_elements
                {
                    @match element
                    {
                        HeadElement::ScriptSource(text) => script { (text) }
                        HeadElement::ScriptLink(path) => script src=(path) {}
                        HeadElement::Style(text) => style { (text) }
                    }
                }

                script src="/static/app.js" defer {}
            }

            body
                hx-ext="response-targets"
                class="container max-w-full min-h-screen bg-gray-50 dark:bg-gray-900 pb-[calc(5rem+env(safe-area-inset-bottom))] lg:pb-0"
            {
                (content)

                // Alert container for out-of-band swaps
                div
                    id="alert-container"
                    class="hidden w-full max-w-md px-4"
                    style="position: fixed; bottom: 1rem; left: 50%; transform: translateX(-50%); z-index: 9999;"
                {}
            }
        }
    }
}

pub fn error_view(title: &str, header: &str, description: &str, fix: &str) -> Markup {
    // Template adapted from https://flowbite.com/blocks/marketing/404/
    let content = html!(
        section class="bg-white dark:bg-gray-900"
        {
            div class="py-8 px-4 mx-auto max-w-screen-xl lg:py-16 lg:px-6"
            {
                div class="mx-auto max-w-screen-sm text-center"
                {
                    h1
                        class="mb-4 text-7xl tracking-tight font-extrabold
                            lg:text-9xl text-blue-600 dark:text-blue-500"
                    {
                        (header)
                    }

                    p
                        class="mb-4 text-3xl md:text-4xl tracking-tight
                            font-bold text-gray-900 dark:text-white"
                    {
                        (description)
                    }

                    p
                        class="mb-4 text-1xl md:text-2xl tracking-tight
                            text-gray-900 dark:text-white"
                    {
                        (fix)
                    }

                    a
                        href="/"
                        class="inline-flex text-white bg-blue-600
                            hover:bg-blue-800 focus:ring-4 focus:outline-hidden
                            focus:ring-blue-300 font-medium rounded text-sm px-5
                            py-2.5 text-center dark:focus:ring-blue-900 my-4"
                    {
                        "Back to Homepage"
                    }
                }
            }
        }
    );

    base(title, &[], &content)
}

pub fn log_in_register(form_title: &str, form: &Markup) -> Markup {
    html! {
        div class="flex flex-col items-center justify-center px-6 py-8 mx-auto"
        {
            a href="#" class="flex items-center mb-6 text-2xl font-semibold text-gray-900 dark:text-white"
            {
                img class="w-8 h-8 mr-2" src="/static/favicon-128x128.png" alt="logo";
                "Budgeteur"
            }

            div class="w-full bg-white rounded-lg shadow dark:border md:mt-0 sm:max-w-md xl:p-0 dark:bg-gray-800 dark:border-gray-700"
            {
                div class="p-6 space-y-4 md:space-y-6 sm:p-8"
                {
                    h1 class="text-xl font-bold leading-tight tracking-tight text-gray-900 md:text-2xl dark:text-white"
                    {
                        (form_title)
                    }

                    (form)
                }
            }
        }
    }
}

pub fn password_input(password: &str, min_length: u8, error_message: Option<&str>) -> Markup {
    html! {
        div
        {
            label
                for="password"
                class=(FORM_LABEL_STYLE)
            {
                "Password"
            }

            input
                type="password"
                name="password"
                id="password"
                placeholder="••••••••"
                class=(FORM_TEXT_INPUT_STYLE)
                required
                autofocus
                value=(password)
                minlength=(min_length);

            @if let Some(error_message) = error_message
            {
                p class="text-red-500 text-base" { (error_message) }
            }
        }

    }
}

pub fn loading_spinner() -> Markup {
    // Spinner SVG adapted from https://flowbite.com/docs/components/spinner/
    html! {
        svg
            aria-hidden="true"
            role="status"
            class="inline text-white w-4 h-4 me-2 mb-1 animate-spin"
            viewBox="0 0 100 101"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
        {
            path
                d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z"
                fill="#E5E7EB" {}
            path
                d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z"
                fill="currentColor" {}
        }
    }
}

/// Returns the CSS styles for adding a dollar sign prefix to number inputs.
/// Used for currency input fields across multiple forms.
pub fn dollar_input_styles() -> HeadElement {
    HeadElement::Style(PreEscaped(
        r#"
        .input-wrapper {
            position: relative;
            display: inline-block;
        }
        .input-wrapper input[type="number"] {
            padding-left: 1.4rem;
        }
        .input-wrapper::before {
            content: '$';
            position: absolute;
            left: 0.6rem;
            top: 50%;
            transform: translateY(-50%);
            pointer-events: none;
        }
        "#
        .to_owned(),
    ))
}

pub fn format_currency(number: f64) -> String {
    static POSITIVE_FMT: OnceLock<Formatter> = OnceLock::new();

    let positive_fmt = POSITIVE_FMT.get_or_init(|| {
        Formatter::currency("$")
            .unwrap()
            .precision(Precision::Decimals(2))
    });

    static NEGATIVE_FMT: OnceLock<Formatter> = OnceLock::new();

    let negative_fmt = NEGATIVE_FMT.get_or_init(|| {
        Formatter::currency("-$")
            .unwrap()
            .precision(Precision::Decimals(2))
    });

    let mut formatted_string = if number < 0.0 {
        negative_fmt.fmt_string(number.abs())
    } else if number > 0.0 {
        positive_fmt.fmt_string(number)
    } else {
        // Zero is hardcoded as "0", so we must specify the formatted string for zero
        "$0.00".to_owned()
    };

    // numfmt omits the last trailing zero, so we must add it ourselves
    // For example, "12.30" is rendered as "12.3" so we append "0".
    if formatted_string.as_bytes()[formatted_string.len() - 3] != b'.' {
        formatted_string = format!("{formatted_string}0");
    }

    formatted_string
}

pub fn format_currency_rounded(number: f64) -> String {
    static POSITIVE_FMT: OnceLock<Formatter> = OnceLock::new();

    let positive_fmt = POSITIVE_FMT.get_or_init(|| {
        Formatter::currency("$")
            .unwrap()
            .precision(Precision::Decimals(0))
    });

    static NEGATIVE_FMT: OnceLock<Formatter> = OnceLock::new();

    let negative_fmt = NEGATIVE_FMT.get_or_init(|| {
        Formatter::currency("-$")
            .unwrap()
            .precision(Precision::Decimals(0))
    });

    let number = number.round();

    if number < 0.0 {
        negative_fmt.fmt_string(number.abs())
    } else if number > 0.0 {
        positive_fmt.fmt_string(number)
    } else {
        // Zero is hardcoded as "0", so we must specify the formatted string for zero
        "$0".to_owned()
    }
}

/// Creates a span with `amount` rounded to the nearest whole number and a
/// tooltip (title) that shows `amount` rounded to two decimal places.
pub fn currency_rounded_with_tooltip(amount: f64) -> Markup {
    html!(
        span title=(format_currency(amount)) { (format_currency_rounded(amount)) }
    )
}

/// A link with blue text for use in a <p> tag.
pub fn link(url: &str, text: &str) -> Markup {
    html! (
        a
            href=(url)
            class="text-blue-600 hover:text-blue-500 dark:text-blue-500 dark:hover:text-blue-400 underline"
        {
          (text)
        }

    )
}
