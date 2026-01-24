//! Card components for displaying expense breakdown by tag.
//!
//! Provides card-based visualizations showing:
//! - Current month spending per tag
//! - Percentage of total expenses
//! - Trend indicators (overspending/saving/on-track)
//! - Annual spending impact projections

use maud::{Markup, html};

use crate::{
    dashboard::aggregation::TagExpenseStats,
    endpoints,
    html::{LINK_STYLE, format_currency},
};

const DISPLAY_THRESHOLD: f64 = 5.5; // Anything < 5.5 rounds to "5" or less
const MINIMUM_MONTHS_OF_DATA: usize = 2;

/// The state of a tag's spending relative to historical patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardState {
    /// Spending is ≥5% above average
    Overspending,
    /// Spending is ≥5% below average
    Saving,
    /// Spending is within 5% of average
    OnTrack,
    /// Less than 1 month of data available
    InsufficientData,
}

/// Determines the card state based on spending patterns.
fn determine_card_state(stat: &TagExpenseStats) -> CardState {
    if stat.months_of_data < MINIMUM_MONTHS_OF_DATA {
        return CardState::InsufficientData;
    }

    let abs_change = stat.percentage_change.abs();

    // Significant change = would display as "6%" or more
    if abs_change < DISPLAY_THRESHOLD {
        CardState::OnTrack
    } else if stat.percentage_change >= DISPLAY_THRESHOLD {
        CardState::Overspending
    } else {
        CardState::Saving
    }
}

/// Formats a percentage value, avoiding "-0%" display.
fn format_percentage(value: f64) -> String {
    let rounded = value.round();
    if rounded.abs() < 0.5 {
        "0".to_string()
    } else {
        format!("{:.0}", rounded)
    }
}

/// Renders the expense cards section with all tag statistics.
///
/// Shows empty state if no tags exist, or a grid of expense cards.
/// Includes a helper card if there are only 1-2 tags to encourage categorization.
///
/// # Arguments
/// * `tag_stats` - Statistics for each tag to display
///
/// # Returns
/// Maud markup containing the expense cards section.
pub(super) fn expense_cards_view(tag_stats: &[TagExpenseStats]) -> Markup {
    if tag_stats.is_empty() {
        return empty_state_view();
    }

    html! {
        section class="w-full mx-auto mt-8 mb-8" {
            // Section header
            div class="flex justify-between items-baseline mb-4" {
                h3 class="text-xl font-semibold" {
                    "Expenses by Tag"
                }
                span class="text-sm text-gray-600 dark:text-gray-400" {
                    "Last 12 months"
                }
            }

            // Card grid
            div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4" {
                @for stat in tag_stats {
                    (expense_card(stat))
                }

                // Show helper card if user has few tags
                @if tag_stats.len() <= 2 {
                    (helper_card())
                }
            }
        }
    }
}

/// Renders a single expense card for a tag.
fn expense_card(stat: &TagExpenseStats) -> Markup {
    let state = determine_card_state(stat);

    html! {
        div
            class="bg-white dark:bg-gray-800 border border-gray-200
                   dark:border-gray-700 rounded-lg p-4 shadow-md 
                   hover:shadow-lg transition-shadow min-h-[200px] 
                   flex flex-col justify-between"
            aria-label=(format!(
                "{} expenses: {} this month, {} from usual {} average{}",
                stat.tag,
                format_currency(stat.current_month_amount),
                if stat.percentage_change >= 0.0 {
                    format!("up {}%", format_percentage(stat.percentage_change))
                } else {
                    format!("down {}%", format_percentage(stat.percentage_change.abs()))
                },
                format_currency(stat.monthly_average),
                if state == CardState::InsufficientData {
                    ", building baseline".to_string()
                } else if state != CardState::OnTrack {
                    format!(", resulting in {} {} per year",
                        format_currency(stat.annual_delta.abs()),
                        if stat.annual_delta >= 0.0 { "more spending" } else { "savings" }
                    )
                } else {
                    String::new()
                }
            ))
        {
            div {
                // Tag name
                h4 class="text-lg font-semibold mb-3 truncate"
                   title=(stat.tag) {
                    (stat.tag)
                }

                // Current amount
                div class="text-3xl font-bold mb-1" {
                    (format_currency(stat.current_month_amount))
                }

                // Percentage of total
                div class="text-sm text-gray-600 dark:text-gray-400 mb-2" {
                    (format_percentage(stat.percentage_of_total)) "% of expenses"
                }

                // Progress bar
                (progress_bar(stat.percentage_of_total))
            }

            // Bottom content (varies by state)
            (card_bottom_content(stat, state))
        }
    }
}

