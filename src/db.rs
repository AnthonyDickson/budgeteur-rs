use rusqlite::{Connection, Error};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct User {
    id: i64,
    email: String,
    password: String,
}

impl User {
    pub fn new(id: i64, email: String, password: String) -> User {
        User {
            id,
            email,
            password,
        }
    }

    pub fn id(&self) -> i64 {
        self.id
    }
    pub fn email(&self) -> &str {
        &self.email
    }
    /// Most likely the hashed password.
    pub fn password(&self) -> &str {
        &self.password
    }
}

pub fn initialize(connection: &Connection) -> Result<(), Error> {
    connection.execute(
        "CREATE TABLE user (\
                id INTEGER PRIMARY KEY,\
                email TEXT UNIQUE NOT NULL,\
                password TEXT UNIQUE NOT NULL\
                )",
        (),
    )?;

    Ok(())
}

/// Create a new user in the database.
///
/// It is up to the caller to ensure the password being passed in is properly hashed.
///
/// # Error
/// Will return an error if there was a problem executing the SQL query. This could be due to:
/// - a syntax error in the SQL string,
/// - the email is already in use, or
/// - the password hash is not unique.
pub fn insert_user(
    email: &str,
    password_hash: &str,
    connection: &Connection,
) -> Result<User, Error> {
    // TODO: Check for empty email, invalid email format, invalid password (e.g., too short).
    let mut stmt = connection.prepare("INSERT INTO user (email, password) VALUES (?1, ?2)")?;
    // TODO: Give descriptive error message, e.g, 'email already used'.
    stmt.execute((email, password_hash))?;

    let id = connection.last_insert_rowid();

    Ok(User::new(id, email.to_string(), password_hash.to_string()))
}

pub fn retrieve_user_by_email(email: &str, db_connection: &Connection) -> Option<User> {
    let mut stmt = db_connection
        .prepare("SELECT id, email, password FROM user WHERE email = :email")
        .unwrap();

    let rows = stmt
        .query_map(&[(":email", &email)], |row| {
            let id: i64 = row.get(0)?;
            let email: String = row.get(1)?;
            let password: String = row.get(2)?;

            Ok(User::new(id, email, password))
        })
        .unwrap();

    let row = rows.into_iter().next()?;

    match row {
        Ok(user) => Some(user),
        Err(e) => panic!("{:#?}", e),
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use crate::db::{initialize, insert_user, retrieve_user_by_email};

    #[tokio::test]
    async fn test_create_user() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let email = "hello@world.com";
        let password = "hunter2";

        let inserted_user = insert_user(email, password, &conn).unwrap();

        assert!(inserted_user.id > 0);
        assert_eq!(inserted_user.email, email);
        assert_eq!(inserted_user.password, password);
    }

    #[tokio::test]
    async fn test_create_user_duplicate_email_password() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let email = "hello@world.com";
        let password = "hunter2";

        assert!(insert_user(email, password, &conn).is_ok());
        assert!(insert_user(email, "hunter3", &conn).is_err());
        assert!(insert_user("bye@world.com", password, &conn).is_err());
    }

    #[test]
    fn test_retrieve_user_by_email_does_not_exist() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        insert_user("foo@bar.baz", "hunter2", &conn).unwrap();

        let email = "notavalidemail";

        assert!(retrieve_user_by_email(email, &conn).is_none());
    }

    #[test]
    fn test_retrieve_user_by_email_valid() {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();

        let test_user = insert_user("foo@bar.baz", "hunter2", &conn).unwrap();
        let retrieved_user = retrieve_user_by_email(test_user.email(), &conn).unwrap();

        assert_eq!(retrieved_user, test_user);
    }
}
