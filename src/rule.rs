//! This file defines the `Rule` type for auto-tagging transactions based on description patterns.
//! A rule matches transaction descriptions that start with a specific pattern and applies a tag.

use std::sync::{Arc, Mutex};

use askama::Template;
use axum::{
    Form,
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use axum_htmx::HxRedirect;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, Error,
    database_id::DatabaseID,
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    not_found::NotFoundTemplate,
    shared_templates::render,
    tag::{Tag, TagName, get_all_tags},
};

/// A rule that automatically tags transactions whose descriptions start with a pattern.
/// Pattern matching is case-insensitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Rule {
    /// The ID of the rule.
    pub id: DatabaseID,

    /// The pattern that transaction descriptions must start with (case-insensitive).
    pub pattern: String,

    /// The ID of the tag to apply when this rule matches.
    pub tag_id: DatabaseID,
}

/// A rule with its associated tag information for display purposes.
#[derive(Debug, Clone)]
pub struct RuleWithTag {
    /// The rule itself.
    pub rule: Rule,
    /// The tag that will be applied by this rule.
    pub tag: Tag,
    /// URL for editing this rule.
    pub edit_url: String,
    /// URL for deleting this rule.
    pub delete_url: String,
}

/// Renders the rules listing page.
#[derive(Template)]
#[template(path = "views/rules.html")]
struct RulesTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    rules: Vec<RuleWithTag>,
    new_rule_route: &'a str,
}

/// Renders the new rule page.
#[derive(Template)]
#[template(path = "views/new_rule.html")]
struct NewRuleTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    form: NewRuleFormTemplate<'a>,
}

/// Renders the form for creating a rule.
#[derive(Template)]
#[template(path = "partials/new_rule_form.html")]
pub struct NewRuleFormTemplate<'a> {
    pub create_rule_endpoint: &'a str,
    pub available_tags: Vec<Tag>,
    pub error_message: &'a str,
}

/// Renders the edit rule page.
#[derive(Template)]
#[template(path = "views/edit_rule.html")]
struct EditRuleTemplate<'a> {
    nav_bar: NavbarTemplate<'a>,
    form: EditRuleFormTemplate<'a>,
}

/// Renders the form for editing a rule.
#[derive(Template)]
#[template(path = "partials/edit_rule_form.html")]
struct EditRuleFormTemplate<'a> {
    update_rule_endpoint: &'a str,
    available_tags: Vec<Tag>,
    rule_pattern: &'a str,
    selected_tag_id: DatabaseID,
    error_message: &'a str,
}

/// Renders an error message for rule operations.
#[derive(Template)]
#[template(path = "partials/rule_error.html")]
struct RuleErrorTemplate<'a> {
    error_message: &'a str,
}

/// Unified state for all rule-related operations.
#[derive(Debug, Clone)]
pub struct RuleState {
    pub db_connection: Arc<Mutex<Connection>>,
}

impl FromRef<AppState> for RuleState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            db_connection: state.db_connection.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RuleFormData {
    pub pattern: String,
    pub tag_id: DatabaseID,
}

pub async fn get_new_rule_page(State(state): State<RuleState>) -> Response {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let available_tags = match get_all_tags(&connection) {
        Ok(tags) => tags,
        Err(error) => {
            tracing::error!("Failed to retrieve tags for new rule page: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load tags").into_response();
        }
    };

    render(
        StatusCode::OK,
        NewRuleTemplate {
            nav_bar: get_nav_bar(endpoints::NEW_RULE_VIEW),
            form: NewRuleFormTemplate {
                create_rule_endpoint: endpoints::POST_RULE,
                available_tags,
                error_message: "",
            },
        },
    )
}

/// Route handler for the rules listing page.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn get_rules_page(State(state): State<RuleState>) -> Response {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let rules = match get_all_rules_with_tags(&connection) {
        Ok(rules) => rules,

        Err(error) => {
            tracing::error!("Failed to retrieve rules: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load rules").into_response();
        }
    };

    render(
        StatusCode::OK,
        RulesTemplate {
            nav_bar: get_nav_bar(endpoints::RULES_VIEW),
            rules,
            new_rule_route: endpoints::NEW_RULE_VIEW,
        },
    )
}

