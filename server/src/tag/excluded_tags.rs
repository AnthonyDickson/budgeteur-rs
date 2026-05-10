//! Shared excluded-tag controls and helpers.

use std::collections::HashSet;

use maud::{Markup, html};
use serde::Deserialize;

use crate::tag::{Tag, TagId};

/// Form data for updating excluded tags.
#[derive(Deserialize)]
pub struct ExcludedTagsForm {
    /// List of tag IDs to exclude from summary calculations.
    #[serde(default)]
    pub excluded_tags: Vec<TagId>,
    /// Optional redirect URL to return to after saving preferences.
    #[serde(default)]
    pub redirect_url: Option<String>,
}

/// A tag paired with its exclusion status for the filter UI.
#[derive(Debug, Clone)]
pub struct TagWithExclusion {
    /// The tag.
    pub tag: Tag,
    /// Whether this tag is currently excluded from summaries.
    pub is_excluded: bool,
}

/// Configuration for rendering excluded tag controls.
pub struct ExcludedTagsViewConfig<'a> {
    /// Heading for the section.
    pub heading: &'a str,
    /// Helper text for the section.
    pub description: &'a str,
    /// Endpoint to submit exclusion updates.
    pub endpoint: &'a str,
    /// Optional HTMX target selector.
    pub hx_target: Option<&'a str>,
    /// Optional HTMX swap strategy.
    pub hx_swap: Option<&'a str>,
    /// Optional HTMX trigger.
    pub hx_trigger: Option<&'a str>,
    /// Optional redirect URL to include as a hidden field.
    pub redirect_url: Option<&'a str>,
    /// Optional id attribute for the form.
    pub form_id: Option<&'a str>,
}

/// Build tag status models for the exclusion filter UI.
pub fn build_tags_with_exclusion_status(
    available_tags: Vec<Tag>,
    excluded_tag_ids: &[TagId],
) -> Vec<TagWithExclusion> {
    let excluded_set: HashSet<_> = excluded_tag_ids.iter().collect();
    available_tags
        .into_iter()
        .map(|tag| TagWithExclusion {
            is_excluded: excluded_set.contains(&tag.id),
            tag,
        })
        .collect()
}

/// Render the excluded tags control block.
pub fn excluded_tags_controls(
    tags_with_status: &[TagWithExclusion],
    config: ExcludedTagsViewConfig<'_>,
) -> Markup {
    if tags_with_status.is_empty() {
        return html! {};
    }

    fn push_attr(attrs: &mut Vec<String>, name: &str, value: Option<&str>) {
        if let Some(value) = value {
            attrs.push(format!("{name}=\"{value}\""));
        }
    }

    let mut attrs = Vec::new();
    attrs.push(format!("hx-post=\"{}\"", config.endpoint));
    attrs.push("hx-target-error=\"#alert-container\"".to_owned());
    attrs.push("class=\"bg-gray-50 dark:bg-gray-800 p-4 rounded w-full\"".to_owned());
    push_attr(&mut attrs, "hx-target", config.hx_target);
    push_attr(&mut attrs, "hx-swap", config.hx_swap);
    push_attr(&mut attrs, "hx-trigger", config.hx_trigger);
    push_attr(&mut attrs, "id", config.form_id);

    let form_open = format!("<form {}>", attrs.join(" "));

    html! {
        div class="mt-6 mb-8 w-full"
        {
            h3 class="text-xl font-semibold mb-4" { (config.heading) }
            (maud::PreEscaped(form_open))
            p class="text-sm text-gray-600 dark:text-gray-400 mb-3"
            {
                (config.description)
            }

            @if let Some(redirect_url) = config.redirect_url {
                input type="hidden" name="redirect_url" value=(redirect_url);
            }

            div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3"
            {
                @for tag_status in tags_with_status {
                    label class="flex items-center space-x-2"
                    {
                        input
                            type="checkbox"
                            name="excluded_tags"
                            value=(tag_status.tag.id)
                            checked[tag_status.is_excluded]
                            class="rounded-sm border-gray-300
                                text-blue-600 shadow-xs
                                focus:border-blue-300 focus:ring-3
                                focus:ring-blue-200/50"
                        ;

                        span
                            class="inline-flex items-center
                                px-2.5 py-0.5
                                text-xs font-semibold text-blue-800
                                bg-blue-100 rounded-full
                                dark:bg-blue-900 dark:text-blue-300"
                        {
                            (tag_status.tag.name)
                        }
                    }
                }
            }
            (maud::PreEscaped("</form>"))
        }
    }
}
