//! The registration page for creating a new user account.

use askama::Template;
use axum::response::{IntoResponse, Response};

use crate::routes::templates::{PasswordInputTemplate, RegisterFormTemplate};

/// The minimum number of characters the password should have to be considered valid on the client side (server-side validation is done on top of this validation).
const PASSWORD_INPUT_MIN_LENGTH: usize = 8;

#[derive(Template)]
#[template(path = "views/register.html")]
struct RegisterPageTemplate<'a> {
    register_form: RegisterFormTemplate<'a>,
}

/// Display the registration page.
pub async fn get_register_page() -> Response {
    RegisterPageTemplate {
        register_form: RegisterFormTemplate {
            password_input: PasswordInputTemplate {
                min_length: PASSWORD_INPUT_MIN_LENGTH,
                ..Default::default()
            },
            ..Default::default()
        },
    }
    .into_response()
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Response, StatusCode, header::CONTENT_TYPE},
    };
    use serde::{Deserialize, Serialize};

    use crate::routes::{endpoints, views::register::get_register_page};

    #[derive(Serialize, Deserialize)]
    struct Foo {
        bar: String,
    }

    #[tokio::test]
    async fn render_register_page() {
        let response = get_register_page().await;
        assert_eq!(response.status(), StatusCode::OK);

        assert!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );

        let document = parse_html(response).await;

        let h1_selector = scraper::Selector::parse("h1").unwrap();
        let titles = document.select(&h1_selector).collect::<Vec<_>>();
        assert_eq!(titles.len(), 1, "want 1 h1, got {}", titles.len());
        let title = titles.first().unwrap();
        let title_text = title.text().collect::<String>().to_lowercase();
        let title_text = title_text.trim();
        let want_title = "create account";
        assert_eq!(
            title_text, want_title,
            "want {}, got {:?}",
            want_title, title_text
        );

        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());
        let form = forms.first().unwrap();
        let hx_post = form.value().attr("hx-post");
        assert_eq!(
            hx_post,
            Some(endpoints::USERS),
            "want form with attribute hx-post=\"{}\", got {:?}",
            endpoints::USERS,
            hx_post
        );

        struct FormInput {
            tag: &'static str,
            type_: &'static str,
            id: &'static str,
        }

        let want_form_inputs: Vec<FormInput> = vec![
            FormInput {
                tag: "input",
                type_: "email",
                id: "email",
            },
            FormInput {
                tag: "input",
                type_: "password",
                id: "password",
            },
            FormInput {
                tag: "input",
                type_: "password",
                id: "confirm-password",
            },
        ];

        for FormInput { tag, type_, id } in want_form_inputs {
            let selector_string = format!("{tag}[type={type_}]#{id}");
            let input_selector = scraper::Selector::parse(&selector_string).unwrap();
            let inputs = form.select(&input_selector).collect::<Vec<_>>();
            assert_eq!(
                inputs.len(),
                1,
                "want 1 {type_} {tag}, got {}",
                inputs.len()
            );
        }

        let log_in_link_selector = scraper::Selector::parse("a[href]").unwrap();
        let links = form.select(&log_in_link_selector).collect::<Vec<_>>();
        assert_eq!(links.len(), 1, "want 1 link, got {}", links.len());
        let link = links.first().unwrap();
        assert_eq!(
            link.value().attr("href"),
            Some(endpoints::LOG_IN_VIEW),
            "want link to {}, got {:?}",
            endpoints::LOG_IN_VIEW,
            link.value().attr("href")
        );
    }

    async fn parse_html(response: Response<Body>) -> scraper::Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        scraper::Html::parse_document(&text)
    }
}