/// Renders the bottom portion of a card based on its state.
fn card_bottom_content(stat: &TagExpenseStats, state: CardState) -> Markup {
    html! {
        div class="mt-3 space-y-1" {
            @match state {
                CardState::InsufficientData => {
                    div class="text-sm text-blue-600 dark:text-blue-400" {
                        "Building baseline..."
                    }
                }
                CardState::OnTrack => {
                    div class="text-sm" {
                        "Avg: " (format_currency(stat.monthly_average)) "/month"
                    }
                    div class="text-sm text-gray-600 dark:text-gray-400" {
                        "→ On track"
                    }
                }
                CardState::Overspending => {
                    div class="text-sm" {
                        "Avg: " (format_currency(stat.monthly_average)) "/month"
                    }
                    div class="text-sm font-medium text-red-600 dark:text-red-400" {
                        "↑ +" (format_percentage(stat.percentage_change)) "% above usual"
                    }
                    div class="text-sm font-semibold text-red-600 dark:text-red-400" {
                        "💡 +" (format_currency(stat.annual_delta)) "/year"
                    }
                }
                CardState::Saving => {
                    div class="text-sm" {
                        "Avg: " (format_currency(stat.monthly_average)) "/month"
                    }
                    div class="text-sm font-medium text-green-600 dark:text-green-400" {
                        "↓ -" (format_percentage(stat.percentage_change.abs())) "% below usual"
                    }
                    div class="text-sm font-semibold text-green-600 dark:text-green-400" {
                        "💡 -" (format_currency(stat.annual_delta.abs())) "/year 🎉"
                    }
                }
            }
        }
    }
}

/// Renders a horizontal progress bar showing percentage of total expenses.
fn progress_bar(percentage: f64) -> Markup {
    let clamped = percentage.clamp(0.0, 100.0);

    // Ensure minimum 3% width so rounded corners are visible
    let display_percentage = if clamped > 0.0 && clamped < 3.0 {
        3.0
    } else {
        clamped
    };

    html! {
        div
            class="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2.5 mb-2"
            role="progressbar"
            aria-valuenow=(format_percentage(clamped))
            aria-valuemin="0"
            aria-valuemax="100"
        {
            @if clamped > 0.0 {
                div
                    class="bg-blue-600 dark:bg-blue-500 h-2.5 rounded-full transition-all"
                    style=(format!("width: {:.1}%", display_percentage))
                {}
            }
        }
    }
}

/// Renders an empty state when no tags exist.
fn empty_state_view() -> Markup {
    html! {
        section class="w-full mx-auto mt-8 mb-8" {
            div class="bg-white dark:bg-gray-800 border border-gray-200
                       dark:border-gray-700 rounded-lg p-8 shadow-md 
                       text-center max-w-md mx-auto" {
                div class="text-4xl mb-4" { "💡" }
                h3 class="text-xl font-semibold mb-3" {
                    "Get Started"
                }
                p class="text-gray-700 dark:text-gray-300 mb-4" {
                    "Add tags to see detailed spending breakdown!"
                }
                p class="text-sm text-gray-600 dark:text-gray-400 mb-6" {
                    "Tags help you understand where your money goes."
                }
                a
                    href=(endpoints::TAGS_VIEW)
                    class=(LINK_STYLE)
                {
                    "Manage Tags →"
                }
            }
        }
    }
}

