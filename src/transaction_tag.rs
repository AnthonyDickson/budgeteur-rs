//! Transaction-Tag Junction Table Operations
//!
//! This module handles the many-to-many relationship between transactions and tags.
//! It provides functions for managing tag assignments to transactions, including
//! adding, removing, querying, and bulk operations on transaction-tag relationships.

use rusqlite::Connection;

use crate::{
    Error,
    database_id::DatabaseID,
    tag::{Tag, TagName},
};

/// Get the number of transactions associated with a tag.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn get_tag_transaction_count(
    tag_id: DatabaseID,
    connection: &Connection,
) -> Result<i64, Error> {
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM transaction_tag WHERE tag_id = ?1",
        [tag_id],
        |row| row.get(0),
    )?;

    Ok(count)
}

/// Create the transaction_tag junction table in the database.
///
/// # Errors
/// Returns an error if the table cannot be created or if there is an SQL error.
pub fn create_transaction_tag_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS transaction_tag (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            transaction_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON UPDATE CASCADE ON DELETE CASCADE,
            FOREIGN KEY(tag_id) REFERENCES tag(id) ON UPDATE CASCADE ON DELETE CASCADE,
            UNIQUE(transaction_id, tag_id)
        )",
        (),
    )?;

    // Create indexes for foreign keys to improve query performance
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_transaction_tag_transaction_id ON transaction_tag(transaction_id)",
        (),
    )?;

    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_transaction_tag_tag_id ON transaction_tag(tag_id)",
        (),
    )?;

    // Ensure the sequence starts at 1
    connection.execute(
        "INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('transaction_tag', 0)",
        (),
    )?;

    Ok(())
}

/// Add a tag to a transaction.
///
/// # Errors
/// This function will return a:
/// - [Error::InvalidTag] if `tag_id` does not refer to a valid tag,
/// - [Error::SqlError] if there is some other SQL error.
// TODO: Remove build config attribute once add_tag_to_transaction function is used elsewhere.
#[cfg(test)]
pub fn add_tag_to_transaction(
    transaction_id: DatabaseID,
    tag_id: DatabaseID,
    connection: &Connection,
) -> Result<(), Error> {
    connection
        .execute(
            "INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1, ?2)",
            (transaction_id, tag_id),
        )
        .map_err(|error| match error {
            // Code 787 occurs when a FOREIGN KEY constraint failed.
            rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                Error::InvalidTag
            }
            error => error.into(),
        })?;
    Ok(())
}

/// Remove a tag from a transaction.
///
/// # Errors
/// This function will return a [Error::SqlError] if there is a SQL error.
// TODO: Remove build config attribute once remove_tag_from_transaction function is used elsewhere.
#[cfg(test)]
pub fn remove_tag_from_transaction(
    transaction_id: DatabaseID,
    tag_id: DatabaseID,
    connection: &Connection,
) -> Result<(), Error> {
    connection.execute(
        "DELETE FROM transaction_tag WHERE transaction_id = ?1 AND tag_id = ?2",
        (transaction_id, tag_id),
    )?;
    Ok(())
}

/// Get all tags for a transaction.
///
/// # Errors
/// This function will return a [Error::SqlError] if there is a SQL error.
pub fn get_transaction_tags(
    transaction_id: DatabaseID,
    connection: &Connection,
) -> Result<Vec<Tag>, Error> {
    connection
        .prepare(
            "SELECT t.id, t.name 
             FROM tag t
             INNER JOIN transaction_tag tt ON t.id = tt.tag_id 
             WHERE tt.transaction_id = ?1
             ORDER BY t.name",
        )?
        .query_map([transaction_id], |row| {
            let id = row.get(0)?;
            let raw_name: String = row.get(1)?;
            let name = TagName::new_unchecked(&raw_name);
            Ok(Tag { id, name })
        })?
        .map(|maybe_tag| maybe_tag.map_err(Error::SqlError))
        .collect()
}

