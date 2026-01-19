/*! This module defines and implements traits for interacting with the application's database. */

use rusqlite::{Connection, Error, Transaction as SqlTransaction};

use crate::{
    account::create_account_table, dashboard::create_dashboard_excluded_tags_table,
    rule::create_rule_table, tag::create_tag_table, transaction::create_transaction_table,
    user::create_user_table,
};

/// Create the all of the database tables for the application.
///
/// This function will ignore any existing tables and only create the ones
/// that do not exist. If you are trying to upgrade from an older schema, then
/// you should delete the old tables first.
///
/// # Errors
/// This function may return a [rusqlite::Error] if something went wrong creating the tables.
pub fn initialize(connection: &Connection) -> Result<(), Error> {
    let transaction =
        SqlTransaction::new_unchecked(connection, rusqlite::TransactionBehavior::Exclusive)?;

    create_user_table(&transaction)?;
    create_account_table(&transaction)?;
    create_tag_table(&transaction)?;
    create_rule_table(&transaction)?;
    create_transaction_table(&transaction)?;
    create_dashboard_excluded_tags_table(&transaction)?;

    transaction.commit()?;

    Ok(())
}
