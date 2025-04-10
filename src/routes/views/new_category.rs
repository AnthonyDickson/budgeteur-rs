use askama_axum::Template;
use axum::response::{IntoResponse, Response};

use crate::routes::{
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    templates::NewCategoryFormTemplate,
};

/// Renders the new Category page.
#[derive(Template)]
#[template(path = "views/new_category.html")]
struct NewCategoryTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    form: NewCategoryFormTemplate<'a>,
}

pub async fn get_new_category_page() -> Response {
    NewCategoryTemplate {
        nav_bar: get_nav_bar(endpoints::NEW_CATEGORY_VIEW),
        form: NewCategoryFormTemplate {
            category_route: endpoints::CATEGORIES,
            error_message: "",
        },
    }
    .into_response()
}

#[cfg(test)]
mod new_category_tests {
    use axum::{http::StatusCode, response::Response};
    use scraper::{ElementRef, Html};

    use crate::routes::{endpoints, views::new_category::get_new_category_page};

    #[tokio::test]
    async fn render_page() {
        let response = get_new_category_page().await;

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
        assert_hx_endpoint(&form, endpoints::CATEGORIES);
        assert_form_input(&form, "name", "text");
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
    fn must_get_form(html: &Html) -> ElementRef {
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
    fn assert_form_input(form: &ElementRef, name: &str, type_: &str) {
        for input in form.select(&scraper::Selector::parse("input").unwrap()) {
            let input_name = input.value().attr("name").unwrap_or_default();

            if input_name == name {
                let input_type = input.value().attr("type").unwrap_or_default();
                let input_required = input.value().attr("required");

                assert_eq!(
                    input_type, type_,
                    "want input with type \"{type_}\", got {input_type:?}"
                );

                assert!(
                    input_required.is_some(),
                    "want input with name {name} to have the required attribute but got none"
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
