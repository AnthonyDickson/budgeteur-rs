//! Data access helpers for the imported-untagged transactions queue.

use rusqlite::{Connection, ToSql, params_from_iter};
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{Error, tag::TagId};

use crate::transaction::TransactionId;

pub const QUICK_TAGGING_QUEUE_PAGE_SIZE: usize = 20;

#[derive(Debug, Clone, PartialEq)]
pub struct UntaggedTransactionRow {
    pub id: TransactionId,
    pub amount: f64,
    pub date: Date,
    pub description: String,
}

pub fn create_quick_tagging_table(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS untagged_transaction (\
            transaction_id INTEGER PRIMARY KEY,\
            created_at TEXT NOT NULL,\
            FOREIGN KEY(transaction_id) REFERENCES \"transaction\"(id) ON DELETE CASCADE\
        )",
        (),
    )?;

    connection.execute(
        "CREATE TRIGGER IF NOT EXISTS remove_from_untagged_queue_on_tag_set
        AFTER UPDATE OF tag_id ON \"transaction\"
        WHEN NEW.tag_id IS NOT NULL
        BEGIN
            DELETE FROM untagged_transaction WHERE transaction_id = NEW.id;
        END;",
        (),
    )?;

    Ok(())
}

pub fn insert_untagged_transactions_for_import(
    transaction_ids: &[TransactionId],
    created_at: OffsetDateTime,
    connection: &Connection,
) -> Result<usize, Error> {
    if transaction_ids.is_empty() {
        return Ok(0);
    }

    let created_at = created_at
        .format(&Rfc3339)
        .unwrap_or_else(|_| created_at.to_string());

    let placeholders = std::iter::repeat_n("?", transaction_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "INSERT INTO untagged_transaction (transaction_id, created_at)
        SELECT id, ?1 FROM \"transaction\"
        WHERE id IN ({placeholders}) AND tag_id IS NULL"
    );

    let mut params: Vec<&dyn ToSql> = Vec::with_capacity(transaction_ids.len() + 1);
    params.push(&created_at);
    for transaction_id in transaction_ids {
        params.push(transaction_id);
    }

    connection
        .execute(&query, params_from_iter(params))
        .map_err(Error::from)
}

pub fn get_untagged_transactions(
    limit: usize,
    connection: &Connection,
) -> Result<Vec<UntaggedTransactionRow>, Error> {
    connection
        .prepare(
            "SELECT \"transaction\".id, amount, date, description
            FROM untagged_transaction
            JOIN \"transaction\" ON \"transaction\".id = untagged_transaction.transaction_id
            ORDER BY untagged_transaction.created_at DESC, untagged_transaction.transaction_id DESC
            LIMIT ?1",
        )?
        .query_map([limit as i64], |row| {
            Ok(UntaggedTransactionRow {
                id: row.get(0)?,
                amount: row.get(1)?,
                date: row.get(2)?,
                description: row.get(3)?,
            })
        })?
        .map(|result| result.map_err(Error::from))
        .collect()
}

pub fn apply_quick_tagging_updates(
    tag_updates: &[(TransactionId, TagId)],
    connection: &Connection,
) -> Result<usize, Error> {
    if tag_updates.is_empty() {
        return Ok(0);
    }

    let mut stmt = connection.prepare(
        "UPDATE \"transaction\"
        SET tag_id = ?2
        WHERE id = ?1
        AND EXISTS (
            SELECT 1 FROM untagged_transaction WHERE transaction_id = ?1
        )",
    )?;

    let mut updated_rows = 0;

    for (transaction_id, tag_id) in tag_updates {
        let rows = stmt
            .execute((transaction_id, tag_id))
            .map_err(|error| match error {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: _,
                        extended_code: rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY,
                    },
                    _,
                ) => Error::InvalidTag(Some(*tag_id)),
                error => error.into(),
            })?;

        updated_rows += rows;
    }

    Ok(updated_rows)
}

pub fn dismiss_untagged_transactions(
    transaction_ids: &[TransactionId],
    connection: &Connection,
) -> Result<usize, Error> {
    if transaction_ids.is_empty() {
        return Ok(0);
    }

    let placeholders = std::iter::repeat_n("?", transaction_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let query =
        format!("DELETE FROM untagged_transaction WHERE transaction_id IN ({placeholders})");

    let params: Vec<&dyn ToSql> = transaction_ids.iter().map(|id| id as &dyn ToSql).collect();

    connection
        .execute(&query, params_from_iter(params))
        .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use time::{OffsetDateTime, macros::date};

    use crate::{
        db::initialize,
        tag::{TagName, create_tag},
        transaction::{Transaction, create_transaction, get_transaction},
    };

    use super::{
        QUICK_TAGGING_QUEUE_PAGE_SIZE, UntaggedTransactionRow, apply_quick_tagging_updates,
        dismiss_untagged_transactions, get_untagged_transactions,
        insert_untagged_transactions_for_import,
    };

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn insert_and_fetch_untagged_transactions() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let tx = create_transaction(Transaction::build(12.34, today, "queue"), &conn)
            .expect("could not create transaction");
        let created_at = OffsetDateTime::now_utc();

        insert_untagged_transactions_for_import(&[tx.id], created_at, &conn)
            .expect("could not insert queue rows");

        let rows = get_untagged_transactions(QUICK_TAGGING_QUEUE_PAGE_SIZE, &conn)
            .expect("could not fetch queue rows");

        assert_eq!(
            rows,
            vec![UntaggedTransactionRow {
                id: tx.id,
                amount: tx.amount,
                date: tx.date,
                description: tx.description,
            }]
        );
    }

    #[test]
    fn apply_tag_updates_removes_queue_entry() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let tag =
            create_tag(TagName::new_unchecked("Groceries"), &conn).expect("could not create tag");
        let tx = create_transaction(Transaction::build(12.34, today, "queue"), &conn)
            .expect("could not create transaction");
        let created_at = OffsetDateTime::now_utc();

        insert_untagged_transactions_for_import(&[tx.id], created_at, &conn)
            .expect("could not insert queue rows");

        let updated =
            apply_quick_tagging_updates(&[(tx.id, tag.id)], &conn).expect("could not update tag");
        assert_eq!(updated, 1);

        let rows = get_untagged_transactions(QUICK_TAGGING_QUEUE_PAGE_SIZE, &conn)
            .expect("could not fetch queue rows");
        assert!(rows.is_empty());

        let updated_tx = get_transaction(tx.id, &conn).expect("could not fetch transaction");
        assert_eq!(updated_tx.tag_id, Some(tag.id));
    }

    #[test]
    fn dismiss_removes_queue_entry_without_changing_tag() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let tx = create_transaction(Transaction::build(12.34, today, "queue"), &conn)
            .expect("could not create transaction");
        let created_at = OffsetDateTime::now_utc();

        insert_untagged_transactions_for_import(&[tx.id], created_at, &conn)
            .expect("could not insert queue rows");

        let dismissed =
            dismiss_untagged_transactions(&[tx.id], &conn).expect("could not dismiss queue row");
        assert_eq!(dismissed, 1);

        let rows = get_untagged_transactions(QUICK_TAGGING_QUEUE_PAGE_SIZE, &conn)
            .expect("could not fetch queue rows");
        assert!(rows.is_empty());

        let updated_tx = get_transaction(tx.id, &conn).expect("could not fetch transaction");
        assert_eq!(updated_tx.tag_id, None);
    }
}
