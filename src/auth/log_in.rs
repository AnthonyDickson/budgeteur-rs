//! Data and functions for logging in a user.

use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

use crate::{models::User, stores::UserStore, Error};

/// The raw data entered by the user in the log-in form.
///
/// The email and password are stored as plain strings. There is no need for validation here since
/// they will be compared against the email and password in the database, which have been verified.
#[derive(Clone, Serialize, Deserialize)]
pub struct LogInData {
    /// Email entered during log-in.
    pub email: String,
    /// Password entered during log-in.
    pub password: String,
    /// Whether to extend the initial auth cookie duration.
    ///
    /// This value comes from a checkbox, so it either has a string value or is not set
    /// (see the [MDN docs](https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input/checkbox#value_2)).
    /// The `Some` variant should be interpreted as `true` irregardless of the
    /// string value, and the `None` variant should be interpreted as `false`.
    pub remember_me: Option<String>,
}

/// Verify the user `credentials` against the data in the database `connection`.
///
/// # Errors
///
/// This function will return an error in a few situations.
/// - The email does not belong to a registered user.
/// - The password is not correct.
/// - An internal error occurred when verifying the password.
pub fn verify_credentials(credentials: LogInData, store: &impl UserStore) -> Result<User, Error> {
    let email: EmailAddress = credentials
        .email
        .parse()
        .map_err(|_| Error::InvalidCredentials)?;

    let user = store.get_by_email(&email).map_err(|e| match e {
        Error::NotFound => Error::InvalidCredentials,
        error => error,
    })?;

    let is_password_correct = user
        .password_hash()
        .verify(&credentials.password)
        .map_err(|e| Error::InternalError(e.to_string()))?;

    match is_password_correct {
        true => Ok(user),
        false => Err(Error::InvalidCredentials),
    }
}

#[cfg(test)]
mod log_in_tests {
    use email_address::EmailAddress;

    use crate::{
        auth::log_in::{verify_credentials, LogInData},
        models::{PasswordHash, User, UserID},
        stores::UserStore,
        Error,
    };

    #[derive(Clone)]
    struct StubUserStore {
        users: Vec<User>,
    }

    impl UserStore for StubUserStore {
        fn create(
            &mut self,
            _email: email_address::EmailAddress,
            _password_hash: PasswordHash,
        ) -> Result<User, Error> {
            todo!()
        }

        fn get(&self, _id: UserID) -> Result<User, Error> {
            todo!()
        }

        fn get_by_email(&self, email: &email_address::EmailAddress) -> Result<User, Error> {
            self.users
                .iter()
                .find(|user| user.email() == email)
                .ok_or(Error::NotFound)
                .map(|user| user.to_owned())
        }
    }

    #[tokio::test]
    async fn log_in_succeeds_with_valid_credentials() {
        let email: EmailAddress = "foo@bar.baz".parse().expect("Could not parse email");
        let password = "averysafeandsecurepassword";
        let password_hash =
            PasswordHash::from_raw_password(password, 4).expect("Failed to create password hash");

        let store = StubUserStore {
            users: vec![User::new(UserID::new(0), email.clone(), password_hash)],
        };

        let user_data = LogInData {
            email: email.to_string(),
            password: password.to_string(),
            remember_me: None,
        };

        assert!(verify_credentials(user_data, &store).is_ok());
    }

    #[tokio::test]
    async fn log_in_fails_with_invalid_credentials() {
        let store = StubUserStore { users: vec![] };
        let user_data = LogInData {
            email: "wrongemail@gmail.com".to_string(),
            password: "definitelyNotTheCorrectPassword".to_string(),
            remember_me: None,
        };

        let result = verify_credentials(user_data, &store);

        assert!(matches!(result, Err(Error::InvalidCredentials)));
    }
}