/// Set tags for a transaction, replacing any existing tags.
///
/// # Errors
/// This function will return a:
/// - [Error::InvalidTag] if any `tag_id` does not refer to a valid tag,
/// - [Error::SqlError] if there is some other SQL error.
pub fn set_transaction_tags(
    transaction_id: DatabaseID,
    tag_ids: &[DatabaseID],
    connection: &Connection,
) -> Result<(), Error> {
    let tx = connection.unchecked_transaction()?;

    // Remove existing tags
    tx.execute(
        "DELETE FROM transaction_tag WHERE transaction_id = ?1",
        [transaction_id],
    )?;

    // Add new tags
    let mut stmt =
        tx.prepare("INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1, ?2)")?;

    for &tag_id in tag_ids {
        stmt.execute((transaction_id, tag_id))
            .map_err(|error| match error {
                // Code 787 occurs when a FOREIGN KEY constraint failed.
                rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                    Error::InvalidTag
                }
                error => error.into(),
            })?;
    }

    drop(stmt);
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod transaction_tag_junction_tests {
    use rusqlite::Connection;
    use std::collections::HashSet;

    use crate::{
        Error,
        tag::{Tag, TagName, create_tag, create_tag_table, delete_tag, get_tag},
        transaction::{Transaction, create_transaction, create_transaction_table},
    };

    use super::{
        add_tag_to_transaction, create_transaction_tag_table, get_tag_transaction_count,
        get_transaction_tags, remove_tag_from_transaction, set_transaction_tags,
    };

    fn get_test_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();

        // Create all necessary tables
        create_tag_table(&connection).expect("Could not create tag table");
        create_transaction_table(&connection).expect("Could not create transaction table");
        create_transaction_tag_table(&connection).expect("Could not create junction table");

        connection
    }

    fn create_test_tag(name: &str, connection: &Connection) -> Tag {
        create_tag(TagName::new_unchecked(name), connection).expect("Could not create test tag")
    }

    fn create_test_transaction(
        amount: f64,
        description: &str,
        connection: &Connection,
    ) -> Transaction {
        create_transaction(
            Transaction::build(amount).description(description),
            connection,
        )
        .expect("Could not create test transaction")
    }

    // ============================================================================
    // BASIC CRUD TESTS
    // ============================================================================

    #[test]
    fn add_tag_to_transaction_succeeds() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        let result = add_tag_to_transaction(transaction.id(), tag.id, &connection);

        assert!(result.is_ok());

        // Verify the relationship was created
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], tag);
    }

    #[test]
    fn remove_tag_from_transaction_succeeds() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // First add the tag
        add_tag_to_transaction(transaction.id(), tag.id, &connection)
            .expect("Could not add tag to transaction");

        // Then remove it
        let result = remove_tag_from_transaction(transaction.id(), tag.id, &connection);

        assert!(result.is_ok());

        // Verify the relationship was removed
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn get_transaction_tags_returns_correct_tags() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let _tag2 = create_test_tag("Transport", &connection);
        let tag3 = create_test_tag("Entertainment", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // Add tags to transaction
        add_tag_to_transaction(transaction.id(), tag1.id, &connection).expect("Could not add tag1");
        add_tag_to_transaction(transaction.id(), tag3.id, &connection).expect("Could not add tag3");
        // Intentionally not adding tag2

        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        let tag_set: HashSet<_> = tags.into_iter().collect();

        let expected_set = HashSet::from([tag3, tag1]); // Note: should be sorted by name
        assert_eq!(tag_set, expected_set);
    }

    #[test]
    fn get_transaction_tags_returns_empty_for_no_tags() {
        let connection = get_test_connection();
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");

        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn set_transaction_tags_replaces_existing_tags() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let tag2 = create_test_tag("Transport", &connection);
        let tag3 = create_test_tag("Entertainment", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // First add some tags
        add_tag_to_transaction(transaction.id(), tag1.id, &connection).expect("Could not add tag1");
        add_tag_to_transaction(transaction.id(), tag2.id, &connection).expect("Could not add tag2");

        // Replace with different set of tags
        let new_tag_ids = vec![tag2.id, tag3.id];
        let result = set_transaction_tags(transaction.id(), &new_tag_ids, &connection);

        assert!(result.is_ok());

        // Verify the tags were replaced
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        let tag_set: HashSet<_> = tags.into_iter().collect();

        let expected_set = HashSet::from([tag2, tag3]);
        assert_eq!(tag_set, expected_set);
    }

    #[test]
    fn set_transaction_tags_with_empty_list_removes_all() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let tag2 = create_test_tag("Transport", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // First add some tags
        add_tag_to_transaction(transaction.id(), tag1.id, &connection).expect("Could not add tag1");
        add_tag_to_transaction(transaction.id(), tag2.id, &connection).expect("Could not add tag2");

        // Set to empty list
        let result = set_transaction_tags(transaction.id(), &[], &connection);

        assert!(result.is_ok());

        // Verify all tags were removed
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 0);
    }

    // ============================================================================
    // ERROR HANDLING TESTS
    // ============================================================================

    #[test]
    fn add_tag_to_transaction_fails_with_invalid_tag_id() {
        let connection = get_test_connection();
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);
        let invalid_tag_id = 999999; // Non-existent tag ID

        let result = add_tag_to_transaction(transaction.id(), invalid_tag_id, &connection);

        assert!(matches!(result, Err(Error::InvalidTag)));
    }

    #[test]
    fn set_transaction_tags_fails_with_invalid_tag_id() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);
        let invalid_tag_id = 999999; // Non-existent tag ID

        let invalid_tag_ids = vec![tag1.id, invalid_tag_id];
        let result = set_transaction_tags(transaction.id(), &invalid_tag_ids, &connection);

        assert!(matches!(result, Err(Error::InvalidTag)));

        // Verify transaction was rolled back - no tags should be added
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 0);
    }

    #[test]
    fn remove_tag_from_transaction_succeeds_with_non_existent_relationship() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // Try to remove a tag that was never added
        let result = remove_tag_from_transaction(transaction.id(), tag.id, &connection);

        // Should succeed (idempotent operation)
        assert!(result.is_ok());
    }

    #[test]
    fn functions_handle_non_existent_transaction_id() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let invalid_transaction_id = 999999; // Non-existent transaction ID

        // Adding a tag to non-existent transaction should fail due to foreign key constraint
        let add_result = add_tag_to_transaction(invalid_transaction_id, tag.id, &connection);
        assert!(add_result.is_err());

        // Removing from non-existent transaction should succeed (idempotent)
        let remove_result =
            remove_tag_from_transaction(invalid_transaction_id, tag.id, &connection);
        assert!(remove_result.is_ok());

        // Getting tags for non-existent transaction should succeed and return empty
        let get_result = get_transaction_tags(invalid_transaction_id, &connection);
        assert!(get_result.is_ok());
        assert_eq!(get_result.unwrap().len(), 0);
    }

    // ============================================================================
    // EDGE CASE AND DATA INTEGRITY TESTS
    // ============================================================================

    #[test]
    fn add_duplicate_tag_to_transaction_fails_due_to_unique_constraint() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);

        // Add tag once
        add_tag_to_transaction(transaction.id(), tag.id, &connection)
            .expect("Could not add tag first time");

        // Try to add the same tag again
        let result = add_tag_to_transaction(transaction.id(), tag.id, &connection);

        // Should fail due to unique constraint
        assert!(result.is_err());
    }

    #[test]
    fn multiple_transactions_can_have_same_tag() {
        let connection = get_test_connection();
        let tag = create_test_tag("Groceries", &connection);
        let transaction1 = create_test_transaction(50.0, "Store purchase", &connection);
        let transaction2 = create_test_transaction(30.0, "Market purchase", &connection);

        // Add same tag to both transactions
        add_tag_to_transaction(transaction1.id(), tag.id, &connection)
            .expect("Could not add tag to transaction1");
        add_tag_to_transaction(transaction2.id(), tag.id, &connection)
            .expect("Could not add tag to transaction2");

        // Verify both transactions have the tag
        let tags1 = get_transaction_tags(transaction1.id(), &connection)
            .expect("Could not get tags for transaction1");
        let tags2 = get_transaction_tags(transaction2.id(), &connection)
            .expect("Could not get tags for transaction2");

        assert_eq!(tags1.len(), 1);
        assert_eq!(tags2.len(), 1);
        assert_eq!(tags1[0], tag);
        assert_eq!(tags2[0], tag);
    }

    #[test]
    fn set_transaction_tags_is_atomic() {
        let connection = get_test_connection();
        let tag1 = create_test_tag("Groceries", &connection);
        let tag2 = create_test_tag("Transport", &connection);
        let transaction = create_test_transaction(50.0, "Store purchase", &connection);
        let invalid_tag_id = 999999;

        // First add a tag
        add_tag_to_transaction(transaction.id(), tag1.id, &connection)
            .expect("Could not add initial tag");

        // Try to set tags with one valid and one invalid ID
        let mixed_tag_ids = vec![tag2.id, invalid_tag_id];
        let result = set_transaction_tags(transaction.id(), &mixed_tag_ids, &connection);

        assert!(matches!(result, Err(Error::InvalidTag)));

        // Verify the original tag is still there (transaction was rolled back)
        let tags = get_transaction_tags(transaction.id(), &connection)
            .expect("Could not get transaction tags");
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], tag1);
    }

    #[test]
    fn delete_tag_with_transactions_succeeds_and_removes_relationships() {
        let connection = get_test_connection();

        // Create test data
        let tag_name = TagName::new_unchecked("TestTag");
        let tag = create_tag(tag_name, &connection).expect("Could not create test tag");

        let transaction = crate::transaction::create_transaction(
            crate::transaction::Transaction::build(100.0).description("Test transaction"),
            &connection,
        )
        .expect("Could not create test transaction");

        // Add tag to transaction
        add_tag_to_transaction(transaction.id(), tag.id, &connection)
            .expect("Could not add tag to transaction");

        // Verify relationship exists
        let count_before = get_tag_transaction_count(tag.id, &connection)
            .expect("Could not get transaction count");
        assert_eq!(count_before, 1);

        // Delete the tag
        let result = delete_tag(tag.id, &connection);
        assert!(result.is_ok());

        // Verify tag is deleted
        let get_result = get_tag(tag.id, &connection);
        assert_eq!(get_result, Err(Error::NotFound));

        // Verify relationship is also deleted (CASCADE DELETE)
        let count_after = get_tag_transaction_count(tag.id, &connection)
            .expect("Could not get transaction count");
        assert_eq!(count_after, 0);
    }
}
