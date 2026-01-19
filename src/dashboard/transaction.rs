//! Database queries for retrieving dashboard transaction data.
//!
//! This module provides a simplified transaction view optimized for dashboard
//! aggregations, containing only the fields needed for charting (amount, date, tag).

use std::ops::RangeInclusive;

use rusqlite::{Connection, params_from_iter};
use time::Date;

use crate::{Error, database_id::DatabaseId};

pub(super) const UNTAGGED_LABEL: &str = "Other";

/// A simplified transaction view for dashboard aggregations.
///
/// This is separate from the main Transaction domain model because
/// the dashboard only needs amount, date, and tag name for charting.
#[derive(Debug)]
pub(super) struct Transaction {
    pub amount: f64,
    pub date: Date,
    pub tag: String,
}

/// Gets transactions and their tags within a date range.
///
/// # Arguments
/// * `date_range` - The inclusive date range to query
/// * `excluded_tags` - Optional slice of tag IDs to exclude from results
/// * `connection` - Database connection reference
///
/// # Returns
/// Vector of simplified transaction views containing amount, date, and tag name.
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
pub(super) fn get_transactions_in_date_range(
    date_range: RangeInclusive<Date>,
    excluded_tags: Option<&[DatabaseId]>,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
    let base_query = format!(
        "SELECT 
            t.amount,
            t.date,
            COALESCE(tag.name, '{UNTAGGED_LABEL}') AS tag_name
        FROM \"transaction\" t
        LEFT JOIN tag ON tag.id = t.tag_id
        WHERE t.date BETWEEN ?1 AND ?2"
    );

    let (query, params) = if let Some(tags) = excluded_tags.filter(|t| !t.is_empty()) {
        let placeholders = tags.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_without_excluded_tags =
            format!("{base_query} AND (t.tag_id IS NULL OR t.tag_id NOT IN ({placeholders}))");

        let mut params = vec![date_range.start().to_string(), date_range.end().to_string()];
        params.extend(tags.iter().map(|tag| tag.to_string()));
        (query_without_excluded_tags, params)
    } else {
        (
            base_query.to_owned(),
            vec![date_range.start().to_string(), date_range.end().to_string()],
        )
    };

    let mut stmt = connection.prepare(&query)?;
    stmt.query_map(params_from_iter(params), |row| {
        Ok(Transaction {
            amount: row.get(0)?,
            date: row.get(1)?,
            tag: row.get(2)?,
        })
    })?
    .collect::<Result<Vec<Transaction>, rusqlite::Error>>()
    .map_err(|error| error.into())
}
#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use time::macros::date;

    use super::get_transactions_in_date_range;
    use crate::{
        db::initialize,
        tag::{TagName, create_tag},
        transaction::{Transaction, create_transaction},
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn returns_transactions_in_date_range() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test transactions
        create_transaction(Transaction::build(100.0, start_date, ""), &conn).unwrap();
        create_transaction(Transaction::build(-50.0, date!(2024 - 01 - 15), ""), &conn).unwrap();
        create_transaction(Transaction::build(75.0, end_date, ""), &conn).unwrap();

        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 3);

        // Verify amounts are correct
        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!(total, 125.0); // 100 - 50 + 75
    }

    #[test]
    fn returns_empty_vec_for_no_transactions() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 0);
    }

    #[test]
    fn excludes_transactions_outside_date_range() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Transactions within range
        create_transaction(Transaction::build(100.0, start_date, ""), &conn).unwrap();
        create_transaction(Transaction::build(-50.0, end_date, ""), &conn).unwrap();

        // Transactions outside range
        create_transaction(Transaction::build(200.0, date!(2023 - 12 - 31), ""), &conn).unwrap();
        create_transaction(Transaction::build(-100.0, date!(2024 - 02 - 01), ""), &conn).unwrap();

        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 2);
        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!(total, 50.0); // 100 - 50
    }

    #[test]
    fn excludes_transactions_with_excluded_tags() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test tags
        let excluded_tag = create_tag(TagName::new("ExcludedTag").unwrap(), &conn).unwrap();
        let included_tag = create_tag(TagName::new("IncludedTag").unwrap(), &conn).unwrap();

        // Create test transactions
        let _excluded_transaction = create_transaction(
            Transaction::build(100.0, start_date, "").tag_id(Some(excluded_tag.id)),
            &conn,
        )
        .unwrap();
        let _included_transaction = create_transaction(
            Transaction::build(50.0, start_date, "").tag_id(Some(included_tag.id)),
            &conn,
        )
        .unwrap();
        let _untagged_transaction =
            create_transaction(Transaction::build(25.0, start_date, ""), &conn).unwrap();

        // Get transactions excluding the excluded tag
        let excluded_tags = vec![excluded_tag.id];
        let transactions =
            get_transactions_in_date_range(start_date..=end_date, Some(&excluded_tags), &conn)
                .unwrap();

        assert_eq!(transactions.len(), 2, "Got transactions: {transactions:#?}");
        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!(total, 75.0); // 50 + 25, excluding 100
    }

    #[test]
    fn includes_all_transactions_when_no_tags_excluded() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create test tag
        let tag = create_tag(TagName::new("TestTag").unwrap(), &conn).unwrap();

        // Create test transactions
        let _tagged_transaction = create_transaction(
            Transaction::build(100.0, start_date, "").tag_id(Some(tag.id)),
            &conn,
        )
        .unwrap();
        let _untagged_transaction =
            create_transaction(Transaction::build(50.0, start_date, ""), &conn).unwrap();

        // Get transactions with no exclusions
        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 2);
        let total: f64 = transactions.iter().map(|t| t.amount).sum();
        assert_eq!(total, 150.0); // 100 + 50
    }

    #[test]
    fn assigns_other_tag_to_untagged_transactions() {
        let conn = get_test_connection();
        let start_date = date!(2024 - 01 - 01);
        let end_date = date!(2024 - 01 - 31);

        // Create untagged transaction
        create_transaction(Transaction::build(100.0, start_date, ""), &conn).unwrap();

        let transactions =
            get_transactions_in_date_range(start_date..=end_date, None, &conn).unwrap();

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].tag, "Other");
    }
}
