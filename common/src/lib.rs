use serde::{Deserialize, Serialize};

mod email;
pub use email::{Email, EmailAddressError};

pub type DatabaseID = i64;

// TODO: Implement password and password hash newtypes that wrap a string + type state to ensure that raw passwords are not accidentally stored as a password hash (RawPassword should have a function that hashes and salts the password and returns a PasswordHash).

/// A user of the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: DatabaseID,
    email: Email,
    password_hash: String,
}

impl User {
    pub fn new(id: DatabaseID, email: Email, password_hash: String) -> Self {
        User {
            id,
            email,
            password_hash,
        }
    }

    pub fn id(&self) -> DatabaseID {
        self.id
    }

    pub fn email(&self) -> &Email {
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
        let email = Email::new("foo@bar.baz").unwrap();
        let password_hash = "definitelyapasswordhash".to_string();

        let user = User::new(id, email.clone(), password_hash.clone());

        assert_eq!(user.id, id);
        assert_eq!(user.email, email);
        assert_eq!(user.password_hash, password_hash);
    }
}
