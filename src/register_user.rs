//! The registration page for creating a new user account.
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

use askama::Template;
use axum::{
    Form,
    extract::{FromRef, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_extra::extract::{PrivateCookieJar, cookie::Key};
use axum_htmx::HxRedirect;
use email_address::EmailAddress;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use time::Duration;

use crate::{
    AppState, Error,
    auth::cookie::{DEFAULT_COOKIE_DURATION, set_auth_cookie},
    models::{PasswordHash, ValidatedPassword},
    routes::{
        endpoints, get_internal_server_error_redirect,
        templates::{
            ConfirmPasswordInputTemplate, EmailInputTemplate, PasswordInputTemplate,
            RegisterFormTemplate,
        },
    },
    state::create_cookie_key,
    stores::{CategoryStore, TransactionStore},
    user::{count_users, create_user, get_user_by_email},
};

/// The minimum number of characters the password should have to be considered valid on the client side (server-side validation is done on top of this validation).
const PASSWORD_INPUT_MIN_LENGTH: usize = 14;

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

/// The state needed for creating a new user.
#[derive(Debug, Clone)]
pub struct RegistrationState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
    pub db_connection: Arc<Mutex<Connection>>,
}

impl RegistrationState {
    /// Create the cookie key from a string and set the default cookie duration.
    pub fn new(cookie_secret: &str, db_connection: Arc<Mutex<Connection>>) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            db_connection: db_connection.clone(),
        }
    }
}

impl<C, T> FromRef<AppState<C, T>> for RegistrationState
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
{
    fn from_ref(state: &AppState<C, T>) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            cookie_duration: state.cookie_duration,
            db_connection: state.db_connection.clone(),
        }
    }
}

// this impl tells `PrivateCookieJar` how to access the key from our state
impl FromRef<RegistrationState> for Key {
    fn from_ref(state: &RegistrationState) -> Self {
        state.cookie_key.clone()
    }
}

#[derive(Serialize, Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub password: String,
    pub confirm_password: String,
}

pub async fn register_user(
    State(state): State<RegistrationState>,
    jar: PrivateCookieJar,
    Form(user_data): Form<RegisterForm>,
) -> Response {
    match count_users(
        &state
            .db_connection
            .lock()
            .expect("Could not acquire database lock"),
    ) {
        Ok(count) if count >= 1 => {
            return RegisterFormTemplate {
                confirm_password_input: ConfirmPasswordInputTemplate {
                    error_message: "An account has already been created, please log in with your existing account.",
                },
                ..Default::default()
            }.into_response();
        }
        _ => {}
    }

    // Make templates ahead of time that preserve the user's input since they are used multiple times in this function.
    let email_input = EmailInputTemplate {
        value: &user_data.email,
        ..Default::default()
    };

    let password_input = PasswordInputTemplate {
        value: &user_data.password,
        min_length: PASSWORD_INPUT_MIN_LENGTH,
        ..Default::default()
    };

    let email = match EmailAddress::from_str(&user_data.email) {
        Ok(email) => email,
        // Due to the client-side validation, the below error will not happen very often, but it still pays to check.
        Err(e) => {
            return RegisterFormTemplate {
                email_input: EmailInputTemplate {
                    value: &user_data.email,
                    error_message: &format!("Invalid email address: {}", e),
                },
                password_input,
                ..Default::default()
            }
            .into_response();
        }
    };

    if get_user_by_email(
        email.as_ref(),
        &state
            .db_connection
            .lock()
            .expect("Could not acquire database lock"),
    )
    .is_ok()
    {
        return RegisterFormTemplate {
            email_input: EmailInputTemplate {
                value: &user_data.email,
                error_message: "The email address is already in use",
            },
            password_input,
            ..Default::default()
        }
        .into_response();
    }

    let validated_password = match ValidatedPassword::new(&user_data.password) {
        Ok(password) => password,
        Err(e) => {
            return RegisterFormTemplate {
                email_input,
                password_input: PasswordInputTemplate {
                    value: &user_data.password,
                    min_length: PASSWORD_INPUT_MIN_LENGTH,
                    error_message: e.to_string().as_ref(),
                },
                ..Default::default()
            }
            .into_response();
        }
    };

    if user_data.password != user_data.confirm_password {
        return RegisterFormTemplate {
            email_input,
            password_input,
            confirm_password_input: ConfirmPasswordInputTemplate {
                error_message: "Passwords do not match",
            },
            ..Default::default()
        }
        .into_response();
    }

    let password_hash = match PasswordHash::new(validated_password, PasswordHash::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("an error occurred while hashing a password: {e}");

            return get_internal_server_error_redirect();
        }
    };

    create_user(
        email,
        password_hash,
        &state
            .db_connection
            .lock()
            .expect("Could not acquire database lock"),
    )
    .map(|user| {
        let jar = set_auth_cookie(jar, user.id(), state.cookie_duration);

        match jar {
            Ok(jar) => (
                StatusCode::SEE_OTHER,
                HxRedirect(Uri::from_static(endpoints::LOG_IN_VIEW)),
                jar,
            )
                .into_response(),
            Err(e) => {
                tracing::error!("An error occurred while setting the auth cookie: {e}");

                get_internal_server_error_redirect()
            }
        }
    })
    .map_err(|e| match e {
        Error::DuplicateEmail => RegisterFormTemplate {
            email_input: EmailInputTemplate {
                value: &user_data.email,
                error_message: "The email address is already in use",
            },
            password_input,
            ..Default::default()
        }
        .into_response(),
        e => {
            tracing::error!("An unhandled error occurred while inserting a new user: {e}");

            get_internal_server_error_redirect()
        }
    })
    .into_response()
}

