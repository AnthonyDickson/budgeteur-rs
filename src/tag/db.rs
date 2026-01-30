//! Database operations for tags.

use rusqlite::{Connection, Row};

use crate::{
    Error,
    tag::{Tag, TagId, TagName},
};

/// Create a tag and return it with its generated ID.
pub fn create_tag(name: TagName, connection: &Connection) -> Result<Tag, Error> {
    connection.execute("INSERT INTO tag (name) VALUES (?1);", (name.as_ref(),))?;

    let id = connection.last_insert_rowid();

    Ok(Tag { id, name })
}

/// Retrieve a single tag by ID.
pub fn get_tag(tag_id: TagId, connection: &Connection) -> Result<Tag, Error> {
    connection
        .prepare("SELECT id, name FROM tag WHERE id = :id;")?
        .query_row(&[(":id", &tag_id)], map_row)
        .map_err(|error| error.into())
}

/// Retrieve all tags ordered alphabetically by name.
pub fn get_all_tags(connection: &Connection) -> Result<Vec<Tag>, Error> {
    connection
        .prepare("SELECT id, name FROM tag ORDER BY name ASC;")?
        .query_map([], map_row)?
        .map(|maybe_tag| maybe_tag.map_err(|error| error.into()))
        .collect()
}

/// Update a tag's name. Returns an error if tag doesn't exist.
pub fn update_tag(tag_id: TagId, new_name: TagName, connection: &Connection) -> Result<(), Error> {
    let rows_affected = connection.execute(
        "UPDATE tag SET name = ?1 WHERE id = ?2",
        (new_name.as_ref(), tag_id),
    )?;

    if rows_affected == 0 {
        return Err(Error::UpdateMissingTag);
    }

    Ok(())
}

/// Delete a tag by ID. Returns an error if the tag doesn't exist.
pub fn delete_tag(tag_id: TagId, connection: &Connection) -> Result<(), Error> {
    let rows_affected = connection.execute("DELETE FROM tag WHERE id = ?1", [tag_id])?;

    if rows_affected == 0 {
        return Err(Error::DeleteMissingTag);
    }

    Ok(())
}

/// Initialize the tag table and indexes.
pub fn create_tag_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS tag (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );

        CREATE INDEX IF NOT EXISTS idx_tag_name ON tag(name);",
    )?;

    Ok(())
}

fn map_row(row: &Row) -> Result<Tag, rusqlite::Error> {
    let id = row.get(0)?;
    let raw_name: String = row.get(1)?;
    let name = TagName::new_unchecked(&raw_name);

    Ok(Tag { id, name })
}

#[cfg(test)]
mod tag_name_tests {
    use crate::{Error, tag::TagName};

    #[test]
    fn new_fails_on_empty_string() {
        let tag_name = TagName::new("");

        assert_eq!(tag_name, Err(Error::EmptyTagName));
    }

    #[test]
    fn new_fails_on_just_whitespace() {
        let tag_name = TagName::new("\n\t \r");

        assert_eq!(tag_name, Err(Error::EmptyTagName));
    }

    #[test]
    fn new_succeeds_on_non_empty_string() {
        let tag_name = TagName::new("🔥");

        assert!(tag_name.is_ok())
    }
}

#[cfg(test)]
mod tag_query_tests {
    use std::collections::HashSet;

    use rusqlite::Connection;

    use crate::{
        Error,
        tag::{TagName, create_tag, get_all_tags, get_tag, update_tag},
    };

    use super::{create_tag_table, delete_tag};

    fn get_test_db_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        create_tag_table(&connection).expect("Could not create tag table");
        connection
    }

    #[test]
    fn create_tag_succeeds() {
        let connection = get_test_db_connection();
        let name = TagName::new("Terrifically a tag").unwrap();

        let tag = create_tag(name.clone(), &connection);

        let got_tag = tag.expect("Could not create tag");
        assert!(got_tag.id > 0);
        assert_eq!(got_tag.name, name);
    }

    #[test]
    fn get_tag_succeeds() {
        let connection = get_test_db_connection();
        let name = TagName::new_unchecked("Foo");
        let inserted_tag = create_tag(name, &connection).expect("Could not create test tag");

        let selected_tag = get_tag(inserted_tag.id, &connection);

        assert_eq!(Ok(inserted_tag), selected_tag);
    }

    #[test]
    fn get_tag_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let inserted_tag = create_tag(TagName::new_unchecked("Foo"), &connection)
            .expect("Could not create test tag");

        let selected_tag = get_tag(inserted_tag.id + 123, &connection);

        assert_eq!(selected_tag, Err(Error::NotFound));
    }

    #[test]
    fn test_get_all_tag() {
        let store = get_test_db_connection();

        let inserted_tags = HashSet::from([
            create_tag(TagName::new_unchecked("Foo"), &store).expect("Could not create test tag"),
            create_tag(TagName::new_unchecked("Bar"), &store).expect("Could not create test tag"),
        ]);

        let selected_tags = get_all_tags(&store).expect("Could not get all tags");
        let selected_tags = HashSet::from_iter(selected_tags);

        assert_eq!(inserted_tags, selected_tags);
    }

    #[test]
    fn update_tag_succeeds() {
        let connection = get_test_db_connection();
        let original_name = TagName::new_unchecked("Original");
        let tag = create_tag(original_name, &connection).expect("Could not create test tag");

        let new_name = TagName::new_unchecked("Updated");
        let result = update_tag(tag.id, new_name.clone(), &connection);

        assert!(result.is_ok());

        let updated_tag = get_tag(tag.id, &connection).expect("Could not get updated tag");
        assert_eq!(updated_tag.name, new_name);
        assert_eq!(updated_tag.id, tag.id);
    }

    #[test]
    fn update_tag_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let invalid_id = 999999;
        let new_name = TagName::new_unchecked("Updated");

        let result = update_tag(invalid_id, new_name, &connection);

        assert_eq!(result, Err(Error::UpdateMissingTag));
    }

    #[test]
    fn delete_tag_succeeds() {
        let connection = get_test_db_connection();
        let name = TagName::new_unchecked("ToDelete");
        let tag = create_tag(name, &connection).expect("Could not create test tag");

        let result = delete_tag(tag.id, &connection);

        assert!(result.is_ok());

        let get_result = get_tag(tag.id, &connection);
        assert_eq!(get_result, Err(Error::NotFound));
    }

    #[test]
    fn delete_tag_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let invalid_id = 999999;

        let result = delete_tag(invalid_id, &connection);

        assert_eq!(result, Err(Error::DeleteMissingTag));
    }
}
