//! Tag preference storage shared across features.
//!
//! Currently stores tag IDs that should be excluded from summary calculations.

use rusqlite::Connection;

use crate::{Error, tag::TagId};

const EXCLUDED_TAGS_TABLE: &str = "dashboard_excluded_tags";

/// Create the excluded tags table in the database.
///
/// This table stores which tags should be excluded from summary calculations.
/// Uses tag_id as the primary key to ensure each tag can only be excluded once.
///
/// # Errors
/// Returns an error if the table cannot be created or if there is an SQL error.
pub fn create_excluded_tags_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        &format!(
            "CREATE TABLE IF NOT EXISTS {table} (
                tag_id INTEGER PRIMARY KEY,
                FOREIGN KEY(tag_id) REFERENCES tag(id) ON UPDATE CASCADE ON DELETE CASCADE
            )",
            table = EXCLUDED_TAGS_TABLE
        ),
        (),
    )?;

    Ok(())
}

/// Saves the list of excluded tag IDs for summary calculations.
///
/// This function replaces all currently excluded tags with the provided list.
///
/// # Arguments
/// * `tag_ids` - Vector of tag IDs to exclude from summaries
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database transaction fails
/// - SQL query preparation or execution fails
pub fn save_excluded_tags(tag_ids: &[TagId], connection: &Connection) -> Result<(), Error> {
    // Using unchecked_transaction because we only have &Connection from the MutexGuard.
    // This is safe because we hold the mutex lock and won't have nested transactions.
    let transaction = connection.unchecked_transaction()?;

    // Clear all existing excluded tags
    transaction.execute(
        &format!("DELETE FROM {table}", table = EXCLUDED_TAGS_TABLE),
        [],
    )?;

    // Insert new excluded tags
    for tag_id in tag_ids {
        transaction.execute(
            &format!(
                "INSERT INTO {table} (tag_id) VALUES (?1)",
                table = EXCLUDED_TAGS_TABLE
            ),
            [tag_id],
        )?;
    }

    transaction.commit()?;
    Ok(())
}

/// Gets the list of tag IDs that are currently excluded from summary calculations.
///
/// # Arguments
/// * `connection` - Database connection reference
///
/// # Returns
/// Vector of tag IDs that should be excluded from summaries.
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
pub fn get_excluded_tags(connection: &Connection) -> Result<Vec<TagId>, Error> {
    let mut stmt = connection.prepare(&format!(
        "SELECT tag_id FROM {table} ORDER BY tag_id",
        table = EXCLUDED_TAGS_TABLE
    ))?;

    let tag_ids = stmt
        .query_map([], |row| row.get::<_, TagId>(0))?
        .collect::<Result<Vec<TagId>, rusqlite::Error>>()?;

    Ok(tag_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db::initialize,
        tag::{TagName, create_tag},
    };
    use rusqlite::Connection;

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn save_and_get_excluded_tags_works() {
        let conn = get_test_connection();

        // Create some test tags
        let tag1 = create_tag(TagName::new("Tag1").unwrap(), &conn).unwrap();
        let _tag2 = create_tag(TagName::new("Tag2").unwrap(), &conn).unwrap();
        let tag3 = create_tag(TagName::new("Tag3").unwrap(), &conn).unwrap();

        // Save excluded tags
        let excluded_tags = vec![tag1.id, tag3.id];
        save_excluded_tags(&excluded_tags, &conn).unwrap();

        // Get excluded tags
        let result = get_excluded_tags(&conn).unwrap();

        assert_eq!(result, excluded_tags);
    }

    #[test]
    fn save_excluded_tags_replaces_existing() {
        let conn = get_test_connection();

        // Create some test tags
        let tag1 = create_tag(TagName::new("Tag1").unwrap(), &conn).unwrap();
        let tag2 = create_tag(TagName::new("Tag2").unwrap(), &conn).unwrap();
        let tag3 = create_tag(TagName::new("Tag3").unwrap(), &conn).unwrap();

        // Save initial excluded tags
        save_excluded_tags(&[tag1.id, tag2.id], &conn).unwrap();

        // Save different excluded tags (should replace)
        let new_excluded = vec![tag3.id];
        save_excluded_tags(&new_excluded, &conn).unwrap();

        // Get excluded tags
        let result = get_excluded_tags(&conn).unwrap();

        assert_eq!(result, new_excluded);
    }

    #[test]
    fn get_excluded_tags_returns_empty_when_none() {
        let conn = get_test_connection();

        let result = get_excluded_tags(&conn).unwrap();

        assert!(result.is_empty());
    }

    #[test]
    fn excluded_tags_are_removed_when_tag_is_deleted() {
        let conn = get_test_connection();

        // Create a test tag
        let tag = create_tag(TagName::new("TestTag").unwrap(), &conn).unwrap();

        // Add it to excluded tags
        save_excluded_tags(&[tag.id], &conn).unwrap();

        // Verify it's excluded
        let excluded = get_excluded_tags(&conn).unwrap();
        assert_eq!(excluded, vec![tag.id]);

        // Delete the tag
        conn.execute("DELETE FROM tag WHERE id = ?1", [tag.id])
            .unwrap();

        // Verify it's no longer in excluded tags (due to CASCADE)
        let excluded = get_excluded_tags(&conn).unwrap();
        assert!(excluded.is_empty());
    }
}
