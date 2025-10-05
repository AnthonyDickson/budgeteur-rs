//! The registration page for setting the password for accessing the app.
use std::sync::{Arc, Mutex};

use askama::Template;
use axum::{
    Form,
    extract::{FromRef, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_extra::extract::{PrivateCookieJar, cookie::Key};
use axum_htmx::HxRedirect;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use time::{Duration, UtcOffset};

use crate::{
    AppState, PasswordHash, ValidatedPassword,
    auth_cookie::{DEFAULT_COOKIE_DURATION, set_auth_cookie},
    endpoints,
    routing::get_internal_server_error_redirect,
    shared_templates::{PasswordInputTemplate, render},
    state::create_cookie_key,
    user::{count_users, create_user},
};

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/confirm_password.html")]
pub struct ConfirmPasswordInputTemplate<'a> {
    pub error_message: &'a str,
}

#[derive(Template)]
#[template(path = "partials/register/form.html")]
pub struct RegisterFormTemplate<'a> {
    pub log_in_route: &'a str,
    pub create_user_route: &'a str,
    pub password_input: PasswordInputTemplate<'a>,
    pub confirm_password_input: ConfirmPasswordInputTemplate<'a>,
}

impl Default for RegisterFormTemplate<'_> {
    fn default() -> Self {
        Self {
            log_in_route: endpoints::LOG_IN_VIEW,
            create_user_route: endpoints::USERS,
            password_input: PasswordInputTemplate::default(),
            confirm_password_input: ConfirmPasswordInputTemplate::default(),
        }
    }
}

/// The minimum number of characters the password should have to be considered valid on the client side (server-side validation is done on top of this validation).
const PASSWORD_INPUT_MIN_LENGTH: usize = 14;

#[derive(Template)]
#[template(path = "views/register.html")]
struct RegisterPageTemplate<'a> {
    register_form: RegisterFormTemplate<'a>,
}

/// Display the registration page.
pub async fn get_register_page() -> Response {
    render(
        StatusCode::OK,
        RegisterPageTemplate {
            register_form: RegisterFormTemplate {
                password_input: PasswordInputTemplate {
                    min_length: PASSWORD_INPUT_MIN_LENGTH,
                    ..Default::default()
                },
                ..Default::default()
            },
        },
    )
}

/// The state needed for creating a new user.
#[derive(Debug, Clone)]
pub struct RegistrationState {
    /// The key to be used for signing and encrypting private cookies.
    pub cookie_key: Key,
    /// The duration for which cookies used for authentication are valid.
    pub cookie_duration: Duration,
    /// The local timezone as a UTC offset.
    pub local_timezone: UtcOffset,
    pub db_connection: Arc<Mutex<Connection>>,
}

impl RegistrationState {
    /// Create the cookie key from a string and set the default cookie duration.
    pub fn new(
        cookie_secret: &str,
        local_timezone: UtcOffset,
        db_connection: Arc<Mutex<Connection>>,
    ) -> Self {
        Self {
            cookie_key: create_cookie_key(cookie_secret),
            cookie_duration: DEFAULT_COOKIE_DURATION,
            local_timezone,
            db_connection: db_connection.clone(),
        }
    }
}

impl FromRef<AppState> for RegistrationState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            cookie_key: state.cookie_key.clone(),
            cookie_duration: state.cookie_duration,
            local_timezone: state.local_timezone,
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
            return render(
                StatusCode::OK,
                RegisterFormTemplate {
                    confirm_password_input: ConfirmPasswordInputTemplate {
                        error_message: "A password has already been created, please log in with your existing password.",
                    },
                    ..Default::default()
                },
            );
        }
        _ => {}
    }

    // Make templates ahead of time that preserve the user's input since they are used multiple times in this function.
    let password_input = PasswordInputTemplate {
        value: &user_data.password,
        min_length: PASSWORD_INPUT_MIN_LENGTH,
        ..Default::default()
    };

    let validated_password = match ValidatedPassword::new(&user_data.password) {
        Ok(password) => password,
        Err(e) => {
            return render(
                StatusCode::OK,
                RegisterFormTemplate {
                    password_input: PasswordInputTemplate {
                        value: &user_data.password,
                        min_length: PASSWORD_INPUT_MIN_LENGTH,
                        error_message: e.to_string().as_ref(),
                    },
                    ..Default::default()
                },
            );
        }
    };

    if user_data.password != user_data.confirm_password {
        return render(
            StatusCode::OK,
            RegisterFormTemplate {
                password_input,
                confirm_password_input: ConfirmPasswordInputTemplate {
                    error_message: "Passwords do not match",
                },
                ..Default::default()
            },
        );
    }

    let password_hash = match PasswordHash::new(validated_password, PasswordHash::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("an error occurred while hashing a password: {e}");

            return get_internal_server_error_redirect();
        }
    };

    create_user(
        password_hash,
        &state
            .db_connection
            .lock()
            .expect("Could not acquire database lock"),
    )
    .map(|user| {
        let jar = set_auth_cookie(jar, user.id, state.cookie_duration, state.local_timezone);

        match jar {
            Ok(jar) => (
                StatusCode::SEE_OTHER,
                HxRedirect(endpoints::LOG_IN_VIEW.to_owned()),
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

    use crate::{endpoints, register_user::get_register_page};

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
        let want_title = "create password";
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

    use axum::{
        Form, Router,
        body::Body,
        extract::State,
        http::{Response, StatusCode},
        response::IntoResponse,
        routing::post,
    };
    use axum_extra::extract::PrivateCookieJar;
    use axum_test::TestServer;
    use rusqlite::Connection;
    use time::UtcOffset;

    use crate::{
        PasswordHash, endpoints,
        register_user::{RegisterForm, register_user},
        user::{create_user, create_user_table},
    };

    use super::RegistrationState;

    fn get_test_app_config() -> RegistrationState {
        let connection =
            Connection::open_in_memory().expect("Could not open in-memory SQLite database");
        create_user_table(&connection).expect("Could not create user table");

        RegistrationState::new("42", UtcOffset::UTC, Arc::new(Mutex::new(connection)))
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
            paragraph_text.contains("existing password"),
            "'{paragraph_text}' does not contain the text 'existing password'"
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
