//! This file defines the `Rule` type for auto-tagging transactions based on description patterns.
//! A rule matches transaction descriptions that start with a specific pattern and applies a tag.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use askama::Template;
use axum::{
    Form,
    extract::{FromRef, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use axum_htmx::HxRedirect;
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, Error,
    alert::AlertTemplate,
    database_id::{DatabaseId, TransactionId},
    endpoints,
    navigation::{NavbarTemplate, get_nav_bar},
    shared_templates::render,
    tag::{Tag, TagId, TagName, get_all_tags},
    transaction::Transaction,
};

/// A rule that automatically tags transactions whose descriptions start with a pattern.
/// Pattern matching is case-insensitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Rule {
    /// The ID of the rule.
    pub id: DatabaseId,

    /// The pattern that transaction descriptions must start with (case-insensitive).
    pub pattern: String,

    /// The ID of the tag to apply when this rule matches.
    pub tag_id: DatabaseId,
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
    auto_tag_all_route: &'a str,
    auto_tag_untagged_route: &'a str,
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
    selected_tag_id: DatabaseId,
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

/// Form data for creating and editing rules.
#[derive(Debug, Serialize, Deserialize)]
pub struct RuleFormData {
    /// The pattern that transaction descriptions must start with (case-insensitive).
    pub pattern: String,
    /// The ID of the tag to apply when this rule matches.
    pub tag_id: DatabaseId,
}

/// Route handler for the new rule page.
pub async fn get_new_rule_page(State(state): State<RuleState>) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let available_tags = get_all_tags(&connection).inspect_err(|error| {
        tracing::error!("Failed to retrieve tags for new rule page: {error}")
    })?;

    Ok(render(
        StatusCode::OK,
        NewRuleTemplate {
            nav_bar: get_nav_bar(endpoints::NEW_RULE_VIEW),
            form: NewRuleFormTemplate {
                create_rule_endpoint: endpoints::POST_RULE,
                available_tags,
                error_message: "",
            },
        },
    ))
}

/// Route handler for the rules listing page.
pub async fn get_rules_page(State(state): State<RuleState>) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let rules = get_all_rules_with_tags(&connection)
        .inspect_err(|error| tracing::error!("Failed to retrieve rules: {error}"))?;

    Ok(render(
        StatusCode::OK,
        RulesTemplate {
            nav_bar: get_nav_bar(endpoints::RULES_VIEW),
            rules,
            new_rule_route: endpoints::NEW_RULE_VIEW,
            auto_tag_all_route: endpoints::AUTO_TAG_ALL,
            auto_tag_untagged_route: endpoints::AUTO_TAG_UNTAGGED,
        },
    ))
}

/// A route handler for creating a new rule.
pub async fn create_rule_endpoint(
    State(state): State<RuleState>,
    Form(new_rule): Form<RuleFormData>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    let available_tags = get_all_tags(&connection).unwrap_or_default();

    if new_rule.pattern.trim().is_empty() {
        return render(
            StatusCode::UNPROCESSABLE_ENTITY,
            NewRuleFormTemplate {
                create_rule_endpoint: endpoints::POST_RULE,
                available_tags,
                error_message: "Error: Pattern cannot be empty",
            },
        );
    }

    let rule_result = create_rule(new_rule.pattern.trim(), new_rule.tag_id, &connection);

    match rule_result {
        Ok(_) => (
            HxRedirect(endpoints::RULES_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while creating a rule: {error}");

            error.into_alert_response()
        }
    }
}

/// Route handler for the edit rule page.
pub async fn get_edit_rule_page(
    Path(rule_id): Path<DatabaseId>,
    State(state): State<RuleState>,
) -> Result<Response, Error> {
    let connection = state
        .db_connection
        .lock()
        .inspect_err(|error| tracing::error!("could not acquire database lock: {error}"))
        .map_err(|_| Error::DatabaseLockError)?;

    let available_tags = get_all_tags(&connection).inspect_err(|error| {
        tracing::error!("Failed to retrieve tags for edit rule page: {error}")
    })?;

    let edit_endpoint = endpoints::format_endpoint(endpoints::EDIT_RULE_VIEW, rule_id);
    let update_endpoint = endpoints::format_endpoint(endpoints::PUT_RULE, rule_id);

    let rule = get_rule(rule_id, &connection).inspect_err(|error| match error {
        Error::NotFound => {}
        error => {
            tracing::error!("An unexpected error ocurred when fetching rule #{rule_id}: {error}");
        }
    })?;

    Ok(render(
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
    ))
}

/// A route handler for updating a rule.
pub async fn update_rule_endpoint(
    Path(rule_id): Path<DatabaseId>,
    State(state): State<RuleState>,
    Form(form_data): Form<RuleFormData>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

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

    let result = update_rule(
        rule_id,
        form_data.pattern.trim(),
        form_data.tag_id,
        &connection,
    );

    match result {
        Ok(_) => (
            HxRedirect(endpoints::RULES_VIEW.to_owned()),
            StatusCode::SEE_OTHER,
        )
            .into_response(),
        Err(Error::UpdateMissingRule) => Error::UpdateMissingRule.into_alert_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while updating rule {rule_id}: {error}");
            error.into_alert_response()
        }
    }
}

