//! Shared view-model structs for the transactions page.

use time::Date;

use crate::{
    endpoints,
    tag::{TagId, TagName},
    transaction::TransactionId,
};

use super::window::{BucketPreset, WindowPreset, WindowRange};

#[derive(Debug, PartialEq)]
pub(crate) struct Transaction {
    /// The ID of the transaction.
    pub(crate) id: TransactionId,
    /// The amount of money spent or earned in this transaction.
    pub(crate) amount: f64,
    /// When the transaction happened.
    pub(crate) date: Date,
    /// A text description of what the transaction was for.
    pub(crate) description: String,
    /// The name of the transactions tag.
    pub(crate) tag_name: Option<TagName>,
    /// The ID of the transactions tag.
    pub(crate) tag_id: Option<TagId>,
}

/// Renders a transaction with its tags as a table row.
#[derive(Debug, PartialEq, Clone)]
pub(crate) struct TransactionTableRow {
    /// The amount of money spent or earned in this transaction.
    pub(crate) amount: f64,
    /// When the transaction happened.
    pub(crate) date: Date,
    /// A text description of what the transaction was for.
    pub(crate) description: String,
    /// The name of the transactions tag.
    pub(crate) tag_name: Option<TagName>,
    /// The ID of the transactions tag.
    pub(crate) tag_id: Option<TagId>,
    /// The API path to edit this transaction
    pub(crate) edit_url: String,
    /// The API path to delete this transaction
    pub(crate) delete_url: String,
}

pub(crate) struct TransactionsViewOptions {
    pub(crate) window_preset: WindowPreset,
    pub(crate) bucket_preset: BucketPreset,
    pub(crate) show_category_summary: bool,
    pub(crate) anchor_date: Date,
}

impl TransactionTableRow {
    pub(crate) fn new_from_transaction(
        transaction: Transaction,
        redirect_url: Option<&str>,
    ) -> Self {
        let mut edit_url =
            endpoints::format_endpoint(endpoints::EDIT_TRANSACTION_VIEW, transaction.id);

        if let Some(redirect_url) = redirect_url {
            edit_url = format!("{edit_url}?{redirect_url}");
        }

        Self {
            amount: transaction.amount,
            date: transaction.date,
            description: transaction.description,
            tag_name: transaction.tag_name,
            tag_id: transaction.tag_id,
            edit_url,
            delete_url: endpoints::format_endpoint(endpoints::DELETE_TRANSACTION, transaction.id),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct BucketTotals {
    pub(crate) income: f64,
    pub(crate) expenses: f64,
}

#[derive(Debug, PartialEq)]
pub(crate) struct DayGroup {
    pub(crate) date: Date,
    pub(crate) transactions: Vec<TransactionTableRow>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct DateBucket {
    pub(crate) range: WindowRange,
    pub(crate) totals: BucketTotals,
    pub(crate) days: Vec<DayGroup>,
    pub(crate) summary: Vec<CategorySummary>,
}

impl DateBucket {
    pub(crate) fn new(range: WindowRange) -> Self {
        Self {
            range,
            totals: BucketTotals {
                income: 0.0,
                expenses: 0.0,
            },
            days: Vec::new(),
            summary: Vec::new(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct CategorySummary {
    pub(crate) label: String,
    pub(crate) total: f64,
    pub(crate) percent: i64,
    pub(crate) kind: CategorySummaryKind,
    pub(crate) transactions: Vec<TransactionTableRow>,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub(crate) enum CategorySummaryKind {
    Income,
    Expense,
}
