/*! The registration page. */
use std::str::FromStr;

use askama::Template;
use axum::{
    extract::State,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    Form,
};
use axum_extra::extract::PrivateCookieJar;
use axum_htmx::HxRedirect;
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

use crate::{
    auth::set_auth_cookie,
    db::{DbError, Insert},
    models::{NewUser, PasswordHash, RawPassword},
    routes::get_internal_server_error_redirect,
    AppState, HtmlTemplate,
};

use super::endpoints;

#[derive(Template)]
#[template(path = "views/register.html")]
struct RegisterPageTemplate<'a> {
    register_form: RegisterFormTemplate<'a>,
}

#[derive(Template)]
#[template(path = "partials/register/form.html")]
struct RegisterFormTemplate<'a> {
    log_in_route: &'a str,
    create_user_route: &'a str,
    email_input: EmailInputTemplate<'a>,
    password_input: PasswordInputTemplate<'a>,
    confirm_password_input: ConfirmPasswordInputTemplate<'a>,
}

impl Default for RegisterFormTemplate<'_> {
    fn default() -> Self {
        Self {
            log_in_route: endpoints::LOG_IN,
            create_user_route: endpoints::USERS,
            email_input: EmailInputTemplate::default(),
            password_input: PasswordInputTemplate::default(),
            confirm_password_input: ConfirmPasswordInputTemplate::default(),
        }
    }
}

#[derive(Template)]
#[template(path = "partials/register/inputs/email.html")]
struct EmailInputTemplate<'a> {
    value: &'a str,
    error_message: &'a str,
}

impl Default for EmailInputTemplate<'_> {
    fn default() -> Self {
        Self {
            value: "",
            error_message: "",
        }
    }
}

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/password.html")]
struct PasswordInputTemplate<'a> {
    value: &'a str,
    error_message: &'a str,
}

#[derive(Template, Default)]
#[template(path = "partials/register/inputs/confirm_password.html")]
struct ConfirmPasswordInputTemplate<'a> {
    error_message: &'a str,
}

/// Display the registration page.
pub async fn get_register_page() -> Response {
    HtmlTemplate(RegisterPageTemplate {
        register_form: RegisterFormTemplate::default(),
    })
    .into_response()
}

#[derive(Serialize, Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub password: String,
    pub confirm_password: String,
}

pub async fn create_user(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(user_data): Form<RegisterForm>,
) -> Response {
    // Make templates ahead of time that preserve the user's input since they are used multiple times in this function.
    let email_input = EmailInputTemplate {
        value: &user_data.email,
        ..Default::default()
    };

    let password_input = PasswordInputTemplate {
        value: &user_data.password,
        ..Default::default()
    };

    if user_data.password != user_data.confirm_password {
        return HtmlTemplate(RegisterFormTemplate {
            email_input,
            password_input,
            confirm_password_input: ConfirmPasswordInputTemplate {
                error_message: "Passwords do not match",
            },
            ..Default::default()
        })
        .into_response();
    }

    let email = match EmailAddress::from_str(&user_data.email) {
        Ok(email) => email,
        // Due to the client-side validation, the below error will not happen very often, but it still pays to check.
        Err(e) => {
            return HtmlTemplate(RegisterFormTemplate {
                email_input: EmailInputTemplate {
                    value: &user_data.email,
                    error_message: &format!("Invalid email address: {}", e),
                    ..Default::default()
                },
                password_input,
                ..Default::default()
            })
            .into_response();
        }
    };

    let raw_password = match RawPassword::new(user_data.password.to_string()) {
        Ok(password) => password,
        Err(e) => {
            return HtmlTemplate(RegisterFormTemplate {
                email_input,
                password_input: PasswordInputTemplate {
                    value: &user_data.password,
                    error_message: e.to_string().as_ref(),
                },
                ..Default::default()
            })
            .into_response();
        }
    };

    let password_hash = match PasswordHash::new(raw_password) {
        Ok(hash) => hash,
        Err(e) => {
            tracing::error!("an error occurred while hashing a password: {e}");

            return get_internal_server_error_redirect();
        }
    };

    NewUser {
        email,
        password_hash,
    }
    .insert(&state.db_connection().lock().unwrap())
    .map(|user| {
        let jar = set_auth_cookie(jar, user.id());

        (
            StatusCode::SEE_OTHER,
            HxRedirect(Uri::from_static(endpoints::LOG_IN)),
            jar,
        )
    })
    .map_err(|e| match e {
        DbError::DuplicateEmail => HtmlTemplate(RegisterFormTemplate {
            email_input: EmailInputTemplate {
                value: &user_data.email,
                error_message: "The email address is already in use",
                ..Default::default()
            },
            password_input,
            ..Default::default()
        })
        .into_response(),
        DbError::DuplicatePassword => HtmlTemplate(RegisterFormTemplate {
            email_input,
            password_input: PasswordInputTemplate {
                value: &user_data.password,
                error_message: "The password is already in use",
            },
            ..Default::default()
        })
        .into_response(),
        e => {
            tracing::error!("An unhandled error occurred while inserting a new user: {e}");

            get_internal_server_error_redirect()
        }
    })
    .into_response()
}

#[cfg(test)]
mod user_tests {
    use axum::{routing::post, Router};
    use axum_test::TestServer;
    use rusqlite::Connection;
    use serde::{Deserialize, Serialize};

    use crate::{
        db::initialize,
        routes::{
            endpoints,
            register::{create_user, RegisterForm},
        },
        AppState,
    };

    fn get_test_app_config() -> AppState {
        let db_connection =
            Connection::open_in_memory().expect("Could not open database in memory.");
        initialize(&db_connection).expect("Could not initialize database.");

        AppState::new(db_connection, "42".to_string())
    }

    #[derive(Serialize, Deserialize)]
    struct Foo {
        bar: String,
    }

    #[tokio::test]
    async fn create_user_succeeds() {
        let app = Router::new()
            .route(endpoints::USERS, post(create_user))
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
    async fn create_user_fails_when_passwords_do_not_match() {
        let app = Router::new()
            .route(endpoints::USERS, post(create_user))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        let response = server
            .post(endpoints::USERS)
            .form(&RegisterForm {
                email: "foo@".to_string(),
                password: "iamtestingwhethericancreateanewuser".to_string(),
                confirm_password: "thisisadifferentpassword".to_string(),
            })
            .await
            .text();

        assert!(response.to_lowercase().contains("passwords do not match"))
    }
}
