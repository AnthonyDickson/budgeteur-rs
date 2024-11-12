//! This file defines the high-level log-in route logic.
//! The auth module handles the lower level authentication and cookie auth logic.

use askama::Template;
use axum::{
    extract::State,
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    Form,
};
use axum_extra::extract::PrivateCookieJar;
use axum_htmx::HxRedirect;

use crate::{
    auth::{set_auth_cookie, verify_credentials, AuthError, LogInData},
    stores::{CategoryStore, TransactionStore, UserStore},
    AppState,
};

use super::{
    endpoints,
    templates::{EmailInputTemplate, PasswordInputTemplate},
};

/// Renders a log-in form with client-side and server-side validation.
#[derive(Template)]
#[template(path = "partials/log_in/form.html")]
struct LogInFormTemplate<'a> {
    email_input: EmailInputTemplate<'a>,
    password_input: PasswordInputTemplate<'a>,
    log_in_route: &'a str,
    register_route: &'a str,
}

impl Default for LogInFormTemplate<'_> {
    fn default() -> Self {
        Self {
            email_input: Default::default(),
            password_input: Default::default(),
            log_in_route: endpoints::LOG_IN,
            register_route: endpoints::REGISTER,
        }
    }
}

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

/// Handler for log-in requests via the POST method.
///
/// On a successful log-in request, the auth cookie set and the client is redirected to the dashboard page.
/// Otherwise, the form is return with an error message explaining the problem.
///
/// # Errors
///
/// This function will return an error in a few situations.
/// - The email does not belong to a registered user.
/// - The password is not correct.
/// - An internal error occurred when verifying the password.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn post_log_in<C, T, U>(
    State(mut state): State<AppState<C, T, U>>,
    jar: PrivateCookieJar,
    Form(user_data): Form<LogInData>,
) -> Response
where
    C: CategoryStore + Send + Sync,
    T: TransactionStore + Send + Sync,
    U: UserStore + Send + Sync,
{
    verify_credentials(user_data.clone(), state.user_store())
        .map(|user| {
            let jar = set_auth_cookie(jar, user.id());

            (
                StatusCode::SEE_OTHER,
                HxRedirect(Uri::from_static(endpoints::DASHBOARD)),
                jar,
            )
        })
        .map_err(|e| LogInFormTemplate {
            email_input: EmailInputTemplate {
                value: &user_data.email,
                error_message: "",
            },
            password_input: PasswordInputTemplate {
                value: "",
                min_length: 0,
                error_message: match e {
                    AuthError::InvalidCredentials => "Incorrect email or password.",
                    AuthError::InternalError => {
                        "An internal error occurred. Please try again later."
                    }
                },
            },
            ..Default::default()
        })
        .into_response()
}

#[cfg(test)]
mod log_in_tests {
    use axum::{http::StatusCode, routing::post, Router};
    use axum_test::TestServer;
    use email_address::EmailAddress;

    use crate::{
        auth::LogInData,
        models::{PasswordHash, User, UserID, ValidatedPassword},
        routes::{endpoints, log_in::post_log_in},
        stores::{CategoryStore, TransactionStore, UserError, UserStore},
        AppState,
    };

    #[derive(Clone)]
    struct StubUserStore {
        users: Vec<User>,
    }

    impl UserStore for StubUserStore {
        fn create(
            &mut self,
            email: email_address::EmailAddress,
            password_hash: PasswordHash,
        ) -> Result<User, UserError> {
            let next_id = match self.users.last() {
                Some(user) => UserID::new(user.id().as_i64() + 1),
                _ => UserID::new(0),
            };

            let user = User::new(next_id, email, password_hash);
            self.users.push(user.clone());

            Ok(user)
        }

        fn get(&self, id: UserID) -> Result<User, UserError> {
            self.users
                .iter()
                .find(|user| user.id() == id)
                .ok_or(UserError::NotFound)
                .map(|user| user.to_owned())
        }

        fn get_by_email(&self, email: &email_address::EmailAddress) -> Result<User, UserError> {
            self.users
                .iter()
                .find(|user| user.email() == email)
                .ok_or(UserError::NotFound)
                .map(|user| user.to_owned())
        }
    }

    #[derive(Clone)]
    struct DummyCategoryStore {}

    impl CategoryStore for DummyCategoryStore {
        fn create(
            &self,
            _name: crate::models::CategoryName,
            _user_id: crate::models::UserID,
        ) -> Result<crate::models::Category, crate::models::CategoryError> {
            todo!()
        }

        fn select(
            &self,
            _category_id: crate::models::DatabaseID,
        ) -> Result<crate::models::Category, crate::models::CategoryError> {
            todo!()
        }

        fn get_by_user(
            &self,
            _user_id: crate::models::UserID,
        ) -> Result<Vec<crate::models::Category>, crate::models::CategoryError> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyTransactionStore {}

    impl TransactionStore for DummyTransactionStore {
        fn create(
            &mut self,
            _amount: f64,
            _user_id: crate::models::UserID,
        ) -> Result<crate::models::Transaction, crate::models::TransactionError> {
            todo!()
        }

        fn create_from_builder(
            &mut self,
            _builder: crate::models::TransactionBuilder,
        ) -> Result<crate::models::Transaction, crate::models::TransactionError> {
            todo!()
        }

        fn get(
            &self,
            _id: crate::models::DatabaseID,
        ) -> Result<crate::models::Transaction, crate::models::TransactionError> {
            todo!()
        }

        fn get_by_user_id(
            &self,
            _user_id: crate::models::UserID,
        ) -> Result<Vec<crate::models::Transaction>, crate::models::TransactionError> {
            todo!()
        }
    }

    type TestAppState = AppState<DummyCategoryStore, DummyTransactionStore, StubUserStore>;

    fn get_test_app_config() -> TestAppState {
        let mut state = AppState::new(
            "42",
            DummyCategoryStore {},
            DummyTransactionStore {},
            StubUserStore { users: vec![] },
        );

        state
            .user_store()
            .create(
                EmailAddress::new_unchecked("test@test.com"),
                PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string()), 4).unwrap(),
            )
            .unwrap();

        state
    }

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(post_log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .assert_status(StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn log_in_fails_with_missing_credentials() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(post_log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .content_type("application/x-www-form-urlencoded")
            .await
            .assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_email() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(post_log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "wrong@email.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .text()
            .contains("invalid");
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_password() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(post_log_in))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "wrongpassword".to_string(),
            })
            .await
            .text()
            .contains("invalid");
    }
}
