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
                    AuthError::InvalidCredentials => INVALID_CREDENTIALS_ERROR_MSG,
                    AuthError::InternalError => {
                        "An internal error occurred. Please try again later."
                    }
                },
            },
            ..Default::default()
        })
        .into_response()
}

const INVALID_CREDENTIALS_ERROR_MSG: &str = "Incorrect email or password.";

#[cfg(test)]
mod log_in_tests {
    use axum::{
        body::Body,
        extract::State,
        http::{Response, StatusCode},
        routing::post,
        Form, Router,
    };
    use axum_extra::extract::{cookie::Cookie, PrivateCookieJar};
    use axum_htmx::HX_REDIRECT;
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use time::{Duration, OffsetDateTime};

    use crate::{
        auth::{LogInData, COOKIE_USER_ID},
        models::{
            Category, CategoryError, CategoryName, DatabaseID, PasswordHash, Transaction,
            TransactionBuilder, TransactionError, User, UserID, ValidatedPassword,
        },
        routes::{
            endpoints,
            log_in::{post_log_in, INVALID_CREDENTIALS_ERROR_MSG},
        },
        stores::{
            transaction::TransactionQuery, CategoryStore, TransactionStore, UserError, UserStore,
        },
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
        fn create(&self, _name: CategoryName, _user_id: UserID) -> Result<Category, CategoryError> {
            todo!()
        }

        fn get(&self, _category_id: DatabaseID) -> Result<Category, CategoryError> {
            todo!()
        }

        fn get_by_user(&self, _user_id: UserID) -> Result<Vec<Category>, CategoryError> {
            todo!()
        }
    }

    #[derive(Clone)]
    struct DummyTransactionStore {}

    impl TransactionStore for DummyTransactionStore {
        fn create(
            &mut self,
            _amount: f64,
            _user_id: UserID,
        ) -> Result<Transaction, TransactionError> {
            todo!()
        }

        fn create_from_builder(
            &mut self,
            _builder: TransactionBuilder,
        ) -> Result<Transaction, TransactionError> {
            todo!()
        }

        fn get(&self, _id: DatabaseID) -> Result<Transaction, TransactionError> {
            todo!()
        }

        fn get_by_user_id(&self, _user_id: UserID) -> Result<Vec<Transaction>, TransactionError> {
            todo!()
        }

        fn get_query(
            &self,
            _filter: TransactionQuery,
        ) -> Result<Vec<Transaction>, TransactionError> {
            todo!()
        }
    }

    type TestAppState = AppState<DummyCategoryStore, DummyTransactionStore, StubUserStore>;

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let response = new_log_in_request("test@test.com", "test").await;

        assert_hx_redirect(&response, endpoints::DASHBOARD);
        assert_set_cookie(&response);
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
        let response = new_log_in_request("wrong@email.com", "test").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_message(response, INVALID_CREDENTIALS_ERROR_MSG).await;
    }

    #[tokio::test]
    async fn log_in_fails_with_incorrect_password() {
        let response = new_log_in_request("test@test.com", "wrongpassword").await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_body_contains_message(response, INVALID_CREDENTIALS_ERROR_MSG).await;
    }

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
                PasswordHash::new(ValidatedPassword::new_unchecked("test"), 4).unwrap(),
            )
            .unwrap();

        state
    }

    async fn new_log_in_request(email: &str, password: &str) -> Response<Body> {
        let state = get_test_app_config();
        let jar = PrivateCookieJar::new(state.cookie_key().to_owned());
        let form = LogInData {
            email: email.to_string(),
            password: password.to_string(),
        };

        post_log_in(State(state), jar, Form(form)).await
    }

    fn assert_hx_redirect(response: &Response<Body>, want_location: &str) {
        let redirect_location = response.headers().get(HX_REDIRECT).unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(redirect_location, want_location);
    }

    fn assert_set_cookie(response: &Response<Body>) {
        let cookie_string = response
            .headers()
            .get("set-cookie")
            .unwrap()
            .to_str()
            .unwrap();
        let auth_cookie = Cookie::parse(cookie_string).unwrap();

        assert_eq!(auth_cookie.name(), COOKIE_USER_ID);
        assert!(auth_cookie.max_age() > Some(Duration::ZERO));
        assert!(auth_cookie.expires().unwrap().datetime() > Some(OffsetDateTime::now_utc()));
    }

    async fn assert_body_contains_message(response: Response<Body>, message: &str) {
        let body = response.into_body();
        let body = axum::body::to_bytes(body, usize::MAX).await.unwrap();

        let text = String::from_utf8_lossy(&body).to_string();

        assert!(
            text.contains(message),
            "response body should contain the text '{}' but got {}",
            message,
            text
        );
    }
}