/// A route handler for creating a new rule.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn create_rule_endpoint(
    State(state): State<RuleState>,
    Form(new_rule): Form<RuleFormData>,
) -> Response {
    if new_rule.pattern.trim().is_empty() {
        let connection = state
            .db_connection
            .lock()
            .expect("Could not acquire database lock");

        let available_tags = get_all_tags(&connection).unwrap_or_default();

        return render(
            StatusCode::UNPROCESSABLE_ENTITY,
            NewRuleFormTemplate {
                create_rule_endpoint: endpoints::POST_RULE,
                available_tags,
                error_message: "Error: Pattern cannot be empty",
            },
        );
    }

    let rule_result = create_rule(
        new_rule.pattern.trim(),
        new_rule.tag_id,
        &state
            .db_connection
            .lock()
            .expect("Could not acquire database lock"),
    );

    match rule_result {
        Ok(_) => (
            HxRedirect(endpoints::RULES_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while creating a rule: {error}");

            let connection = state
                .db_connection
                .lock()
                .expect("Could not acquire database lock");

            let available_tags = get_all_tags(&connection).unwrap_or_default();

            render(
                StatusCode::INTERNAL_SERVER_ERROR,
                NewRuleFormTemplate {
                    create_rule_endpoint: endpoints::POST_RULE,
                    available_tags,
                    error_message: "An unexpected error occurred. Please try again.",
                },
            )
        }
    }
}

/// Route handler for the edit rule page.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
#[axum::debug_handler]
pub async fn get_edit_rule_page(
    Path(rule_id): Path<DatabaseID>,
    State(state): State<RuleState>,
) -> Response {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let available_tags = match get_all_tags(&connection) {
        Ok(tags) => tags,
        Err(error) => {
            tracing::error!("Failed to retrieve tags for edit rule page: {error}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load tags").into_response();
        }
    };

    let edit_endpoint = endpoints::format_endpoint(endpoints::EDIT_RULE_VIEW, rule_id);
    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_RULE, rule_id);

    match get_rule(rule_id, &connection) {
        Ok(rule) => render(
            StatusCode::OK,
            EditRuleTemplate {
                nav_bar: get_nav_bar(&edit_endpoint),
                form: EditRuleFormTemplate {
                    update_rule_endpoint: &update_endpoint,
                    available_tags,
                    rule_pattern: &rule.pattern,
                    selected_tag_id: rule.tag_id,
                    error_message: "",
                },
            },
        ),
        Err(Error::NotFound) => render(StatusCode::NOT_FOUND, NotFoundTemplate),
        Err(error) => {
            tracing::error!("An unexpected error ocurred when fetching rule #{rule_id}: {error}");
            Redirect::to(endpoints::INTERNAL_ERROR_VIEW).into_response()
        }
    }
}

