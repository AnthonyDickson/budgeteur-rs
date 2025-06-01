//! This file defines the high-level log-in route logic.
//! The auth module handles the lower level authentication and cookie auth logic.

use askama_axum::Template;
use axum::response::{IntoResponse, Response};

use crate::routes::templates::LogInFormTemplate;

///  Renders the full log-in page.
#[derive(Template, Default)]
#[template(path = "views/log_in.html")]
struct LogInTemplate<'a> {
    log_in_form: LogInFormTemplate<'a>,
}

/// Display the log-in page.
pub async fn get_log_in_page() -> Response {
    LogInTemplate::default().into_response()
}

#[cfg(test)]
mod log_in_tests {
    use std::{collections::HashMap, iter::zip};

    use axum::{
        Form,
        extract::State,
        http::{StatusCode, header::CONTENT_TYPE},
    };
    use axum_extra::extract::PrivateCookieJar;
    use email_address::EmailAddress;
    use scraper::Html;

    use crate::{
        Error,
        auth::log_in::LogInData,
        models::{PasswordHash, User, UserID, ValidatedPassword},
        routes::{
            endpoints,
            log_in::{INVALID_CREDENTIALS_ERROR_MSG, post_log_in},
        },
        state::LoginState,
        stores::UserStore,
    };

    use super::get_log_in_page;

    #[derive(Clone)]
    struct StubUserStore {
        users: Vec<User>,
    }

    impl UserStore for StubUserStore {
        fn create(
            &mut self,
            email: email_address::EmailAddress,
            password_hash: PasswordHash,
        ) -> Result<User, Error> {
            let next_id = match self.users.last() {
                Some(user) => UserID::new(user.id().as_i64() + 1),
                _ => UserID::new(0),
            };

            let user = User::new(next_id, email, password_hash);
            self.users.push(user.clone());

            Ok(user)
        }

        fn get(&self, id: UserID) -> Result<User, Error> {
            self.users
                .iter()
                .find(|user| user.id() == id)
                .ok_or(Error::NotFound)
                .map(|user| user.to_owned())
        }

        fn get_by_email(&self, email: &email_address::EmailAddress) -> Result<User, Error> {
            self.users
                .iter()
                .find(|user| user.email() == email)
                .ok_or(Error::NotFound)
                .map(|user| user.to_owned())
        }

        fn count(&self) -> Result<usize, Error> {
            todo!()
        }
    }

    #[tokio::test]
    async fn log_in_page_displays_form() {
        let response = get_log_in_page().await;

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

        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();
        let document = scraper::Html::parse_document(&text);
        assert_valid_html(&document);

        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());
        let form = forms.first().unwrap();
        let hx_post = form.value().attr("hx-post");
        assert_eq!(
            hx_post,
            Some(endpoints::LOG_IN_API),
            "want form with attribute hx-post=\"{}\", got {:?}",
            endpoints::LOG_IN_API,
            hx_post
        );

        let mut expected_form_elements: HashMap<&str, Vec<&str>> = HashMap::new();
        expected_form_elements.insert("input", vec!["email", "password"]);
        expected_form_elements.insert("button", vec!["submit"]);

        for (tag, element_types) in expected_form_elements {
            for element_type in element_types {
                let selector_string = format!("{tag}[type={element_type}]");
                let input_selector = scraper::Selector::parse(&selector_string).unwrap();
                let inputs = form.select(&input_selector).collect::<Vec<_>>();
                assert_eq!(
                    inputs.len(),
                    1,
                    "want 1 {element_type} {tag}, got {}",
                    inputs.len()
                );
            }
        }

        let register_link_selector = scraper::Selector::parse("a[href]").unwrap();
        let links = form.select(&register_link_selector).collect::<Vec<_>>();
        assert_eq!(links.len(), 2, "want 2 link, got {}", links.len());
        let want_endpoints = [endpoints::FORGOT_PASSWORD_VIEW, endpoints::REGISTER_VIEW];

        for (link, endpoint) in zip(links, want_endpoints) {
            assert_eq!(
                link.value().attr("href"),
                Some(endpoint),
                "want link to {}, got {:?}",
                endpoint,
                link.value().attr("href")
            );
        }
    }

    #[tokio::test]
    async fn log_in_page_displays_error_message() {
        let state = get_test_app_config();
        let jar = PrivateCookieJar::new(state.cookie_key.clone());
        let form = LogInData {
            email: "foo@bar.baz".to_string(),
            password: "wrongpassword".to_string(),
            remember_me: None,
        };
        let response = post_log_in(State(state), jar, Form(form)).await;

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

        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();
        let document = scraper::Html::parse_fragment(&text);
        assert_valid_html(&document);

        let form_selector = scraper::Selector::parse("form").unwrap();
        let forms = document.select(&form_selector).collect::<Vec<_>>();
        assert_eq!(forms.len(), 1, "want 1 form, got {}", forms.len());
        let form = forms.first().unwrap();

        let p_selector = scraper::Selector::parse("p").unwrap();
        let p = form.select(&p_selector).collect::<Vec<_>>();
        let p = p.first();

        assert!(
            p.is_some(),
            "could not find p tag for error messsage in form"
        );

        let p = p.unwrap();

        let p_text = p.text().collect::<String>();
        assert!(
            p_text.contains(INVALID_CREDENTIALS_ERROR_MSG),
            "error message should contain string \"{INVALID_CREDENTIALS_ERROR_MSG}\" but got {p_text}"
        );
    }

    fn get_test_app_config() -> LoginState<StubUserStore> {
        let mut state = LoginState::new("42", StubUserStore { users: vec![] });

        state
            .user_store
            .create(
                EmailAddress::new_unchecked("test@test.com"),
                PasswordHash::new(ValidatedPassword::new_unchecked("test"), 4).unwrap(),
            )
            .unwrap();

        state
    }

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}\n{}",
            html.errors,
            html.html()
        );
    }
}