/// Renders a helper card encouraging users to add more tags.
fn helper_card() -> Markup {
    html! {
        div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200
                   dark:border-blue-800 rounded-lg p-4 shadow-md 
                   min-h-[200px] flex flex-col justify-between" {
            div {
                div class="text-3xl mb-3" { "💡" }
                h4 class="text-lg font-semibold mb-3" {
                    "Tip"
                }
                p class="text-sm text-gray-700 dark:text-gray-300 mb-2" {
                    "Add more tags to see detailed spending breakdown!"
                }
                p class="text-sm text-gray-600 dark:text-gray-400" {
                    "Keep tags broad (aim for ~10 tags)."
                }
            }
            a
                href=(endpoints::TAGS_VIEW)
                class=(LINK_STYLE)
            {
                "Manage Tags →"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Test suite for expense cards
    //!
    //! Key testing principles:
    //! 1. **Boundary testing**: Card state changes should align with displayed percentages
    //! 2. **Consistency**: Same displayed value should never produce different states
    //! 3. **Rounding alignment**: State thresholds must account for percentage rounding
    //!
    //! The critical invariant: If two expenses display the same rounded percentage
    //! (e.g., both show "5%"), they MUST show the same card state.

    use super::*;

    fn create_test_stat(
        tag: &str,
        current: f64,
        average: f64,
        months: usize,
        percentage_of_total: f64,
    ) -> TagExpenseStats {
        let percentage_change = if average > 0.0 {
            ((current - average) / average) * 100.0
        } else {
            0.0
        };

        TagExpenseStats {
            tag: tag.to_owned(),
            current_month_amount: current,
            percentage_of_total,
            monthly_average: average,
            percentage_change,
            annual_delta: (current - average) * 12.0,
            months_of_data: months,
        }
    }

    #[test]
    fn card_state_insufficient_data() {
        let stat = create_test_stat("Food", 100.0, 100.0, 0, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::InsufficientData);
    }

    #[test]
    fn card_state_on_track() {
        let stat = create_test_stat("Food", 100.0, 98.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::OnTrack);
    }

    #[test]
    fn card_state_overspending() {
        let stat = create_test_stat("Food", 150.0, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::Overspending);
    }

    #[test]
    fn card_state_saving() {
        let stat = create_test_stat("Food", 80.0, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::Saving);
    }

    #[test]
    fn card_state_boundary_cases() {
        // The key insight: if percentage rounds to "5" or less, should be OnTrack
        // If it rounds to "6" or more, should be Overspending/Saving

        // 4.4% rounds to "4" → OnTrack
        let stat = create_test_stat("Food", 104.4, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::OnTrack);
        assert_eq!(format_percentage(stat.percentage_change), "4");

        // 4.6% rounds to "5" → OnTrack (edge case)
        let stat = create_test_stat("Food", 104.6, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::OnTrack);
        assert_eq!(format_percentage(stat.percentage_change), "5");

        // 5.4% rounds to "5" → OnTrack (edge case)
        let stat = create_test_stat("Food", 105.4, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::OnTrack);
        assert_eq!(format_percentage(stat.percentage_change), "5");

        // 5.5% rounds to "6" → Overspending
        let stat = create_test_stat("Food", 105.5, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::Overspending);
        assert_eq!(format_percentage(stat.percentage_change), "6");

        // 5.6% rounds to "6" → Overspending
        let stat = create_test_stat("Food", 105.6, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::Overspending);
        assert_eq!(format_percentage(stat.percentage_change), "6");
    }

    #[test]
    fn card_state_negative_boundary_cases() {
        // Same logic for savings

        // -4.4% rounds to "-4" → OnTrack
        let stat = create_test_stat("Food", 95.6, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::OnTrack);
        assert_eq!(format_percentage(stat.percentage_change), "-4");

        // -5.4% rounds to "-5" → OnTrack
        let stat = create_test_stat("Food", 94.6, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::OnTrack);
        assert_eq!(format_percentage(stat.percentage_change), "-5");

        // -5.5% rounds to "-6" → Saving
        let stat = create_test_stat("Food", 94.5, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::Saving);
        assert_eq!(format_percentage(stat.percentage_change), "-6");

        // -5.6% rounds to "-6" → Saving
        let stat = create_test_stat("Food", 94.4, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::Saving);
        assert_eq!(format_percentage(stat.percentage_change), "-6");
    }

    #[test]
    fn card_state_exactly_at_threshold() {
        // Exactly 5.0% → rounds to "5" → OnTrack
        let stat = create_test_stat("Food", 105.0, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::OnTrack);
        assert_eq!(format_percentage(stat.percentage_change), "5");

        // Exactly -5.0% → rounds to "-5" → OnTrack
        let stat = create_test_stat("Food", 95.0, 100.0, 5, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::OnTrack);
        assert_eq!(format_percentage(stat.percentage_change), "-5");
    }

    #[test]
    fn display_and_state_consistency() {
        // This test ensures we never show the same displayed percentage
        // with different card states

        let test_cases = vec![
            (104.0, 100.0), // 4%
            (104.5, 100.0), // 4.5%
            (105.0, 100.0), // 5%
            (105.5, 100.0), // 5.5%
            (106.0, 100.0), // 6%
            (95.5, 100.0),  // -4.5%
            (95.0, 100.0),  // -5%
            (94.5, 100.0),  // -5.5%
        ];

        for (current, average) in test_cases {
            let stat = create_test_stat("Food", current, average, 5, 50.0);
            let state = determine_card_state(&stat);
            let displayed = format_percentage(stat.percentage_change);

            // Rule: If display shows "5" or less (absolute), must be OnTrack
            let abs_displayed: i32 = displayed.trim_start_matches('-').parse().unwrap();

            if abs_displayed <= 5 {
                assert_eq!(
                    state,
                    CardState::OnTrack,
                    "Displayed '{}%' (from {:.2}%) should be OnTrack, got {:?}",
                    displayed,
                    stat.percentage_change,
                    state
                );
            } else {
                assert_ne!(
                    state,
                    CardState::OnTrack,
                    "Displayed '{}%' (from {:.2}%) should NOT be OnTrack, got {:?}",
                    displayed,
                    stat.percentage_change,
                    state
                );
            }
        }
    }

    #[test]
    fn format_percentage_avoids_negative_zero() {
        assert_eq!(format_percentage(0.0), "0");
        assert_eq!(format_percentage(-0.0), "0");
        assert_eq!(format_percentage(-0.4), "0");
        assert_eq!(format_percentage(0.4), "0");
        assert_eq!(format_percentage(5.0), "5");
        assert_eq!(format_percentage(-5.0), "-5");
    }

    #[test]
    fn renders_empty_state_when_no_tags() {
        let stats: Vec<TagExpenseStats> = vec![];
        let html = expense_cards_view(&stats).into_string();

        assert!(html.contains("Get Started"));
        assert!(html.contains("Manage Tags"));
    }

    #[test]
    fn renders_helper_card_when_few_tags() {
        let stats = vec![create_test_stat("Food", 100.0, 100.0, 5, 100.0)];
        let html = expense_cards_view(&stats).into_string();

        assert!(html.contains("Tip"));
        assert!(html.contains("Add more tags"));
    }

    #[test]
    fn does_not_render_helper_card_when_many_tags() {
        let stats = vec![
            create_test_stat("Food", 100.0, 100.0, 5, 30.0),
            create_test_stat("Transport", 50.0, 50.0, 5, 20.0),
            create_test_stat("Utilities", 40.0, 40.0, 5, 15.0),
        ];
        let html = expense_cards_view(&stats).into_string();

        assert!(!html.contains("Add more tags"));
    }

    #[test]
    fn tag_name_displays_with_emoji() {
        let stat = create_test_stat("🍔 Food", 100.0, 100.0, 5, 50.0);
        let html = expense_card(&stat).into_string();

        assert!(html.contains("🍔 Food"));
    }

    #[test]
    fn progress_bar_has_minimum_width_for_small_percentages() {
        let html = progress_bar(0.5).into_string();
        // Should render with 3% width (minimum for rounded corners to show)
        assert!(html.contains("width: 3.0%"));
    }

    #[test]
    fn progress_bar_empty_for_zero_percentage() {
        let html = progress_bar(0.0).into_string();
        // Should have the container but no inner bar
        assert!(html.contains("progressbar"));
        assert!(!html.contains("bg-blue-600"));
    }

    #[test]
    fn progress_bar_clamps_negative_values() {
        let html = progress_bar(-5.0).into_string();
        // Should render as 0 (no bar)
        assert!(html.contains("progressbar"));
        assert!(html.contains("aria-valuenow=\"0\""));
        assert!(!html.contains("bg-blue-600"));
    }

    #[test]
    fn progress_bar_clamps_over_100() {
        let html = progress_bar(150.0).into_string();
        // Should clamp to 100%
        assert!(html.contains("width: 100.0%"));
    }
}
