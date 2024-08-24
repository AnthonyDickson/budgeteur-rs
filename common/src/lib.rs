use serde::{Deserialize, Serialize};

mod category;
pub use category::{Category, CategoryName, CategoryNameError};

mod email;
pub use email::{Email, EmailAddressError};

mod password;
pub use password::{PasswordError, PasswordHash, RawPassword};

pub type DatabaseID = i64;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserID(i64);

impl UserID {
    pub fn new(id: i64) -> Self {
        Self(id)
    }

    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

/// A user of the application.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: UserID,
    email: Email,
    password_hash: PasswordHash,
}

impl User {
    pub fn new(id: UserID, email: Email, password_hash: PasswordHash) -> Self {
        User {
            id,
            email,
            password_hash,
        }
    }

    pub fn id(&self) -> UserID {
        self.id
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_transaction() {
        let id = UserID::new(1);
        let email = Email::new("foo@bar.baz").unwrap();
        let password_hash =
            unsafe { PasswordHash::new_unchecked("definitelyapasswordhash".to_string()) };

        let user = User::new(id, email.clone(), password_hash.clone());

        assert_eq!(user.id, id);
        assert_eq!(user.email, email);
        assert_eq!(user.password_hash, password_hash);
    }
}
