use rusqlite::{Connection, Row};

use crate::{
    Error, endpoints,
    rule::models::{Rule, RuleId, RuleWithTag},
    tag::{Tag, TagId, TagName},
};

/// Create a rule in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn create_rule(pattern: &str, tag_id: TagId, connection: &Connection) -> Result<Rule, Error> {
    connection.execute(
        "INSERT INTO rule (pattern, tag_id) VALUES (?1, ?2);",
        (pattern, tag_id),
    )?;

    let id = connection.last_insert_rowid() as u32;

    Ok(Rule {
        id,
        pattern: pattern.to_string(),
        tag_id,
    })
}

/// Retrieve a rule in the database by `rule_id`.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub(super) fn get_rule(rule_id: RuleId, connection: &Connection) -> Result<Rule, Error> {
    connection
        .prepare("SELECT id, pattern, tag_id FROM rule WHERE id = :id;")?
        .query_row(&[(":id", &rule_id)], map_rule_row)
        .map_err(|error| error.into())
}

/// Retrieve all rules in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub(super) fn get_all_rules(connection: &Connection) -> Result<Vec<Rule>, Error> {
    connection
        // Sort by descending length to ensure that ambiguous patterns (e.g, foo, foobar)
        // always match the more specific (longer) pattern first
        .prepare("SELECT id, pattern, tag_id FROM rule ORDER BY LENGTH(pattern) DESC;")?
        .query_map([], map_rule_row)?
        .map(|maybe_rule| maybe_rule.map_err(|error| error.into()))
        .collect()
}

/// Retrieve all rules with their associated tag information.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub(super) fn get_all_rules_with_tags(connection: &Connection) -> Result<Vec<RuleWithTag>, Error> {
    connection
        .prepare(
            "SELECT r.id, r.pattern, r.tag_id, t.id, t.name 
             FROM rule r 
             INNER JOIN tag t ON r.tag_id = t.id
             ORDER BY t.name ASC, r.pattern ASC",
        )?
        .query_map([], |row| {
            let rule_id = row.get(0)?;
            let pattern = row.get(1)?;
            let tag_id = row.get(2)?;
            let tag_name_str: String = row.get(4)?;

            let rule = Rule {
                id: rule_id,
                pattern,
                tag_id,
            };

            let tag = Tag {
                id: tag_id,
                name: TagName::new_unchecked(&tag_name_str),
            };

            Ok(RuleWithTag {
                edit_url: endpoints::format_endpoint(endpoints::EDIT_RULE_VIEW, rule_id),
                delete_url: endpoints::format_endpoint(endpoints::DELETE_RULE, rule_id),
                rule,
                tag,
            })
        })?
        .map(|maybe_rule_with_tag| maybe_rule_with_tag.map_err(|error| error.into()))
        .collect()
}

/// Update a rule in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error or if the rule doesn't exist.
pub(super) fn update_rule(
    rule_id: RuleId,
    new_pattern: &str,
    new_tag_id: TagId,
    connection: &Connection,
) -> Result<(), Error> {
    let rows_affected = connection.execute(
        "UPDATE rule SET pattern = ?1, tag_id = ?2 WHERE id = ?3",
        (new_pattern, new_tag_id, rule_id),
    )?;

    if rows_affected == 0 {
        return Err(Error::UpdateMissingRule);
    }

    Ok(())
}

/// Delete a rule from the database.
///
/// # Errors
/// This function will return an error if there is an SQL error or if the rule doesn't exist.
pub(super) fn delete_rule(rule_id: RuleId, connection: &Connection) -> Result<(), Error> {
    let rows_affected = connection.execute("DELETE FROM rule WHERE id = ?1", [rule_id])?;

    if rows_affected == 0 {
        return Err(Error::DeleteMissingRule);
    }

    Ok(())
}

pub fn create_rule_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS rule (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                pattern TEXT NOT NULL,
                tag_id INTEGER NOT NULL,
                FOREIGN KEY(tag_id) REFERENCES tag(id) ON UPDATE CASCADE ON DELETE CASCADE,
                UNIQUE(pattern, tag_id)
            );",
        (),
    )?;

    // Create index for foreign key to improve query performance
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_rule_tag_id ON rule(tag_id)",
        (),
    )?;

    // Improve performance when sorting by pattern
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_rule_pattern ON rule(pattern)",
        (),
    )?;

    // Ensure the sequence starts at 1
    connection.execute(
        "INSERT OR IGNORE INTO sqlite_sequence (name, seq) VALUES ('rule', 0)",
        (),
    )?;

    Ok(())
}

