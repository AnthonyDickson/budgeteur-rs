use serde::{Deserialize, Serialize};

pub type DatabaseID = i64;

// TODO: Implement Email newtype that wraps a string.
// #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
// pub struct Email(String);

// #[derive(Error, Debug, Clone, PartialEq)]
// #[error("{0} is not a valid email address")]
// pub struct EmailAddressError(String);

// impl EmailAddress {
//     pub fn new(raw_email: &str) -> Result<Self, EmailAddressError> {
//         if email_regex().is_match(raw_email) {
//             Ok(Self(raw_email.into()))
//         } else {
//             Err(EmailAddressError(raw_email))
//         }
//     }
// }
// TODO: Implement password and password hash newtypes that wrap a string + type state to ensure that raw passwords are not accidentally stored as a password hash (RawPassword should have a function that hashes and salts the password and returns a PasswordHash).

/// A user of the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: DatabaseID,
    email: String,
    password_hash: String,
}

impl User {
    pub fn new(id: DatabaseID, email: String, password_hash: String) -> Self {
        User {
            id,
            email,
            password_hash,
        }
    }

    pub fn id(&self) -> DatabaseID {
        self.id
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_transaction() {
        let id = 1;
        let email = "foo@bar.baz".to_string();
        let password_hash = "definitelyapasswordhash".to_string();

        let user = User::new(id, email.clone(), password_hash.clone());

        assert_eq!(user.id, id);
        assert_eq!(user.email, email);
        assert_eq!(user.password_hash, password_hash);
    }
}
