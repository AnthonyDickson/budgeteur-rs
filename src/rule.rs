//! This file defines the `Rule` type for auto-tagging transactions based on description patterns.
//! A rule matches transaction descriptions that start with a specific pattern and applies a tag.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

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
    selected_tag_id: DatabaseID,
    error_message: &'a str,
}

/// Renders an error message for rule operations.
#[derive(Template)]
#[template(path = "partials/rule_error.html")]
struct RuleErrorTemplate<'a> {
    error_message: &'a str,
}

/// Alert message types for styling
#[derive(Debug, Clone)]
pub enum AlertType {
    Success,
    Error,
}

/// Renders alert messages with appropriate styling
#[derive(Template)]
#[template(path = "partials/alert.html")]
struct AlertTemplate<'a> {
    alert_type: AlertType,
    message: &'a str,
    details: &'a str,
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
            auto_tag_all_route: endpoints::AUTO_TAG_ALL,
            auto_tag_untagged_route: endpoints::AUTO_TAG_UNTAGGED,
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

/// A route handler for applying auto-tagging rules to all transactions.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn auto_tag_all_transactions_endpoint(
    State(state): State<RuleState>,
) -> impl IntoResponse {
    let start_time = std::time::Instant::now();
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    match apply_rules_to_transactions(false, &connection) {
        Ok(result) => {
            let duration = start_time.elapsed();
            tracing::info!(
                "Auto-tagging all transactions completed in {:.2}ms: {} transactions processed, {} tags applied",
                duration.as_millis(),
                result.transactions_processed,
                result.tags_applied
            );

            let message = if result.transactions_processed > 0 {
                "Auto-tagging completed successfully!"
            } else {
                "Auto-tagging completed - no transactions were processed."
            };

            let details = format!(
                "Tagged {} transactions with {} tags in {:.1}ms",
                result.transactions_processed,
                result.tags_applied,
                duration.as_millis()
            );

            render(
                StatusCode::OK,
                AlertTemplate {
                    alert_type: AlertType::Success,
                    message,
                    details: &details,
                },
            )
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
                AlertTemplate {
                    alert_type: AlertType::Error,
                    message: "Auto-tagging failed",
                    details: &details,
                },
            )
        }
    }
}

