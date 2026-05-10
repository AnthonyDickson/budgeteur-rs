use std::sync::{Arc, Mutex};

use axum::extract::FromRef;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    tag::{Tag, TagId},
};

pub type RuleId = u32;

/// A rule that automatically tags transactions whose descriptions start with a pattern.
/// Pattern matching is case-insensitive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct Rule {
    pub id: RuleId,

    /// The pattern that transaction descriptions must start with (case-insensitive).
    pub pattern: String,

    /// The ID of the tag to apply when this rule matches.
    pub tag_id: TagId,
}

/// A rule with its associated tag information for display purposes.
#[derive(Debug, Clone)]
pub(super) struct RuleWithTag {
    /// The rule itself.
    pub rule: Rule,
    /// The tag that will be applied by this rule.
    pub tag: Tag,
    /// URL for editing this rule.
    pub edit_url: String,
    /// URL for deleting this rule.
    pub delete_url: String,
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

/// Form data for creating and editing rules.
#[derive(Debug, Serialize, Deserialize)]
pub struct RuleFormData {
    /// The pattern that transaction descriptions must start with (case-insensitive).
    pub pattern: String,
    /// The ID of the tag to apply when this rule matches.
    pub tag_id: TagId,
}
