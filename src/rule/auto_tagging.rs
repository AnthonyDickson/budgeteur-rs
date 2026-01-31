use std::collections::HashSet;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use rusqlite::Connection;

use crate::{
    Error,
    alert::Alert,
    rule::{db::get_all_rules, models::RuleState},
    tag::TagId,
    transaction::{Transaction, TransactionId},
};

/// Describes which transactions should be used for a batch tagging operation.
pub enum TaggingMode<'a> {
    FetchAll,
    FetchUntagged,
    FromArgs(&'a [Transaction]),
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
                "Auto-tagging completed successfully!".to_owned()
            } else {
                "Auto-tagging completed - no transactions were processed.".to_owned()
            };

            let details = format!(
                "Tagged {} transactions with {} tags in {:.1}ms",
                result.transactions_tagged,
                result.tags_applied,
                duration.as_millis()
            );

            (
                StatusCode::OK,
                Alert::Success { message, details }.into_html(),
            )
                .into_response()
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

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Alert::Error {
                    message: "Auto-tagging failed".to_owned(),
                    details,
                }
                .into_html(),
            )
                .into_response()
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
                "Auto-tagging untagged transactions completed successfully!".to_owned()
            } else {
                "Auto-tagging completed - no untagged transactions were processed.".to_owned()
            };

            let details = format!(
                "Tagged {} untagged transactions with {} tags in {:.1}ms",
                result.transactions_tagged,
                result.tags_applied,
                duration.as_millis()
            );

            Alert::Success { message, details }.into_response()
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

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Alert::Error {
                    message: "Auto-tagging failed".to_owned(),
                    details,
                }
                .into_html(),
            )
                .into_response()
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

/// Check if a transaction description matches a rule pattern (case-insensitive).
///
/// # Arguments
/// * `description` - The transaction description to check
/// * `pattern` - The rule pattern to match against
///
/// # Returns
/// `true` if the description starts with the pattern (case-insensitive), `false` otherwise
#[inline]
fn matches_rule_pattern(description: &str, pattern: &str) -> bool {
    description
        .to_lowercase()
        .starts_with(&pattern.to_lowercase())
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
/// **Note**: If you want transactional integrity (all or nothing), pass in a
/// transaction for `connection`.
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

    // Batch insert new tags
    let mut stmt = connection
        .prepare("UPDATE \"transaction\" SET tag_id = ?2 WHERE \"transaction\".id = ?1")?;

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

    Ok(())
}

#[cfg(test)]
mod auto_tagging_tests {
    use std::collections::HashSet;

    use rusqlite::Connection;
    use time::macros::date;

    use crate::{
        rule::{
            TaggingMode,
            auto_tagging::{
                apply_rules_to_transactions, batch_set_transaction_tags,
                get_transactions_for_auto_tagging, matches_rule_pattern,
            },
            create_rule,
            db::create_rule_table,
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