/// A route handler for applying auto-tagging rules to untagged transactions only.
///
/// # Panics
///
/// Panics if the lock for the database connection is already held by the same thread.
pub async fn auto_tag_untagged_transactions_endpoint(
    State(state): State<RuleState>,
) -> impl IntoResponse {
    let start_time = std::time::Instant::now();
    let connection = state
        .db_connection
        .lock()
        .expect("Could not acquire database lock");

    match apply_rules_to_transactions(true, &connection) {
        Ok(result) => {
            let duration = start_time.elapsed();
            tracing::info!(
                "Auto-tagging untagged transactions completed in {:.2}ms: {} transactions processed, {} tags applied",
                duration.as_millis(),
                result.transactions_processed,
                result.tags_applied
            );

            let message = if result.transactions_processed > 0 {
                "Auto-tagging untagged transactions completed successfully!"
            } else {
                "Auto-tagging completed - no untagged transactions were processed."
            };

            let details = format!(
                "Tagged {} untagged transactions with {} tags in {:.1}ms",
                result.transactions_processed,
                result.tags_applied,
                duration.as_millis()
            );

            render(
                StatusCode::OK,
                AlertTemplate {
                    alert_type: AlertType::Success,
                    message,
                    details: &details,
                },
            )
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
                AlertTemplate {
                    alert_type: AlertType::Error,
                    message: "Auto-tagging failed",
                    details: &details,
                },
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
#[inline]
pub fn matches_rule_pattern(description: &str, pattern: &str) -> bool {
    description
        .to_lowercase()
        .starts_with(&pattern.to_lowercase())
}

/// Get transaction IDs and descriptions for auto-tagging, optionally filtering to untagged only.
///
/// # Arguments
/// * `untagged_only` - If true, only return transactions that have no tags
/// * `connection` - Database connection
///
/// # Returns
/// A vector of tuples containing (transaction_id, description) pairs
///
/// # Errors
/// Returns an error if there are database errors during the operation
fn get_transactions_for_auto_tagging(
    untagged_only: bool,
    connection: &Connection,
) -> Result<Vec<(DatabaseID, String)>, Error> {
    let query = if untagged_only {
        "SELECT t.id, t.description 
         FROM \"transaction\" t
         LEFT JOIN transaction_tag tt ON t.id = tt.transaction_id
         WHERE tt.transaction_id IS NULL"
    } else {
        "SELECT id, description FROM \"transaction\""
    };

    connection
        .prepare(query)?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Error::from)
}

/// Batch set tags for multiple transactions, replacing any existing tags.
/// This is more efficient than calling set_transaction_tags multiple times.
///
/// # Arguments
/// * `transaction_tag_map` - Map of transaction_id to vector of tag_ids
/// * `connection` - Database connection
///
/// # Errors
/// This function will return a:
/// - [Error::InvalidTag] if any `tag_id` does not refer to a valid tag,
/// - [Error::SqlError] if there is some other SQL error.
fn batch_set_transaction_tags(
    transaction_tag_map: HashMap<DatabaseID, Vec<DatabaseID>>,
    connection: &Connection,
) -> Result<(), Error> {
    if transaction_tag_map.is_empty() {
        return Ok(());
    }

    let tx = connection.unchecked_transaction()?;

    // Batch delete existing tags for all transactions
    let transaction_ids: Vec<DatabaseID> = transaction_tag_map.keys().copied().collect();
    let placeholders = transaction_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let delete_query = format!(
        "DELETE FROM transaction_tag WHERE transaction_id IN ({})",
        placeholders
    );

    let delete_params: Vec<&dyn rusqlite::ToSql> = transaction_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();
    tx.execute(&delete_query, &delete_params[..])?;

    // Batch insert new tags
    let mut stmt =
        tx.prepare("INSERT INTO transaction_tag (transaction_id, tag_id) VALUES (?1, ?2)")?;

    for (transaction_id, tag_ids) in &transaction_tag_map {
        for &tag_id in tag_ids {
            stmt.execute((transaction_id, tag_id))
                .map_err(|error| match error {
                    // Code 787 occurs when a FOREIGN KEY constraint failed.
                    rusqlite::Error::SqliteFailure(error, Some(_))
                        if error.extended_code == 787 =>
                    {
                        Error::InvalidTag
                    }
                    error => error.into(),
                })?;
        }
    }

    drop(stmt);
    tx.commit()?;
    Ok(())
}

/// Result of applying auto-tagging rules to transactions.
#[derive(Debug, Clone)]
pub struct AutoTaggingResult {
    /// Number of transactions processed
    pub transactions_processed: usize,
    /// Number of tags applied
    pub tags_applied: usize,
}

/// Apply all rules to transactions, optionally filtering to only untagged transactions.
///
/// # Arguments
/// * `untagged_only` - If true, only process transactions that have no tags
/// * `connection` - Database connection
///
/// # Returns
/// Result containing statistics about the auto-tagging operation
///
/// # Errors
/// Returns an error if there are database errors during the operation
pub fn apply_rules_to_transactions(
    untagged_only: bool,
    connection: &Connection,
) -> Result<AutoTaggingResult, Error> {
    // Step 1: Get all rules
    let rules = get_all_rules(connection)?;
    if rules.is_empty() {
        return Ok(AutoTaggingResult {
            transactions_processed: 0,
            tags_applied: 0,
        });
    }

    // Step 2: Get transactions for processing
    let transactions = get_transactions_for_auto_tagging(untagged_only, connection)?;
    if transactions.is_empty() {
        return Ok(AutoTaggingResult {
            transactions_processed: 0,
            tags_applied: 0,
        });
    }

    // Step 3: Get all existing transaction-tag relationships in one query (IDs only)
    let existing_tag_ids: HashMap<DatabaseID, HashSet<DatabaseID>> = connection
        .prepare("SELECT transaction_id, tag_id FROM transaction_tag")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(HashMap::new(), |mut map, (tx_id, tag_id)| {
            map.entry(tx_id).or_default().insert(tag_id);
            map
        });

    // Step 4: Collect all tag updates to apply in batch
    let mut updates: HashMap<DatabaseID, Vec<DatabaseID>> = HashMap::new();
    let mut transactions_processed = 0;
    let mut tags_applied = 0;

    for (transaction_id, description) in &transactions {
        // Find all matching rules for this transaction
        let mut matching_tag_ids = HashSet::new();
        for rule in &rules {
            if matches_rule_pattern(description, &rule.pattern) {
                matching_tag_ids.insert(rule.tag_id);
            }
        }

        // If we found matching rules, prepare the tag update
        if !matching_tag_ids.is_empty() {
            // Get existing tags for this transaction (from our cache)
            let mut all_tag_ids = existing_tag_ids
                .get(transaction_id)
                .cloned()
                .unwrap_or_default();
            let initial_tag_count = all_tag_ids.len();

            // Add new tags
            all_tag_ids.extend(matching_tag_ids);

            // Only update if there are actually new tags to add
            if all_tag_ids.len() > initial_tag_count {
                let final_tag_ids: Vec<DatabaseID> = all_tag_ids.into_iter().collect();
                tags_applied += final_tag_ids.len() - initial_tag_count;
                updates.insert(*transaction_id, final_tag_ids);
                transactions_processed += 1;
            }
        }
    }

    // Step 5: Apply all updates in a single batch transaction
    batch_set_transaction_tags(updates, connection)?;

    Ok(AutoTaggingResult {
        transactions_processed,
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

#[cfg(test)]
mod auto_tagging_tests {
    use std::collections::{HashMap, HashSet};

    use rusqlite::Connection;

    use crate::{
        rule::{
            apply_rules_to_transactions, batch_set_transaction_tags, create_rule,
            create_rule_table, get_transactions_for_auto_tagging,
        },
        tag::{TagName, create_tag, create_tag_table},
        transaction::{Transaction, create_transaction, create_transaction_table},
        transaction_tag::{
            create_transaction_tag_table, get_transaction_tags, set_transaction_tags,
        },
    };

    fn get_test_db_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        create_tag_table(&connection).expect("Could not create tag table");
        create_rule_table(&connection).expect("Could not create rule table");
        create_transaction_table(&connection).expect("Could not create transaction table");
        create_transaction_tag_table(&connection).expect("Could not create junction table");
        connection
    }

    #[test]
    fn apply_rules_to_transactions_with_no_rules_returns_zero_results() {
        let connection = get_test_db_connection();

        // Create some transactions but no rules
        let _tx1 = create_transaction(
            Transaction::build(100.0).description("starbucks coffee"),
            &connection,
        )
        .unwrap();
        let _tx2 = create_transaction(
            Transaction::build(50.0).description("grocery store"),
            &connection,
        )
        .unwrap();

        let result = apply_rules_to_transactions(false, &connection).unwrap();

        assert_eq!(result.transactions_processed, 0);
        assert_eq!(result.tags_applied, 0);
    }

    #[test]
    fn apply_rules_with_no_transactions_returns_zero_results() {
        let connection = get_test_db_connection();

        // Create rules but no transactions
        let tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let _rule = create_rule("starbucks", tag.id, &connection).unwrap();

        let result = apply_rules_to_transactions(false, &connection).unwrap();

        assert_eq!(result.transactions_processed, 0);
        assert_eq!(result.tags_applied, 0);
    }

    #[test]
    fn apply_rules_to_transactions_applies_matching_rules() {
        let connection = get_test_db_connection();

        // Create tags and rules
        let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let grocery_tag = create_tag(TagName::new_unchecked("Groceries"), &connection).unwrap();
        let _coffee_rule = create_rule("starbucks", coffee_tag.id, &connection).unwrap();
        let _grocery_rule = create_rule("supermarket", grocery_tag.id, &connection).unwrap();

        // Create transactions
        let tx1 = create_transaction(
            Transaction::build(100.0).description("starbucks downtown"),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0).description("supermarket shopping"),
            &connection,
        )
        .unwrap();
        let _tx3 = create_transaction(
            Transaction::build(25.0).description("gas station"),
            &connection,
        )
        .unwrap(); // No matching rule

        let result = apply_rules_to_transactions(false, &connection).unwrap();

        assert_eq!(result.transactions_processed, 2);
        assert_eq!(result.tags_applied, 2);

        // Verify tags were applied
        let tx1_tags = get_transaction_tags(tx1.id(), &connection).unwrap();
        let tx2_tags = get_transaction_tags(tx2.id(), &connection).unwrap();

        assert_eq!(tx1_tags.len(), 1);
        assert_eq!(tx1_tags[0].id, coffee_tag.id);
        assert_eq!(tx2_tags.len(), 1);
        assert_eq!(tx2_tags[0].id, grocery_tag.id);
    }

    #[test]
    fn apply_rules_to_transactions_case_insensitive_matching() {
        let connection = get_test_db_connection();

        let tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let _rule = create_rule("starbucks", tag.id, &connection).unwrap();

        // Test various case combinations
        let tx1 = create_transaction(
            Transaction::build(100.0).description("STARBUCKS CAFE"),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0).description("Starbucks Coffee"),
            &connection,
        )
        .unwrap();
        let tx3 = create_transaction(
            Transaction::build(25.0).description("starbucks downtown"),
            &connection,
        )
        .unwrap();

        let result = apply_rules_to_transactions(false, &connection).unwrap();

        assert_eq!(result.transactions_processed, 3);
        assert_eq!(result.tags_applied, 3);

        // Verify all transactions got tagged
        let tx1_tags = get_transaction_tags(tx1.id(), &connection).unwrap();
        let tx2_tags = get_transaction_tags(tx2.id(), &connection).unwrap();
        let tx3_tags = get_transaction_tags(tx3.id(), &connection).unwrap();

        assert_eq!(tx1_tags.len(), 1);
        assert_eq!(tx2_tags.len(), 1);
        assert_eq!(tx3_tags.len(), 1);
    }

    #[test]
    fn apply_rules_to_transactions_untagged_only_mode() {
        let connection = get_test_db_connection();

        let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let existing_tag = create_tag(TagName::new_unchecked("Existing"), &connection).unwrap();
        let _rule = create_rule("starbucks", coffee_tag.id, &connection).unwrap();

        // Create transactions - one already tagged, one not
        let tx1 = create_transaction(
            Transaction::build(100.0).description("starbucks cafe"),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0).description("starbucks downtown"),
            &connection,
        )
        .unwrap();

        // Tag tx1 with an existing tag
        set_transaction_tags(tx1.id(), &[existing_tag.id], &connection).unwrap();

        // Apply rules in untagged-only mode
        let result = apply_rules_to_transactions(true, &connection).unwrap();

        assert_eq!(result.transactions_processed, 1); // Only tx2 should be processed
        assert_eq!(result.tags_applied, 1);

        // Verify tx1 still has only the existing tag, tx2 has the coffee tag
        let tx1_tags = get_transaction_tags(tx1.id(), &connection).unwrap();
        let tx2_tags = get_transaction_tags(tx2.id(), &connection).unwrap();

        assert_eq!(tx1_tags.len(), 1);
        assert_eq!(tx1_tags[0].id, existing_tag.id);
        assert_eq!(tx2_tags.len(), 1);
        assert_eq!(tx2_tags[0].id, coffee_tag.id);
    }

    #[test]
    fn apply_rules_to_transactions_merges_with_existing_tags() {
        let connection = get_test_db_connection();

        let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let existing_tag = create_tag(TagName::new_unchecked("Existing"), &connection).unwrap();
        let _rule = create_rule("starbucks", coffee_tag.id, &connection).unwrap();

        let tx = create_transaction(
            Transaction::build(100.0).description("starbucks cafe"),
            &connection,
        )
        .unwrap();

        // Give transaction an existing tag
        set_transaction_tags(tx.id(), &[existing_tag.id], &connection).unwrap();

        // Apply rules (all mode, not untagged-only)
        let result = apply_rules_to_transactions(false, &connection).unwrap();

        assert_eq!(result.transactions_processed, 1);
        assert_eq!(result.tags_applied, 1); // Only 1 new tag added

        // Verify transaction has both tags
        let tx_tags = get_transaction_tags(tx.id(), &connection).unwrap();
        let tag_ids: HashSet<_> = tx_tags.iter().map(|t| t.id).collect();

        assert_eq!(tx_tags.len(), 2);
        assert!(tag_ids.contains(&existing_tag.id));
        assert!(tag_ids.contains(&coffee_tag.id));
    }

    #[test]
    fn apply_rules_to_transactions_multiple_rules_match_same_transaction() {
        let connection = get_test_db_connection();

        let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        let chain_tag = create_tag(TagName::new_unchecked("Chain"), &connection).unwrap();
        let _coffee_rule = create_rule("starbucks", coffee_tag.id, &connection).unwrap();
        let _chain_rule = create_rule("starbucks", chain_tag.id, &connection).unwrap(); // Same pattern, different tag

        let tx = create_transaction(
            Transaction::build(100.0).description("starbucks downtown"),
            &connection,
        )
        .unwrap();

        let result = apply_rules_to_transactions(false, &connection).unwrap();

        assert_eq!(result.transactions_processed, 1);
        assert_eq!(result.tags_applied, 2); // Both rules applied

        // Verify transaction has both tags
        let tx_tags = get_transaction_tags(tx.id(), &connection).unwrap();
        let tag_ids: HashSet<_> = tx_tags.iter().map(|t| t.id).collect();

        assert_eq!(tx_tags.len(), 2);
        assert!(tag_ids.contains(&coffee_tag.id));
        assert!(tag_ids.contains(&chain_tag.id));
    }

    #[test]
    fn apply_rules_to_transactions_no_duplicate_tags() {
        let connection = get_test_db_connection();

        let coffee_tag = create_tag(TagName::new_unchecked("Coffee"), &connection).unwrap();
        // Create two rules with different patterns but same tag that both match the same transaction
        let _rule1 = create_rule("starbucks", coffee_tag.id, &connection).unwrap();
        let _rule2 = create_rule("star", coffee_tag.id, &connection).unwrap(); // Different pattern, same tag, both match "starbucks"

        let tx = create_transaction(
            Transaction::build(100.0).description("starbucks cafe"),
            &connection,
        )
        .unwrap();

        let result = apply_rules_to_transactions(false, &connection).unwrap();

        assert_eq!(result.transactions_processed, 1);
        assert_eq!(result.tags_applied, 1); // Only 1 tag applied despite 2 matching rules

        // Verify transaction has only one tag (no duplicates)
        let tx_tags = get_transaction_tags(tx.id(), &connection).unwrap();

        assert_eq!(tx_tags.len(), 1);
        assert_eq!(tx_tags[0].id, coffee_tag.id);
    }

    #[test]
    fn get_transactions_for_auto_tagging_all_mode() {
        let connection = get_test_db_connection();

        let tag = create_tag(TagName::new_unchecked("Test"), &connection).unwrap();

        // Create transactions - one tagged, one untagged
        let tx1 = create_transaction(
            Transaction::build(100.0).description("tagged transaction"),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0).description("untagged transaction"),
            &connection,
        )
        .unwrap();

        // Tag tx1
        set_transaction_tags(tx1.id(), &[tag.id], &connection).unwrap();

        let transactions = get_transactions_for_auto_tagging(false, &connection).unwrap();

        // Should return both transactions in all mode
        assert_eq!(transactions.len(), 2);
        let tx_ids: HashSet<_> = transactions.iter().map(|(id, _)| *id).collect();
        assert!(tx_ids.contains(&tx1.id()));
        assert!(tx_ids.contains(&tx2.id()));
    }

    #[test]
    fn get_transactions_for_auto_tagging_untagged_only_mode() {
        let connection = get_test_db_connection();

        let tag = create_tag(TagName::new_unchecked("Test"), &connection).unwrap();

        // Create transactions - one tagged, one untagged
        let tx1 = create_transaction(
            Transaction::build(100.0).description("tagged transaction"),
            &connection,
        )
        .unwrap();
        let tx2 = create_transaction(
            Transaction::build(50.0).description("untagged transaction"),
            &connection,
        )
        .unwrap();

        // Tag tx1
        set_transaction_tags(tx1.id(), &[tag.id], &connection).unwrap();

        let transactions = get_transactions_for_auto_tagging(true, &connection).unwrap();

        // Should return only untagged transaction
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].0, tx2.id());
        assert_eq!(transactions[0].1, "untagged transaction");
    }

    #[test]
    fn batch_set_transaction_tags_with_empty_map() {
        let connection = get_test_db_connection();
        let empty_map = HashMap::new();

        let result = batch_set_transaction_tags(empty_map, &connection);

        assert!(result.is_ok());
    }

    #[test]
    fn batch_set_transaction_tags_updates_multiple_transactions() {
        let connection = get_test_db_connection();

        let tag1 = create_tag(TagName::new_unchecked("Tag1"), &connection).unwrap();
        let tag2 = create_tag(TagName::new_unchecked("Tag2"), &connection).unwrap();
        let tag3 = create_tag(TagName::new_unchecked("Tag3"), &connection).unwrap();

        let tx1 =
            create_transaction(Transaction::build(100.0).description("tx1"), &connection).unwrap();
        let tx2 =
            create_transaction(Transaction::build(50.0).description("tx2"), &connection).unwrap();

        // Set initial tags
        set_transaction_tags(tx1.id(), &[tag1.id], &connection).unwrap();
        set_transaction_tags(tx2.id(), &[tag1.id, tag2.id], &connection).unwrap();

        // Batch update both transactions
        let mut updates = HashMap::new();
        updates.insert(tx1.id(), vec![tag2.id, tag3.id]);
        updates.insert(tx2.id(), vec![tag3.id]);

        let result = batch_set_transaction_tags(updates, &connection);
        assert!(result.is_ok());

        // Verify updates
        let tx1_tags = get_transaction_tags(tx1.id(), &connection).unwrap();
        let tx2_tags = get_transaction_tags(tx2.id(), &connection).unwrap();

        let tx1_tag_ids: HashSet<_> = tx1_tags.iter().map(|t| t.id).collect();
        let tx2_tag_ids: HashSet<_> = tx2_tags.iter().map(|t| t.id).collect();

        assert_eq!(tx1_tag_ids, HashSet::from([tag2.id, tag3.id]));
        assert_eq!(tx2_tag_ids, HashSet::from([tag3.id]));
    }
}
