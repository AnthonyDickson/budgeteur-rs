//! Import result handling for generating appropriate alert messages.

use std::{sync::OnceLock, time::Duration};

use numfmt::{Formatter, Precision};

use crate::rule::TaggingResult;

/// Builds appropriate alert templates for import operations
pub struct ImportMessageBuilder {
    transaction_count: usize,
    duration: Duration,
}

/// Contains the message and details for an alert
pub struct AlertMessage {
    pub message: String,
    pub details: String,
}

impl ImportMessageBuilder {
    /// Create a new import message builder.
    ///
    /// # Arguments
    ///
    /// * `transaction_count` - The number of transactions that were imported
    /// * `duration` - The time it took to complete the import operation
    pub fn new(transaction_count: usize, duration: Duration) -> Self {
        Self {
            transaction_count,
            duration,
        }
    }

    /// Creates a success alert message based on import and tagging results.
    ///
    /// # Arguments
    ///
    /// * `tagging_result` - The result of applying auto-tagging rules to imported transactions
    ///
    /// # Returns
    ///
    /// An `AlertMessage` with appropriate success message and timing details
    pub fn success_with_tagging(&self, tagging_result: &TaggingResult) -> AlertMessage {
        let duration_ms = get_thousands_separator_formatter().fmt_string(self.duration.as_millis());

        match (self.transaction_count, tagging_result.tags_applied) {
            (0, _) => {
                let message = "Import completed".to_string();
                let details = format!(
                    "No new transactions were imported (possibly duplicates). Completed in {duration_ms}ms."
                );
                tracing::info!("Import completed in {duration_ms}ms: no new transactions");
                AlertMessage { message, details }
            }
            (tx_count, 0) => {
                let message = "Import completed successfully!".to_string();
                let details = format!(
                    "Imported {tx_count} transactions in {duration_ms}ms. No automatic tags were applied."
                );
                tracing::info!(
                    "Import completed in {duration_ms}ms: {tx_count} transactions imported, no tags applied"
                );
                AlertMessage { message, details }
            }
            (tx_count, tag_count) => {
                let message = "Import completed successfully!".to_string();
                let details = format!(
                    "Imported {tx_count} transactions and applied {tag_count} tags automatically in {duration_ms}ms."
                );
                tracing::info!(
                    "Import completed in {duration_ms}ms: {tx_count} transactions imported, {tag_count} tags applied"
                );
                AlertMessage { message, details }
            }
        }
    }

    /// Creates an error alert message for when auto-tagging fails but import succeeds.
    ///
    /// # Arguments
    ///
    /// * `error_msg` - The error message from the failed auto-tagging operation
    ///
    /// # Returns
    ///
    /// An `AlertMessage` with appropriate error message that acknowledges successful import
    /// but failed auto-tagging, providing guidance for manual tagging if transactions were imported
    pub fn error_with_partial_success(&self, error_msg: &str) -> AlertMessage {
        let formatter = get_thousands_separator_formatter();
        let tx_count = formatter.fmt_string(self.transaction_count);
        let duration_ms = formatter.fmt_string(self.duration.as_millis());

        tracing::error!(
            "Auto-tagging failed after importing {tx_count} transactions in {duration_ms}ms: {error_msg}"
        );

        let (message, details) = if self.transaction_count > 0 {
            (
                "Import completed but auto-tagging failed".to_string(),
                format!(
                    "Imported {tx_count} transactions successfully in {duration_ms}ms, \
                    but automatic tagging failed. You can apply tags manually."
                ),
            )
        } else {
            (
                "Import completed".to_string(),
                format!("No new transactions were imported. Completed in {duration_ms}ms."),
            )
        };

        AlertMessage { message, details }
    }
}

fn get_thousands_separator_formatter() -> &'static Formatter {
    static FORMATTER: OnceLock<Formatter> = OnceLock::new();

    FORMATTER.get_or_init(|| {
        Formatter::new()
            .separator(',')
            .unwrap()
            .precision(Precision::Decimals(0))
    })
}