/// A route handler for deleting a rule.
pub async fn delete_rule_endpoint(
    Path(rule_id): Path<DatabaseId>,
    State(state): State<RuleState>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match delete_rule(rule_id, &connection) {
        Ok(_) => render(
            StatusCode::OK,
            AlertTemplate::success("Rule deleted successfully", ""),
        ),
        Err(Error::DeleteMissingRule) => Error::DeleteMissingRule.into_alert_response(),
        Err(error) => {
            tracing::error!("An unexpected error occurred while deleting rule {rule_id}: {error}");
            error.into_alert_response()
        }
    }
}

/// A route handler for applying auto-tagging rules to all transactions.
pub async fn auto_tag_all_transactions_endpoint(State(state): State<RuleState>) -> Response {
    let start_time = std::time::Instant::now();

    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match apply_rules_to_transactions(TaggingMode::FetchAll, &connection) {
        Ok(result) => {
            let duration = start_time.elapsed();
            tracing::info!(
                "Auto-tagging all transactions completed in {:.2}ms: {} transactions processed, {} tags applied",
                duration.as_millis(),
                result.transactions_tagged,
                result.tags_applied
            );

            let message = if result.transactions_tagged > 0 {
                "Auto-tagging completed successfully!"
            } else {
                "Auto-tagging completed - no transactions were processed."
            };

            let details = format!(
                "Tagged {} transactions with {} tags in {:.1}ms",
                result.transactions_tagged,
                result.tags_applied,
                duration.as_millis()
            );

            render(StatusCode::OK, AlertTemplate::success(message, &details))
        }
        Err(error) => {
            let duration = start_time.elapsed();
            tracing::error!(
                "Failed to apply auto-tagging to all transactions after {:.2}ms: {error}",
                duration.as_millis()
            );

            let details = format!(
                "Failed after {:.1}ms. Please try again.",
                duration.as_millis()
            );

            render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error("Auto-tagging failed", &details),
            )
        }
    }
}

/// A route handler for applying auto-tagging rules to untagged transactions only.
pub async fn auto_tag_untagged_transactions_endpoint(State(state): State<RuleState>) -> Response {
    let start_time = std::time::Instant::now();

    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    match apply_rules_to_transactions(TaggingMode::FetchUntagged, &connection) {
        Ok(result) => {
            let duration = start_time.elapsed();
            tracing::info!(
                "Auto-tagging untagged transactions completed in {:.2}ms: {} transactions processed, {} tags applied",
                duration.as_millis(),
                result.transactions_tagged,
                result.tags_applied
            );

            let message = if result.transactions_tagged > 0 {
                "Auto-tagging untagged transactions completed successfully!"
            } else {
                "Auto-tagging completed - no untagged transactions were processed."
            };

            let details = format!(
                "Tagged {} untagged transactions with {} tags in {:.1}ms",
                result.transactions_tagged,
                result.tags_applied,
                duration.as_millis()
            );

            render(StatusCode::OK, AlertTemplate::success(message, &details))
        }
        Err(error) => {
            let duration = start_time.elapsed();
            tracing::error!(
                "Failed to apply auto-tagging to untagged transactions after {:.2}ms: {error}",
                duration.as_millis()
            );

            let details = format!(
                "Failed after {:.1}ms. Please try again.",
                duration.as_millis()
            );

            render(
                StatusCode::INTERNAL_SERVER_ERROR,
                AlertTemplate::error("Auto-tagging failed", &details),
            )
        }
    }
}

