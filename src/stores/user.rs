//! Defines the user store trait.

use email_address::EmailAddress;

use crate::{
    Error,
    models::{PasswordHash, User, UserID},
};

/// Handles the creation and retrieval of User objects.
pub trait UserStore {
    /// Create a new user.
    fn create(&mut self, email: EmailAddress, password_hash: PasswordHash) -> Result<User, Error>;

    /// Get the number of registered accounts/users.
    fn count(&self) -> Result<usize, Error>;

    /// Get a user by their ID.
    fn get(&self, id: UserID) -> Result<User, Error>;

    /// Get a user by their email.
    ///
    /// Returns [Error::NotFound] if no user with the given email exists.
    fn get_by_email(&self, email: &EmailAddress) -> Result<User, Error>;
}
