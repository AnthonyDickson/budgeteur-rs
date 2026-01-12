use maud::{DOCTYPE, Markup, PreEscaped, html};

// TODO: Remove allow dead code once migration to maud is complete
#[allow(dead_code)]
pub enum HeadElement {
    /// The file path or URL to a JavaScript script.
    ScriptLink(String),
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
                class="container max-w-full min-h-screen bg-gray-50 dark:bg-gray-900"
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

pub const FORM_LABEL_STYLE: &str = "block mb-2 text-sm font-medium text-gray-900 dark:text-white";
pub const FORM_TEXT_INPUT_STYLE: &str =
    "block w-full p-2.5 rounded text-sm text-gray-900 dark:text-white
disabled:text-gray-500 bg-gray-50 dark:bg-gray-700 border border-gray-300
dark:border-gray-600 dark:placeholder-gray-400 focus:ring-blue-600
focus:border-blue-600 focus:dark:border-blue-500 focus:dark:ring-blue-500";

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
