use axum::response::{IntoResponse, Response};
use maud::{Markup, html};

use crate::{
    endpoints,
    navigation::NavBar,
    view_templates::{BUTTON_PRIMARY_STYLE, FORM_TEXT_INPUT_STYLE, base, loading_spinner},
};

fn import_form_view() -> Markup {
    let import_route = endpoints::IMPORT;
    let spinner = loading_spinner();

    html! {
        form
            hx-post=(import_route)
            enctype="multipart/form-data"
            hx-disabled-elt="#files, #submit-button"
            hx-indicator="#indicator"
            hx-swap="none"
            hx-target-error="#alert-container"
            class="space-y-4 md:space-y-6"
        {
            div
            {
                label
                    for="files"
                    class="block mb-2 text-sm font-medium text-gray-900 dark:text-white"
                {
                    "Choose file(s) to upload"
                }

                input
                    id="files"
                    type="file"
                    name="files"
                    accept="text/csv"
                    placeholder="files"
                    multiple
                    required
                    class=(FORM_TEXT_INPUT_STYLE);

                p
                {
                    "Export and upload your bank statements in CSV format to automatically import your transactions."
                }
            }

             button
                type="submit"
                id="submit-button"
                class=(BUTTON_PRIMARY_STYLE)
            {
                span class="inline htmx-indicator" id="indicator" { (spinner) }
                " Upload Files"
            }
        }
    }
}

fn import_view() -> Markup {
    let nav_bar = NavBar::new(endpoints::IMPORT_VIEW).into_html();
    let form = import_form_view();

    let content = html! {
        (nav_bar)

        div
            class="flex flex-col items-center px-6 py-8 mx-auto lg:py-0
            text-gray-900 dark:text-white"
        {
            div class="relative"
            {
                (form)
            }
        }
    };

    base("Import Transactions", &[], &content)
}

/// Route handler for the import CSV page.
pub async fn get_import_page() -> Response {
    import_view().into_response()
}

#[cfg(test)]
mod import_transactions_tests {
    use axum::{http::StatusCode, response::Response};
    use scraper::{ElementRef, Html};

    use crate::{csv_import::import_page::get_import_page, endpoints};

    #[tokio::test]
    async fn render_page() {
        let response = get_import_page().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .expect("content-type header missing"),
            "text/html; charset=utf-8"
        );

        let html = parse_html(response).await;
        assert_valid_html(&html);

        let form = must_get_form(&html);
        assert_hx_endpoint(&form, endpoints::IMPORT);
        assert_form_enctype(&form, "multipart/form-data");
        assert_form_input(&form, "files", "file");
        assert_form_submit_button(&form);
    }

    async fn parse_html(response: Response) -> Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        Html::parse_document(&text)
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }

    #[track_caller]
    fn must_get_form(html: &Html) -> ElementRef<'_> {
        html.select(&scraper::Selector::parse("form").unwrap())
            .next()
            .expect("No form found")
    }

    #[track_caller]
    fn assert_hx_endpoint(form: &ElementRef, endpoint: &str) {
        let hx_post = form
            .value()
            .attr("hx-post")
            .expect("hx-post attribute missing");

        assert_eq!(
            hx_post, endpoint,
            "want form with attribute hx-post=\"{endpoint}\", got {hx_post:?}"
        );
        assert_eq!(hx_post, endpoint);
    }

    #[track_caller]
    fn assert_form_enctype(form: &ElementRef, enctype: &str) {
        let form_enctype = form
            .value()
            .attr("enctype")
            .expect("enctype attribute missing");

        assert_eq!(
            form_enctype, enctype,
            "want form with attribute enctype=\"{enctype}\", got {form_enctype:?}"
        );
    }

    #[track_caller]
    fn assert_form_input(form: &ElementRef, name: &str, type_: &str) {
        for input in form.select(&scraper::Selector::parse("input").unwrap()) {
            let input_name = input.value().attr("name").unwrap_or_default();

            if input_name == name {
                let input_type = input.value().attr("type").unwrap_or_default();
                let input_required = input.value().attr("required");
                let input_multiple = input.value().attr("multiple");
                let input_accept = input.value().attr("accept").unwrap_or_default();

                assert_eq!(
                    input_type, type_,
                    "want input with type \"{type_}\", got {input_type:?}"
                );

                assert!(
                    input_required.is_some(),
                    "want input with name {name} to have the required attribute but got none"
                );

                assert!(
                    input_multiple.is_some(),
                    "want input with name {name} to have the multiple attribute but got none"
                );

                assert_eq!(
                    input_accept, "text/csv",
                    "want input with name {name} to have the accept attribute \"text/csv\" but got {input_accept:?}"
                );

                return;
            }
        }

        panic!("No input found with name \"{name}\" and type \"{type_}\"");
    }

    #[track_caller]
    fn assert_form_submit_button(form: &ElementRef) {
        let submit_button = form
            .select(&scraper::Selector::parse("button").unwrap())
            .next()
            .expect("No button found");

        assert_eq!(
            submit_button.value().attr("type").unwrap_or_default(),
            "submit",
            "want submit button with type=\"submit\""
        );
    }
}