#[cfg(test)]
mod get_register_page_tests {
    use axum::{
        body::Body,
        http::{Response, StatusCode, header::CONTENT_TYPE},
    };
    use scraper::Html;
    use serde::{Deserialize, Serialize};

    use crate::{register_user::get_register_page, routes::endpoints};

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
        assert_valid_html(&document);

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

    #[track_caller]
    fn assert_valid_html(html: &Html) {
        assert!(
            html.errors.is_empty(),
            "Got HTML parsing errors: {:?}",
            html.errors
        );
    }
}

#[cfg(test)]
mod register_user_tests {
    use std::sync::{Arc, Mutex};

    use askama_axum::IntoResponse;
    use axum::{
        Form, Router,
        body::Body,
        extract::State,
        http::{Response, StatusCode},
        routing::post,
    };
    use axum_extra::extract::PrivateCookieJar;
    use axum_test::TestServer;
    use rusqlite::Connection;
    use serde::{Deserialize, Serialize};

    use crate::{
        models::PasswordHash,
        register_user::{RegisterForm, register_user},
        routes::endpoints,
        user::{create_user, create_user_table},
    };

    use super::RegistrationState;

    fn get_test_app_config() -> RegistrationState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_user_table(&connection).expect("Could not create user table");

        RegistrationState::new("42", Arc::new(Mutex::new(connection)))
    }

    #[derive(Serialize, Deserialize)]
    struct Foo {
        bar: String,
    }

    #[tokio::test]
    async fn create_user_succeeds() {
        let app = Router::new()
            .route(endpoints::USERS, post(register_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: "foo@bar.baz".to_string(),
                password: "iamtestingwhethericancreateanewuser".to_string(),
                confirm_password: "iamtestingwhethericancreateanewuser".to_string(),
            })
            .await
            .assert_status_see_other();
    }

    #[tokio::test]
    async fn create_user_fails_with_existing_user() {
        let state = get_test_app_config();
        create_user(
            "test@test.com".parse().unwrap(),
            PasswordHash::from_raw_password("foobarbazquxgobbledygook", 4).unwrap(),
            &state
                .db_connection
                .lock()
                .expect("Could not acquire database connection"),
        )
        .expect("Could not create test user");

        let response = register_user(
            State(state.clone()),
            PrivateCookieJar::new(state.cookie_key),
            Form(RegisterForm {
                email: "foo.bar.baz".to_string(),
                password: "averystrongandsecurepassword".to_string(),
                confirm_password: "averystrongandsecurepassword".to_string(),
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        let fragment = parse_html(response).await;
        let p_selector = scraper::Selector::parse("p.text-red-500").unwrap();
        let paragraphs = fragment.select(&p_selector).collect::<Vec<_>>();
        assert_eq!(paragraphs.len(), 1, "want 1 p, got {}", paragraphs.len());
        let paragraph = paragraphs.first().unwrap();
        let paragraph_text = paragraph.text().collect::<String>().to_lowercase();
        assert!(
            paragraph_text.contains("existing account"),
            "'{paragraph_text}' does not contain the text 'invalid email address'"
        );
    }

    #[tokio::test]
    async fn create_user_fails_with_invalid_email() {
        let state = get_test_app_config();
        let response = register_user(
            State(state.clone()),
            PrivateCookieJar::new(state.cookie_key),
            Form(RegisterForm {
                email: "foo.bar.baz".to_string(),
                password: "averystrongandsecurepassword".to_string(),
                confirm_password: "averystrongandsecurepassword".to_string(),
            }),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);

        let fragment = parse_html(response).await;

        let p_selector = scraper::Selector::parse("p.text-red-500").unwrap();
        let paragraphs = fragment.select(&p_selector).collect::<Vec<_>>();
        assert_eq!(paragraphs.len(), 1, "want 1 p, got {}", paragraphs.len());
        let paragraph = paragraphs.first().unwrap();
        let paragraph_text = paragraph.text().collect::<String>().to_lowercase();
        assert!(
            paragraph_text.contains("invalid email address"),
            "'{paragraph_text}' does not contain the text 'invalid email address'"
        );
    }

    #[tokio::test]
    async fn create_user_fails_when_password_is_empty() {
        let app = Router::new()
            .route(endpoints::USERS, post(register_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: "foo@bar.baz".to_string(),
                password: "".to_string(),
                confirm_password: "".to_string(),
            })
            .await
            .text();

        let fragment = parse_html(response.into_response()).await;

        let p_selector = scraper::Selector::parse("p.text-red-500").unwrap();
        let paragraphs = fragment.select(&p_selector).collect::<Vec<_>>();
        assert_eq!(paragraphs.len(), 1, "want 1 p, got {}", paragraphs.len());
        let paragraph = paragraphs.first().unwrap();
        let paragraph_text = paragraph.text().collect::<String>().to_lowercase();
        assert!(
            paragraph_text.contains("password is too weak"),
            "'{paragraph_text}' does not contain the text 'password is too weak'"
        );
    }

    #[tokio::test]
    async fn create_user_fails_when_password_is_weak() {
        let app = Router::new()
            .route(endpoints::USERS, post(register_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: "foo@bar.baz".to_string(),
                password: "foo".to_string(),
                confirm_password: "foo".to_string(),
            })
            .await
            .text();

        let fragment = parse_html(response.into_response()).await;

        let p_selector = scraper::Selector::parse("p.text-red-500").unwrap();
        let paragraphs = fragment.select(&p_selector).collect::<Vec<_>>();
        assert_eq!(paragraphs.len(), 1, "want 1 p, got {}", paragraphs.len());
        let paragraph = paragraphs.first().unwrap();
        let paragraph_text = paragraph.text().collect::<String>().to_lowercase();
        assert!(
            paragraph_text.contains("password is too weak"),
            "'{paragraph_text}' does not contain the text 'password is too weak'"
        );
    }

    #[tokio::test]
    async fn create_user_fails_when_passwords_do_not_match() {
        let app = Router::new()
            .route(endpoints::USERS, post(register_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: "foo@bar.baz".to_string(),
                password: "iamtestingwhethericancreateanewuser".to_string(),
                confirm_password: "thisisadifferentpassword".to_string(),
            })
            .await
            .text();

        let fragment = parse_html(response.into_response()).await;

        let p_selector = scraper::Selector::parse("p.text-red-500").unwrap();
        let paragraphs = fragment.select(&p_selector).collect::<Vec<_>>();
        assert_eq!(paragraphs.len(), 1, "want 1 p, got {}", paragraphs.len());
        let paragraph = paragraphs.first().unwrap();
        let paragraph_text = paragraph.text().collect::<String>().to_lowercase();
        assert!(
            paragraph_text.contains("passwords do not match"),
            "'{paragraph_text}' does not contain the text 'passwords do not match'"
        );
    }

    async fn parse_html(response: Response<Body>) -> scraper::Html {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();
        let text = String::from_utf8_lossy(&body).to_string();

        scraper::Html::parse_fragment(&text)
    }
}