fn map_rule_row(row: &Row) -> Result<Rule, rusqlite::Error> {
    let id = row.get(0)?;
    let pattern = row.get(1)?;
    let tag_id = row.get(2)?;

    Ok(Rule {
        id,
        pattern,
        tag_id,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rusqlite::Connection;

    use crate::{
        Error,
        rule::{create_rule, create_rule_table},
        tag::{TagName, create_tag, create_tag_table},
    };

    use super::{delete_rule, get_all_rules, get_rule, update_rule};

    fn get_test_db_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        create_tag_table(&connection).expect("Could not create tag table");
        create_rule_table(&connection).expect("Could not create rule table");
        connection
    }

    #[test]
    fn create_rule_succeeds() {
        let connection = get_test_db_connection();
        let tag = create_tag(TagName::new_unchecked("Groceries"), &connection).unwrap();
        let pattern = "store";

        let rule = create_rule(pattern, tag.id, &connection);

        let got_rule = rule.expect("Could not create rule");
        assert!(got_rule.id > 0);
        assert_eq!(got_rule.pattern, pattern);
        assert_eq!(got_rule.tag_id, tag.id);
    }

    #[test]
    fn get_rule_succeeds() {
        let connection = get_test_db_connection();
        let tag = create_tag(TagName::new_unchecked("Transport"), &connection).unwrap();
        let inserted_rule =
            create_rule("bus", tag.id, &connection).expect("Could not create test rule");

        let selected_rule = get_rule(inserted_rule.id, &connection);

        assert_eq!(Ok(inserted_rule), selected_rule);
    }

    #[test]
    fn get_rule_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let tag = create_tag(TagName::new_unchecked("Food"), &connection).unwrap();
        let inserted_rule =
            create_rule("restaurant", tag.id, &connection).expect("Could not create test rule");

        let selected_rule = get_rule(inserted_rule.id + 123, &connection);

        assert_eq!(selected_rule, Err(Error::NotFound));
    }

    #[test]
    fn test_get_all_rules() {
        let connection = get_test_db_connection();
        let tag1 = create_tag(TagName::new_unchecked("Groceries"), &connection).unwrap();
        let tag2 = create_tag(TagName::new_unchecked("Transport"), &connection).unwrap();

        let inserted_rules = HashSet::from([
            create_rule("supermarket", tag1.id, &connection).expect("Could not create test rule"),
            create_rule("bus", tag2.id, &connection).expect("Could not create test rule"),
        ]);

        let selected_rules = get_all_rules(&connection).expect("Could not get all rules");
        let selected_rules = HashSet::from_iter(selected_rules);

        assert_eq!(inserted_rules, selected_rules);
    }

    #[test]
    fn update_rule_succeeds() {
        let connection = get_test_db_connection();
        let tag1 = create_tag(TagName::new_unchecked("Original"), &connection).unwrap();
        let tag2 = create_tag(TagName::new_unchecked("Updated"), &connection).unwrap();
        let rule =
            create_rule("old pattern", tag1.id, &connection).expect("Could not create test rule");

        let new_pattern = "new pattern";
        let result = update_rule(rule.id, new_pattern, tag2.id, &connection);

        assert!(result.is_ok());

        let updated_rule = get_rule(rule.id, &connection).expect("Could not get updated rule");
        assert_eq!(updated_rule.pattern, new_pattern);
        assert_eq!(updated_rule.tag_id, tag2.id);
        assert_eq!(updated_rule.id, rule.id);
    }

    #[test]
    fn update_rule_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let tag = create_tag(TagName::new_unchecked("Test"), &connection).unwrap();
        let invalid_id = 999999;
        let new_pattern = "updated";

        let result = update_rule(invalid_id, new_pattern, tag.id, &connection);

        assert_eq!(result, Err(Error::UpdateMissingRule));
    }

    #[test]
    fn delete_rule_succeeds() {
        let connection = get_test_db_connection();
        let tag = create_tag(TagName::new_unchecked("ToDelete"), &connection).unwrap();
        let rule =
            create_rule("delete me", tag.id, &connection).expect("Could not create test rule");

        let result = delete_rule(rule.id, &connection);

        assert!(result.is_ok());

        let get_result = get_rule(rule.id, &connection);
        assert_eq!(get_result, Err(Error::NotFound));
    }

    #[test]
    fn delete_rule_with_invalid_id_returns_not_found() {
        let connection = get_test_db_connection();
        let invalid_id = 999999;

        let result = delete_rule(invalid_id, &connection);

        assert_eq!(result, Err(Error::DeleteMissingRule));
    }
}
