//! Transaction management for the budgeting application.
//!
//! This module contains everything related to transactions:
//! - The `Transaction` model and `TransactionBuilder` for creating transactions
//! - Database functions for storing, querying, and managing transactions
//! - View handlers for transaction-related web pages

mod core;
mod create_transaction_endpoint;
mod delete_transaction_endpoint;
mod new_transaction_page;
mod transactions_page;

pub use core::{Transaction, TransactionBuilder, create_transaction_table, map_transaction_row};
pub use create_transaction_endpoint::create_transaction_endpoint;
pub use delete_transaction_endpoint::delete_transaction_endpoint;
pub use new_transaction_page::get_new_transaction_page;
pub use transactions_page::get_transactions_page;

#[cfg(test)]
pub use core::{count_transactions, create_transaction};
