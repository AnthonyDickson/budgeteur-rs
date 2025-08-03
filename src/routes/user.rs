//! The registration page for creating a new user account.
use std::str::FromStr;

use axum::{
    Form,
    extract::State,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
};
use axum_extra::extract::PrivateCookieJar;
use axum_htmx::HxRedirect;
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

use crate::{
    Error,
    auth::cookie::set_auth_cookie,
    models::{PasswordHash, ValidatedPassword},
    routes::get_internal_server_error_redirect,
    state::RegistrationState,
    user::{count_users, create_user, get_user_by_email},
};

use super::{
    endpoints,
    templates::{
        ConfirmPasswordInputTemplate, EmailInputTemplate, PasswordInputTemplate,
        RegisterFormTemplate,
    },
};

/// The minimum number of characters the password should have to be considered valid on the client side (server-side validation is done on top of this validation).
const PASSWORD_INPUT_MIN_LENGTH: usize = 14;

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
mod tests {
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
        routes::{
            endpoints,
            user::{RegisterForm, register_user},
        },
        state::RegistrationState,
        user::{create_user, create_user_table},
    };

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