/// Create a rule in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn create_rule(
    pattern: &str,
    tag_id: DatabaseId,
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
pub fn get_rule(rule_id: DatabaseId, connection: &Connection) -> Result<Rule, Error> {
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
    rule_id: DatabaseId,
    new_pattern: &str,
    new_tag_id: DatabaseId,
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
pub fn delete_rule(rule_id: DatabaseId, connection: &Connection) -> Result<(), Error> {
    let rows_affected = connection.execute("DELETE FROM rule WHERE id = ?1", [rule_id])?;

    if rows_affected == 0 {
        return Err(Error::DeleteMissingRule);
    }

    Ok(())
}

/// Retrieve all rules in the database.
///
/// # Errors
/// This function will return an error if there is an SQL error.
pub fn get_all_rules(connection: &Connection) -> Result<Vec<Rule>, Error> {
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
pub fn get_all_rules_with_tags(connection: &Connection) -> Result<Vec<RuleWithTag>, Error> {
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

/// Check if a transaction description matches a rule pattern (case-insensitive).
///
/// # Arguments
/// * `description` - The transaction description to check
/// * `pattern` - The rule pattern to match against
///
/// # Returns
/// `true` if the description starts with the pattern (case-insensitive), `false` otherwise
#[inline]
pub fn matches_rule_pattern(description: &str, pattern: &str) -> bool {
    description
        .to_lowercase()
        .starts_with(&pattern.to_lowercase())
}

/// Describes which transactions should be used for a batch tagging operation.
pub enum TaggingMode<'a> {
    FetchAll,
    FetchUntagged,
    FromArgs(&'a [Transaction]),
}

/// Get transaction IDs and descriptions for auto-tagging, optionally filtering to untagged only.
///
/// # Arguments
/// * `mode` - Specify how to get transactions.
/// * `connection` - Database connection
///
/// # Returns
/// A vector of tuples containing (transaction_id, description) pairs
///
/// # Errors
/// Returns an error if there are database errors during the operation
fn get_transactions_for_auto_tagging(
    mode: TaggingMode,
    connection: &Connection,
) -> Result<Vec<(TransactionId, Option<TagId>, String)>, Error> {
    let query = match mode {
        TaggingMode::FetchAll => "SELECT id, tag_id, description FROM \"transaction\"",
        TaggingMode::FetchUntagged => {
            "SELECT id, tag_id, description FROM \"transaction\" WHERE tag_id IS NULL"
        }
        TaggingMode::FromArgs(transactions) => {
            return Ok(transactions
                .iter()
                .map(|transaction| {
                    (
                        transaction.id,
                        transaction.tag_id,
                        transaction.description.clone(),
                    )
                })
                .collect());
        }
    };

    connection
        .prepare(query)?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)
}

/// Batch set tags for multiple transactions, replacing any existing tags.
///
/// # Arguments
/// * `transaction_tag_map` - Vec of transaction_id and tag_id pairs
/// * `connection` - Database connection
///
/// # Errors
/// This function will return a:
/// - [Error::InvalidTag] if any `tag_id` does not refer to a valid tag,
/// - [Error::SqlError] if there is some other SQL error.
fn batch_set_transaction_tags(
    transaction_tag_pairs: Vec<(TransactionId, TagId)>,
    connection: &Connection,
) -> Result<(), Error> {
    if transaction_tag_pairs.is_empty() {
        return Ok(());
    }

    let tx = connection.unchecked_transaction()?;

    // Batch insert new tags
    let mut stmt =
        tx.prepare("UPDATE \"transaction\" SET tag_id = ?2 WHERE \"transaction\".id = ?1")?;

    for (transaction_id, tag_id) in &transaction_tag_pairs {
        stmt.execute((transaction_id, tag_id))
            .map_err(|error| match error {
                // Code 787 occurs when a FOREIGN KEY constraint failed.
                rusqlite::Error::SqliteFailure(error, Some(_)) if error.extended_code == 787 => {
                    Error::InvalidTag(Some(*tag_id))
                }
                error => error.into(),
            })?;
    }

    drop(stmt);
    tx.commit()?;
    Ok(())
}

/// Result of applying auto-tagging rules to transactions.
#[derive(Debug, Clone)]
pub struct TaggingResult {
    /// Number of transactions processed
    pub transactions_tagged: usize,
    /// Number of tags applied
    pub tags_applied: usize,
}

impl TaggingResult {
    /// Creates a new empty tagging result with zero transactions processed and zero tags applied
    pub fn empty() -> Self {
        Self {
            transactions_tagged: 0,
            tags_applied: 0,
        }
    }
}

/// Apply all rules to transactions, optionally filtering to only untagged transactions.
///
/// # Arguments
/// * `mode` - Specify which transactions to process.
/// * `connection` - Database connection
///
/// # Returns
/// Result containing statistics about the auto-tagging operation
///
/// # Errors
/// Returns an error if there are database errors during the operation
pub fn apply_rules_to_transactions(
    mode: TaggingMode,
    connection: &Connection,
) -> Result<TaggingResult, Error> {
    let rules = get_all_rules(connection)?;
    if rules.is_empty() {
        return Ok(TaggingResult::empty());
    }

    let transactions = get_transactions_for_auto_tagging(mode, connection)?;
    if transactions.is_empty() {
        return Ok(TaggingResult::empty());
    }

    let mut updates: Vec<(TransactionId, TagId)> = Vec::new();
    let mut transactions_tagged = 0;
    let mut applied_tags = HashSet::new();

    for (transaction_id, _, description) in &transactions {
        let mut matching_tag_id = None;
        for rule in &rules {
            if matches_rule_pattern(description, &rule.pattern) {
                matching_tag_id = Some(rule.tag_id);
                break;
            }
        }

        if let Some(matching_tag_id) = matching_tag_id {
            updates.push((*transaction_id, matching_tag_id));
            applied_tags.insert(matching_tag_id);
            transactions_tagged += 1;
        }
    }

    batch_set_transaction_tags(updates, connection)?;

    let tags_applied = applied_tags.len();

    Ok(TaggingResult {
        transactions_tagged,
        tags_applied,
    })
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

#[cfg(test)]
mod auto_tagging_tests {
    use std::collections::HashSet;

    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        rule::{
            TaggingMode, apply_rules_to_transactions, batch_set_transaction_tags, create_rule,
            create_rule_table, get_transactions_for_auto_tagging,
        },
        tag::{TagName, create_tag, create_tag_table},
        transaction::{Transaction, create_transaction, create_transaction_table, get_transaction},
    };

    fn get_test_db_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        create_tag_table(&connection).expect("Could not create tag table");
        create_rule_table(&connection).expect("Could not create rule table");
        create_transaction_table(&connection).expect("Could not create transaction table");
        connection
    }

    #[test]
    fn no_rules_returns_zero_results() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        // Create some transactions but no rules
        let _tx1 = create_transaction(
            Transaction::build(100.0, today, "starbucks coffee"),
            &connection,
        )
        .unwrap();
        let _tx2 = create_transaction(
            Transaction::build(50.0, today, "grocery store"),
            &connection,
        )
        .unwrap();

        let result = apply_rules_to_transactions(TaggingMode::FetchAll, &connection).unwrap();

        assert_eq!(result.transactions_tagged, 0);
        assert_eq!(result.tags_applied, 0);
    }

    #[test]
    fn no_transactions_returns_zero_results() {
        let connection = get_test_db_connection();

        // Create rules but no transactions
        let tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let _rule = create_rule("starbucks", tag.id, &connection).unwrap();

        let result = apply_rules_to_transactions(TaggingMode::FetchAll, &connection).unwrap();

        assert_eq!(result.transactions_tagged, 0);
        assert_eq!(result.tags_applied, 0);
    }

    #[test]
    fn applies_matching_rules() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        // Create tags and rules
        let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let grocery_tag = create_tag(TagName::new_unchecked("Groceries"), &connection).unwrap();
        let _coffee_rule = create_rule("starbucks", coffee_tag.id, &connection).unwrap();
        let _grocery_rule = create_rule("supermarket", grocery_tag.id, &connection).unwrap();

        // Create transactions
        let tx1 = create_transaction(
            Transaction::build(100.0, today, "starbucks downtown"),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0, today, "supermarket shopping"),
            &connection,
        )
        .unwrap();
        let _tx3 = create_transaction(Transaction::build(25.0, today, "gas station"), &connection)
            .unwrap(); // No matching rule

        let result = apply_rules_to_transactions(TaggingMode::FetchAll, &connection).unwrap();

        assert_eq!(result.transactions_tagged, 2);
        assert_eq!(result.tags_applied, 2);

        // Verify tags were applied
        let got_tx1 = get_transaction(tx1.id, &connection).unwrap();
        let got_tx2 = get_transaction(tx2.id, &connection).unwrap();

        assert_eq!(got_tx1.tag_id, Some(coffee_tag.id));
        assert_eq!(got_tx2.tag_id, Some(grocery_tag.id));
    }

    #[test]
    fn case_insensitive_matching() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        let tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let _rule = create_rule("starbucks", tag.id, &connection).unwrap();

        // Test various case combinations
        let tx1 = create_transaction(
            Transaction::build(100.0, today, "STARBUCKS CAFE"),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0, today, "Starbucks Coffee"),
            &connection,
        )
        .unwrap();
        let tx3 = create_transaction(
            Transaction::build(25.0, today, "starbucks downtown"),
            &connection,
        )
        .unwrap();

        let result = apply_rules_to_transactions(TaggingMode::FetchAll, &connection).unwrap();

        assert_eq!(result.transactions_tagged, 3);
        assert_eq!(result.tags_applied, 1);

        // Verify all transactions got tagged
        let got_tx1 = get_transaction(tx1.id, &connection).unwrap();
        let got_tx2 = get_transaction(tx2.id, &connection).unwrap();
        let got_tx3 = get_transaction(tx3.id, &connection).unwrap();

        assert_eq!(got_tx1.tag_id, Some(tag.id));
        assert_eq!(got_tx2.tag_id, Some(tag.id));
        assert_eq!(got_tx3.tag_id, Some(tag.id));
    }

    #[test]
    fn untagged_only_mode() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let existing_tag = create_tag(TagName::new_unchecked("Existing"), &connection).unwrap();
        let _rule = create_rule("starbucks", coffee_tag.id, &connection).unwrap();

        // Create transactions - one already tagged, one not
        let tx1 = create_transaction(
            Transaction::build(100.0, today, "starbucks cafe").tag_id(Some(existing_tag.id)),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0, today, "starbucks downtown"),
            &connection,
        )
        .unwrap();

        // Apply rules in untagged-only mode
        let result = apply_rules_to_transactions(TaggingMode::FetchUntagged, &connection).unwrap();

        assert_eq!(result.transactions_tagged, 1); // Only tx2 should be processed
        assert_eq!(result.tags_applied, 1);

        // Verify tx1 still has only the existing tag, tx2 has the coffee tag
        let got_tx1 = get_transaction(tx1.id, &connection).unwrap();
        let got_tx2 = get_transaction(tx2.id, &connection).unwrap();

        assert_eq!(got_tx1.tag_id, Some(existing_tag.id));
        assert_eq!(got_tx2.tag_id, Some(coffee_tag.id));
    }

    #[test]
    fn replaces_existing_tags() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let existing_tag = create_tag(TagName::new_unchecked("Existing"), &connection).unwrap();
        let _rule = create_rule("starbucks", coffee_tag.id, &connection).unwrap();

        let tx = create_transaction(
            Transaction::build(100.0, today, "starbucks cafe").tag_id(Some(existing_tag.id)),
            &connection,
        )
        .unwrap();

        // Apply rules (all mode, not untagged-only)
        let result = apply_rules_to_transactions(TaggingMode::FetchAll, &connection).unwrap();

        assert_eq!(result.transactions_tagged, 1);
        assert_eq!(result.tags_applied, 1); // Only 1 new tag added

        // Verify transaction has both tags
        let got_tx = get_transaction(tx.id, &connection).unwrap();

        assert_eq!(got_tx.tag_id, Some(coffee_tag.id));
    }

    #[test]
    fn applies_longer_rule() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        let foo_tag = create_tag(TagName::new_unchecked("Foo"), &connection).unwrap();
        let bar_tag = create_tag(TagName::new_unchecked("Bar"), &connection).unwrap();
        let _rule1 = create_rule("foo", foo_tag.id, &connection).unwrap();
        let _rule2 = create_rule("foobar", bar_tag.id, &connection).unwrap(); // Different pattern, same tag, both match "starbucks"

        let tx =
            create_transaction(Transaction::build(100.0, today, "foobar"), &connection).unwrap();

        let result = apply_rules_to_transactions(TaggingMode::FetchAll, &connection).unwrap();

        assert_eq!(result.transactions_tagged, 1);
        assert_eq!(result.tags_applied, 1); // Only 1 tag applied despite 2 matching rules
        let got_tx = get_transaction(tx.id, &connection).unwrap();
        assert_eq!(got_tx.tag_id, Some(bar_tag.id));
    }

    #[test]
    fn auto_tagging_all_mode() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        let tag = create_tag(TagName::new_unchecked("Test"), &connection).unwrap();

        // Create transactions - one tagged, one untagged
        let tx1 = create_transaction(
            Transaction::build(100.0, today, "tagged transaction").tag_id(Some(tag.id)),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0, today, "untagged transaction"),
            &connection,
        )
        .unwrap();

        let transactions =
            get_transactions_for_auto_tagging(TaggingMode::FetchAll, &connection).unwrap();

        // Should return both transactions in all mode
        assert_eq!(transactions.len(), 2);
        let tx_ids: HashSet<_> = transactions.iter().map(|(id, _, _)| *id).collect();
        assert!(tx_ids.contains(&tx1.id));
        assert!(tx_ids.contains(&tx2.id));
    }

    #[test]
    fn get_transactions_for_auto_tagging_untagged_only_mode() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        let tag = create_tag(TagName::new_unchecked("Test"), &connection).unwrap();

        // Create transactions - one tagged, one untagged
        create_transaction(
            Transaction::build(100.0, today, "tagged transaction").tag_id(Some(tag.id)),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0, today, "untagged transaction"),
            &connection,
        )
        .unwrap();

        let transactions =
            get_transactions_for_auto_tagging(TaggingMode::FetchUntagged, &connection).unwrap();

        // Should return only untagged transaction
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].0, tx2.id);
        assert_eq!(transactions[0].1, None);
        assert_eq!(transactions[0].2, "untagged transaction");
    }

    #[test]
    fn get_transactions_for_auto_tagging_from_args_mode() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        let tag = create_tag(TagName::new_unchecked("Test"), &connection).unwrap();

        // Create transactions - one tagged, one untagged
        let tx1 = create_transaction(
            Transaction::build(100.0, today, "tagged transaction").tag_id(Some(tag.id)),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0, today, "untagged transaction"),
            &connection,
        )
        .unwrap();

        let transactions = get_transactions_for_auto_tagging(
            TaggingMode::FromArgs(&[tx1.clone(), tx2.clone()]),
            &connection,
        )
        .unwrap();

        assert_eq!(transactions.len(), 2);
        assert_eq!(transactions[0].0, tx1.id);
        assert_eq!(transactions[0].1, Some(tag.id));
        assert_eq!(transactions[0].2, "tagged transaction");
        assert_eq!(transactions[1].0, tx2.id);
        assert_eq!(transactions[1].1, None);
        assert_eq!(transactions[1].2, "untagged transaction");
    }

    #[test]
    fn batch_set_transaction_tags_with_empty_vec() {
        let connection = get_test_db_connection();
        let empty_vec = Vec::new();

        let result = batch_set_transaction_tags(empty_vec, &connection);

        assert!(result.is_ok());
    }

    #[test]
    fn batch_set_transaction_tags_updates_multiple_transactions() {
        let connection = get_test_db_connection();
        let today = date!(2025 - 10 - 05);

        let tag1 = create_tag(TagName::new_unchecked("Tag1"), &connection).unwrap();
        let tag2 = create_tag(TagName::new_unchecked("Tag2"), &connection).unwrap();
        let tag3 = create_tag(TagName::new_unchecked("Tag3"), &connection).unwrap();

        let tx1 = create_transaction(
            Transaction::build(100.0, today, "tx1").tag_id(Some(tag1.id)),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0, today, "tx2").tag_id(Some(tag1.id)),
            &connection,
        )
        .unwrap();
        // Batch update both transactions
        let updates = vec![(tx1.id, tag2.id), (tx2.id, tag3.id)];

        let result = batch_set_transaction_tags(updates, &connection);
        assert!(result.is_ok());

        // Verify updates
        let got_tx1 = get_transaction(tx1.id, &connection).unwrap();
        let got_tx2 = get_transaction(tx2.id, &connection).unwrap();
        assert_eq!(got_tx1.tag_id, Some(tag2.id));
        assert_eq!(got_tx2.tag_id, Some(tag3.id));
    }
}
