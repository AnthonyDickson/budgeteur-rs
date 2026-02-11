//! Database query helpers for the transactions page.

use rusqlite::Connection;

use crate::{Error, tag::TagName};

use super::{models::Transaction, window::WindowRange};

/// The order to sort transactions in a query.
pub(crate) enum SortOrder {
    /// Sort in order of increasing value.
    #[allow(dead_code)]
    Ascending,
    /// Sort in order of decreasing value.
    Descending,
}

/// Get transactions with sorting by date in a windowed date range.
///
/// # Arguments
/// * `window_range` - Inclusive date range of transactions to return
/// * `sort_order` - Sort direction for date field
/// * `connection` - Database connection reference
///
/// # Errors
/// Returns [Error::SqlError] if:
/// - Database connection fails
/// - SQL query preparation or execution fails
/// - Transaction row mapping fails
pub(crate) fn get_transaction_table_rows_in_range(
    window_range: WindowRange,
    sort_order: SortOrder,
    connection: &Connection,
) -> Result<Vec<Transaction>, Error> {
    let order_clause = match sort_order {
        SortOrder::Ascending => "ORDER BY date ASC",
        SortOrder::Descending => "ORDER BY date DESC",
    };

    // Sort by date, and then ID to keep transaction order stable after updates
    let query = format!(
        "SELECT \"transaction\".id, amount, date, description, tag.name, tag.id FROM \"transaction\" \
        LEFT JOIN tag ON \"transaction\".tag_id = tag.id \
        WHERE \"transaction\".date BETWEEN ?1 AND ?2 \
        {}, \"transaction\".id ASC",
        order_clause
    );

    connection
        .prepare(&query)?
        .query_map(
            [window_range.start.to_string(), window_range.end.to_string()],
            |row| {
                let tag_name = row
                    .get::<usize, Option<String>>(4)?
                    .map(|some_tag_name| TagName::new_unchecked(&some_tag_name));

                Ok(Transaction {
                    id: row.get(0)?,
                    amount: row.get(1)?,
                    date: row.get(2)?,
                    description: row.get(3)?,
                    tag_name,
                    tag_id: row.get(5)?,
                })
            },
        )?
        .map(|transaction_result| transaction_result.map_err(Error::SqlError))
        .collect()
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use time::{Duration, OffsetDateTime, macros::date};

    use crate::{
        db::initialize,
        transaction::{
            Transaction, TransactionId, create_transaction,
            models::Transaction as TableTransaction, window::WindowRange,
        },
    };

    use super::{SortOrder, get_transaction_table_rows_in_range};

    fn get_test_connection() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialize(&conn).unwrap();
        conn
    }

    #[test]
    fn get_transactions_in_range() {
        let conn = get_test_connection();

        let today = OffsetDateTime::now_utc().date();

        for i in 0..10 {
            let transaction_builder = Transaction::build(
                (i + 1) as f64,
                today - Duration::days(i),
                &format!("transaction #{i}"),
            );

            create_transaction(transaction_builder, &conn).unwrap();
        }

        let window_range = WindowRange {
            start: today - Duration::days(4),
            end: today,
        };
        let got =
            get_transaction_table_rows_in_range(window_range, SortOrder::Ascending, &conn).unwrap();

        assert_eq!(got.len(), 5, "got {} transactions, want 5", got.len());
    }

    #[test]
    fn get_transactions_in_range_orders_by_date() {
        let conn = get_test_connection();
        let today = date!(2025 - 10 - 05);
        let mut want = Vec::new();
        for i in 1..=6 {
            let date = if i <= 3 {
                today
            } else {
                today - Duration::days(1)
            };
            let transaction = create_transaction(Transaction::build(i as f64, date, ""), &conn)
                .expect("Could not create transaction");

            want.push(TableTransaction {
                id: i as TransactionId,
                amount: transaction.amount,
                date: transaction.date,
                description: transaction.description.clone(),
                tag_name: None,
                tag_id: None,
            });
        }

        want.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));

        let window_range = WindowRange {
            start: today - Duration::days(1),
            end: today,
        };
        let got = get_transaction_table_rows_in_range(window_range, SortOrder::Ascending, &conn)
            .expect("Could not query transactions");

        assert_eq!(want.len(), 6, "expected 6 transactions, got {}", want.len());
        assert_eq!(want, got);
    }
}
