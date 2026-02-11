//! Transaction management for the budgeting application.
//!
//! This module contains everything related to transactions:
//! - The `Transaction` model and `TransactionBuilder` for creating transactions
//! - Database functions for storing, querying, and managing transactions
//! - View handlers for transaction-related web pages

mod core;
mod create_endpoint;
mod create_page;
mod delete_endpoint;
mod edit_endpoint;
mod edit_page;
mod form;
mod grouping;
mod models;
mod query;
#[cfg(test)]
mod test_utils;
mod transactions_page;
mod view;
mod window;

pub use core::{
    Transaction, TransactionBuilder, TransactionId, create_transaction_table, get_transaction,
    map_transaction_row,
};
pub use create_endpoint::create_transaction_endpoint;
pub use create_page::get_create_transaction_page;
pub use delete_endpoint::delete_transaction_endpoint;
pub use edit_endpoint::edit_transaction_endpoint;
pub use edit_page::get_edit_transaction_page;
pub use transactions_page::get_transactions_page;

#[cfg(test)]
pub use core::count_transactions;
#[cfg(test)]
pub use core::create_transaction;
