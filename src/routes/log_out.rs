//! This file defines the high-level log-out route logic.
//! The underlying auth logic is handled by the auth module.

use axum::response::{IntoResponse, Redirect, Response};
use axum_extra::extract::PrivateCookieJar;

use crate::auth::invalidate_auth_cookie;

use super::endpoints;

/// Invalidate the auth cookie and redirect the client to the log-in page.
pub async fn get_log_out(jar: PrivateCookieJar) -> Response {
    let jar = invalidate_auth_cookie(jar);

    (jar, Redirect::to(endpoints::LOG_IN)).into_response()
}

#[cfg(test)]
mod log_out_tests {
    use axum::{routing::post, Router};
    use axum_extra::extract::cookie::Expiration;
    use axum_test::TestServer;
    use email_address::EmailAddress;
    use time::{Duration, OffsetDateTime};

    use crate::{
        auth::{LogInData, COOKIE_USER_ID},
        models::{PasswordHash, User, UserID, ValidatedPassword},
        routes::{endpoints, log_in::post_log_in, log_out::get_log_out},
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
            &self,
            _amount: f64,
            _user_id: crate::models::UserID,
        ) -> Result<crate::models::Transaction, crate::models::TransactionError> {
            todo!()
        }

        fn create_from_builder(
            &self,
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
                PasswordHash::new(ValidatedPassword::new_unchecked("test".to_string())).unwrap(),
            )
            .unwrap();

        state
    }

    #[tokio::test]
    async fn log_out_invalidates_auth_cookie_and_redirects() {
        let app = Router::new()
            .route(endpoints::LOG_IN, post(post_log_in))
            .route(endpoints::LOG_OUT, post(get_log_out))
            .with_state(get_test_app_config());

        let server = TestServer::new(app).expect("Could not create test server.");

        server
            .post(endpoints::LOG_IN)
            .form(&LogInData {
                email: "test@test.com".to_string(),
                password: "test".to_string(),
            })
            .await
            .assert_status_see_other();

        let response = server.post(endpoints::LOG_OUT).await;
        response.assert_status_see_other();

        let auth_cookie = response.cookie(COOKIE_USER_ID);

        assert_eq!(auth_cookie.max_age(), Some(Duration::ZERO));
        assert_eq!(
            auth_cookie.expires(),
            Some(Expiration::DateTime(OffsetDateTime::UNIX_EPOCH))
        );
    }
}
