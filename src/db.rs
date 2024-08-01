use rusqlite::{Connection, Error};

struct User {
    id: i32,
    email: String,
    password: String,
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

#[cfg(test)]
mod tests {
    use axum::BoxError;
    use rusqlite::Connection;

    use crate::db::{initialize, User};

    #[tokio::test]
    async fn test_create_and_retrieve_user_from_db() -> Result<(), BoxError> {
        let conn = Connection::open_in_memory()?;
        initialize(&conn)?;

        let credentials = User {
            id: 0,
            email: "hello@world.com".to_string(),
            password: "hunter2".to_string(),
        };

        conn.execute(
            "INSERT INTO user (email, password) VALUES (?1, ?2)",
            (&credentials.email, &credentials.password),
        )?;

        let mut stmt = conn.prepare("SELECT id, email, password FROM user")?;
        let user_iter = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                email: row.get(1)?,
                password: row.get(2)?,
            })
        })?;

        if let Some(Ok(user)) = user_iter.into_iter().next() {
            assert_eq!(user.email, credentials.email);
            assert_eq!(user.password, credentials.password);
        } else {
            panic!();
        }

        Ok(())
    }
}
