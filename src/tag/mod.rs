//! Tag management for categorizing transactions.

mod create;
mod db;
mod delete;
mod domain;
mod edit;
mod excluded_tags;
mod list;
mod preferences;

pub use create::{create_tag_endpoint, get_new_tag_page};
pub use db::{create_tag, create_tag_table, get_all_tags, get_tag, update_tag};
pub use delete::delete_tag_endpoint;
pub use domain::{Tag, TagId, TagName};
pub use edit::{get_edit_tag_page, update_tag_endpoint};
pub use excluded_tags::{
    ExcludedTagsForm, ExcludedTagsViewConfig, TagWithExclusion, build_tags_with_exclusion_status,
    excluded_tags_controls,
};
pub use list::get_tags_page;
pub use preferences::{create_excluded_tags_table, get_excluded_tags, save_excluded_tags};