/// A route handler for updating a rule.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn update_rule_endpoint(
    Path(rule_id): Path<DatabaseID>,
    State(state): State<RuleState>,
    Form(form_data): Form<RuleFormData>,
) -> impl IntoResponse {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_RULE, rule_id);

    if form_data.pattern.trim().is_empty() {
        let available_tags = get_all_tags(&connection).unwrap_or_default();

        return render(
            StatusCode::UNPROCESSABLE_ENTITY,
            EditRuleFormTemplate {
                update_rule_endpoint: &update_endpoint,
                available_tags,
                rule_pattern: &form_data.pattern,
                selected_tag_id: form_data.tag_id,
                error_message: "Error: Pattern cannot be empty",
            },
        );
    }

    if let Err(error) = update_rule(
        rule_id,
        form_data.pattern.trim(),
        form_data.tag_id,
        &connection,
    ) {
        let (status, error_message) = if error == Error::NotFound {
            (StatusCode::NOT_FOUND, "Rule not found")
        } else {
            tracing::error!("An unexpected error occurred while updating rule {rule_id}: {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "An unexpected error occurred. Please try again.",
            )
        };

        let available_tags = get_all_tags(&connection).unwrap_or_default();

        render(
            status,
            EditRuleFormTemplate {
                update_rule_endpoint: &update_endpoint,
                available_tags,
                rule_pattern: &form_data.pattern,
                selected_tag_id: form_data.tag_id,
                error_message,
            },
        )
    } else {
        (
            HxRedirect(endpoints::RULES_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response()
    }
}

/// A route handler for deleting a rule.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn delete_rule_endpoint(
    Path(rule_id): Path<DatabaseID>,
    State(state): State<RuleState>,
) -> impl IntoResponse {
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    if let Err(error) = delete_rule(rule_id, &connection) {
        let error_message = if error == Error::NotFound {
            "Rule not found"
        } else {
            tracing::error!("An unexpected error occurred while deleting rule {rule_id}: {error}");
            "An unexpected error occurred. Please try again."
        };

        render(StatusCode::OK, RuleErrorTemplate { error_message })
    } else {
        (
            HxRedirect(endpoints::RULES_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response()
    }
}

/// Create a rule in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn create_rule(
    pattern: &str,
    tag_id: DatabaseID,
    connection: &Connection,
) -> Result<Rule, Error> {
    connection.execute(
        "INSERT INTO rule (pattern, tag_id) VALUES (?1, ?2);",
        (pattern, tag_id),
    )?;

    let id = connection.last_insert_rowid();

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
pub fn get_rule(rule_id: DatabaseID, connection: &Connection) -> Result<Rule, Error> {
    connection
        .prepare("SELECT id, pattern, tag_id FROM rule WHERE id = :id;")?
        .query_row(&[(":id", &rule_id)], map_rule_row)
        .map_err(|error| error.into())
}

/// Update a rule in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error or if the rule doesn't exist.
pub fn update_rule(
    rule_id: DatabaseID,
    new_pattern: &str,
    new_tag_id: DatabaseID,
    connection: &Connection,
) -> Result<(), Error> {
    let rows_affected = connection.execute(
        "UPDATE rule SET pattern = ?1, tag_id = ?2 WHERE id = ?3",
        (new_pattern, new_tag_id, rule_id),
    )?;

    if rows_affected == 0 {
        return Err(Error::NotFound);
    }

    Ok(())
}

/// Delete a rule from the database.
///
/// # Errors
/// This function will return an error if there is an SQL error or if the rule doesn't exist.
pub fn delete_rule(rule_id: DatabaseID, connection: &Connection) -> Result<(), Error> {
    let rows_affected = connection.execute("DELETE FROM rule WHERE id = ?1", [rule_id])?;

    if rows_affected == 0 {
        return Err(Error::NotFound);
    }

    Ok(())
}

/// Retrieve all rules in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
// TODO: Remove test config attribute once used elsewhere
#[cfg(test)]
pub fn get_all_rules(connection: &Connection) -> Result<Vec<Rule>, Error> {
    connection
        .prepare("SELECT id, pattern, tag_id FROM rule;")?
        .query_map([], map_rule_row)?
        .map(|maybe_rule| maybe_rule.map_err(|error| error.into()))
        .collect()
}

/// Retrieve all rules with their associated tag information.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn get_all_rules_with_tags(connection: &Connection) -> Result<Vec<RuleWithTag>, Error> {
    connection
        .prepare(
            "SELECT r.id, r.pattern, r.tag_id, t.id, t.name 
             FROM rule r 
             INNER JOIN tag t ON r.tag_id = t.id
             ORDER BY r.pattern",
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

/// Check if a transaction description matches a rule pattern (case-insensitive).
///
/// # Arguments
/// * `description` - The transaction description to check
/// * `pattern` - The rule pattern to match against
///
/// # Returns
/// `true` if the description starts with the pattern (case-insensitive), `false` otherwise
// TODO: Remove test config attribute once used elsewhere
#[cfg(test)]
#[inline]
pub fn matches_rule_pattern(description: &str, pattern: &str) -> bool {
    description
        .to_lowercase()
        .starts_with(&pattern.to_lowercase())
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
mod rule_tests {
    use std::collections::HashSet;

    use rusqlite::Connection;

    use crate::{
        Error,
        rule::{create_rule, get_all_rules, get_rule, matches_rule_pattern, update_rule},
        tag::{TagName, create_tag, create_tag_table},
    };

    use super::{create_rule_table, delete_rule};

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

        assert_eq!(result, Err(Error::NotFound));
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

        assert_eq!(result, Err(Error::NotFound));
    }

    #[test]
    fn matches_rule_pattern_case_insensitive() {
        // Test exact match
        assert!(matches_rule_pattern("foo bar", "foo"));
        assert!(matches_rule_pattern("FOO BAR", "foo"));
        assert!(matches_rule_pattern("foo bar", "FOO"));
        assert!(matches_rule_pattern("FOO BAR", "FOO"));

        // Test prefix match
        assert!(matches_rule_pattern("foo bar baz", "foo"));
        assert!(matches_rule_pattern("FOO BAR BAZ", "foo"));
        assert!(matches_rule_pattern("foo bar baz", "FOO"));

        // Test no match - pattern not at start
        assert!(!matches_rule_pattern("baz foo bar", "foo"));
        assert!(!matches_rule_pattern("BAZ FOO BAR", "foo"));

        // Test no match - different text
        assert!(!matches_rule_pattern("buzz", "foo"));
        assert!(!matches_rule_pattern("BUZZ", "foo"));

        // Test empty cases
        assert!(matches_rule_pattern("anything", ""));
        assert!(!matches_rule_pattern("", "foo"));
    }
}
