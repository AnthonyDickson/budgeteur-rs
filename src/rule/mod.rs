//! This file defines the `Rule` type for auto-tagging transactions based on description patterns.
//! A rule matches transaction descriptions that start with a specific pattern and applies a tag.

mod auto_tagging;
mod create;
mod db;
mod delete;
mod edit;
mod list;
mod models;

pub use auto_tagging::{
    TaggingMode, TaggingResult, apply_rules_to_transactions, auto_tag_all_transactions_endpoint,
    auto_tag_untagged_transactions_endpoint,
};
pub use create::{create_rule_endpoint, get_new_rule_page};
pub use db::create_rule_table;
pub use delete::delete_rule_endpoint;
pub use edit::{get_edit_rule_page, update_rule_endpoint};
pub use list::get_rules_page;

#[cfg(test)]
pub use db::create_rule;
