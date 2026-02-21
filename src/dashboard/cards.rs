//! Card components for expense breakdown by tag.
//!
//! See `expenses-by-tag-design-spec.md` for UI specifications.

use maud::{Markup, html};
use time::{Date, Month, format_description::BorrowedFormatItem, macros::format_description};

use crate::{
    dashboard::aggregation::TagExpenseStats,
    endpoints,
    html::{LINK_STYLE, currency_rounded_with_tooltip, format_currency},
};

/// Uses 5.5% (not 5.0%) to align with percentage rounding:
/// - 5.4% rounds to "5" → OnTrack
/// - 5.5% rounds to "6" → Overspending/Saving
///
/// This ensures displayed value matches card state.
/// See expenses-by-tag-tech-spec.md for full explanation.
const DISPLAY_THRESHOLD: f64 = 5.5;
const MINIMUM_MONTHS_OF_DATA: usize = 2;
/// Used to ensure a minimum width so rounded corners are visible
const PROGRESS_BAR_MIN_PERCENTAGE: f64 = 3.0;

/// The state of a tag's spending relative to historical patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CardState {
    /// Spending is ≥5% above average
    Overspending,
    /// Spending is ≥5% below average
    Saving,
    /// Spending is within 5% of average
    OnTrack,
    /// Less than 2 months of data available
    InsufficientData,
}

/// Determines the card state based on spending patterns.
///
/// Uses 5.5% threshold (not 5.0%) to align with percentage rounding.
/// See technical spec for threshold alignment explanation.
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

/// Renders the expense cards section.
///
/// Shows empty state if no tags, helper card if ≤2 tags.
pub(super) fn expense_cards_view(tag_stats: &[TagExpenseStats], displayed_month: Date) -> Markup {
    if tag_stats.is_empty() {
        return empty_state_view();
    }

    let datetime = month_datetime_attr(displayed_month);
    let displayed_month_label = format_month_year_label(displayed_month);

    html! {
        section class="w-full mx-auto mt-8 mb-8" {
            // Section header
            div class="flex justify-between items-baseline mb-4" {
                h3 class="text-xl font-semibold" {
                    "Expenses by Tag"
                }
                span class="text-sm text-gray-600 dark:text-gray-400" {
                    time datetime=(datetime) {
                        (displayed_month_label)
                    }
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

const MONTH_ATTRIBUTE_FORMAT: &[BorrowedFormatItem] =
    format_description!("[year]-[month repr:numerical padding:zero]");

fn month_datetime_attr(date: Date) -> String {
    date.format(MONTH_ATTRIBUTE_FORMAT)
        .unwrap_or_else(|_| date.to_string())
}

fn format_month_year_label(date: Date) -> String {
    let month_name = match date.month() {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    };
    format!("{} {}", month_name, date.year())
}

/// Renders a single expense card for a tag.
fn expense_card(stat: &TagExpenseStats) -> Markup {
    let state = determine_card_state(stat);

    html! {
        div
            class="bg-white dark:bg-gray-800 border border-gray-200
                   dark:border-gray-700 rounded p-4 shadow-md 
                   hover:shadow-lg transition-shadow min-h-[200px] 
                   flex flex-col justify-between"
            aria-label=(create_card_aria_label(stat, state))
        {
            div {
                // Tag name
                h4 class="text-lg font-semibold mb-3 truncate"
                   title=(stat.tag) {
                    (stat.tag)
                }

                // Current amount
                div class="text-3xl font-bold mb-1" {
                    (currency_rounded_with_tooltip(stat.last_month_amount))
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
                        "Avg: " (currency_rounded_with_tooltip(stat.monthly_average)) "/month"
                    }
                    div class="text-sm text-gray-600 dark:text-gray-400" {
                        "→ On track"
                    }
                }
                CardState::Overspending => {
                    div class="text-sm" {
                        "Avg: " (currency_rounded_with_tooltip(stat.monthly_average)) "/month"
                    }
                    div class="text-sm font-medium text-red-600 dark:text-red-400" {
                        "↑ +" (format_percentage(stat.percentage_change)) "% above usual"
                    }
                    div class="text-sm font-semibold text-red-600 dark:text-red-400" {
                        "💡 +" (currency_rounded_with_tooltip(stat.annual_delta)) "/year"
                    }
                }
                CardState::Saving => {
                    div class="text-sm" {
                        "Avg: " (currency_rounded_with_tooltip(stat.monthly_average)) "/month"
                    }
                    div class="text-sm font-medium text-green-600 dark:text-green-400" {
                        "↓ -" (format_percentage(stat.percentage_change.abs())) "% below usual"
                    }
                    div class="text-sm font-semibold text-green-600 dark:text-green-400" {
                        "💡 -" (currency_rounded_with_tooltip(stat.annual_delta.abs())) "/year 🎉"
                    }
                }
            }
        }
    }
}

/// Renders a horizontal progress bar showing percentage of total expenses.
fn progress_bar(percentage: f64) -> Markup {
    let clamped = percentage.clamp(0.0, 100.0);

    let display_percentage = if clamped > 0.0 && clamped < PROGRESS_BAR_MIN_PERCENTAGE {
        PROGRESS_BAR_MIN_PERCENTAGE
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

fn create_card_aria_label(stat: &TagExpenseStats, state: CardState) -> String {
    let tag = &stat.tag;
    let amount = format_currency(stat.last_month_amount);
    let average = format_currency(stat.monthly_average);

    let change_description = if stat.percentage_change >= 0.0 {
        format!("up {}%", format_percentage(stat.percentage_change))
    } else {
        format!("down {}%", format_percentage(stat.percentage_change.abs()))
    };

    let impact_description = match state {
        CardState::InsufficientData => ", building baseline".to_string(),
        CardState::OnTrack => String::new(),
        CardState::Overspending | CardState::Saving => {
            let delta = format_currency(stat.annual_delta.abs());
            let impact_type = if stat.annual_delta >= 0.0 {
                "more spending"
            } else {
                "savings"
            };
            format!(", resulting in {} {} per year", delta, impact_type)
        }
    };

    format!(
        "{} expenses: {} last month, {} from usual {} average{}",
        tag, amount, change_description, average, impact_description
    )
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

    use scraper::{Html, Selector};
    use time::macros::date;

    use super::*;

    fn create_test_stat(
        tag: &str,
        last_month: f64,
        average: f64,
        months: usize,
        percentage_of_total: f64,
    ) -> TagExpenseStats {
        let percentage_change = if average > 0.0 {
            ((last_month - average) / average) * 100.0
        } else {
            0.0
        };

        TagExpenseStats {
            tag: tag.to_owned(),
            last_month_amount: last_month,
            percentage_of_total,
            monthly_average: average,
            percentage_change,
            annual_delta: (last_month - average) * 12.0,
            months_of_data: months,
        }
    }

    #[test]
    fn card_state_insufficient_data() {
        // Zero months
        let stat = create_test_stat("Food", 100.0, 100.0, 0, 50.0);
        assert_eq!(determine_card_state(&stat), CardState::InsufficientData);

        // One month
        let stat = create_test_stat("Food", 100.0, 100.0, 1, 50.0);
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
        let test_cases = vec![
            (104.0, 100.0, "4", CardState::OnTrack),
            (104.5, 100.0, "5", CardState::OnTrack), // Edge: rounds to 5
            (105.0, 100.0, "5", CardState::OnTrack),
            (105.5, 100.0, "6", CardState::Overspending), // Boundary
            (106.0, 100.0, "6", CardState::Overspending),
            (94.5, 100.0, "-6", CardState::Saving),
            (95.0, 100.0, "-5", CardState::OnTrack),
            (95.5, 100.0, "-5", CardState::OnTrack), // Edge: rounds to -5
        ];

        for (last_month, average, expected_display, expected_state) in test_cases {
            let stat = create_test_stat("Food", last_month, average, 5, 50.0);
            let state = determine_card_state(&stat);
            let displayed = format_percentage(stat.percentage_change);

            assert_eq!(
                displayed, expected_display,
                "Display formatting failed for {:.1}%",
                stat.percentage_change
            );
            assert_eq!(
                state, expected_state,
                "State mismatch for displayed '{}%' (actual {:.2}%)",
                displayed, stat.percentage_change
            );
        }
    }

    #[test]
    fn format_percentage_avoids_negative_zero() {
        // Zero should always display as "0"
        assert_eq!(format_percentage(0.0), "0");
        assert_eq!(format_percentage(-0.0), "0");

        // Small values round to zero
        assert_eq!(format_percentage(0.4), "0");
        assert_eq!(format_percentage(-0.4), "0");

        // Normal rounding
        assert_eq!(format_percentage(5.0), "5");
        assert_eq!(format_percentage(-5.0), "-5");
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

    fn parse_html(markup: &Markup) -> Html {
        Html::parse_fragment(&markup.clone().into_string())
    }

    #[test]
    fn renders_empty_state_when_no_tags() {
        let stats: Vec<TagExpenseStats> = vec![];
        let html = parse_html(&expense_cards_view(&stats, date!(2024 - 12 - 01)));

        // Check for semantic structure
        let h3_selector = Selector::parse("h3").unwrap();
        let h3_text = html
            .select(&h3_selector)
            .next()
            .unwrap()
            .text()
            .collect::<String>();
        assert_eq!(h3_text.trim(), "Get Started");

        // Check for link to tags
        let link_selector = Selector::parse("a[href='/tags']").unwrap();
        assert!(
            html.select(&link_selector).next().is_some(),
            "Should have link to tags page"
        );
    }

    #[test]
    fn renders_helper_card_when_few_tags() {
        let stats = vec![create_test_stat("Food", 100.0, 100.0, 5, 100.0)];
        let html = parse_html(&expense_cards_view(&stats, date!(2024 - 12 - 01)));

        // Check for tip card by class
        let tip_selector = Selector::parse(".bg-blue-50").unwrap();
        assert!(
            html.select(&tip_selector).next().is_some(),
            "Should render helper card with blue background"
        );

        // Check for heading
        let h4_selector = Selector::parse("h4").unwrap();
        let h4_texts: Vec<_> = html
            .select(&h4_selector)
            .map(|el| el.text().collect::<String>())
            .collect();
        assert!(
            h4_texts.iter().any(|t| t.contains("Tip")),
            "Should have 'Tip' heading"
        );
    }

    #[test]
    fn does_not_render_helper_card_when_many_tags() {
        let stats = vec![
            create_test_stat("Food", 100.0, 100.0, 5, 30.0),
            create_test_stat("Transport", 50.0, 50.0, 5, 20.0),
            create_test_stat("Utilities", 40.0, 40.0, 5, 15.0),
        ];
        let html = parse_html(&expense_cards_view(&stats, date!(2024 - 12 - 01)));

        // Should have 3 expense cards
        let card_selector = Selector::parse(".bg-white.dark\\:bg-gray-800").unwrap();
        let card_count = html.select(&card_selector).count();
        assert_eq!(card_count, 3, "Should have exactly 3 expense cards");

        // Should NOT have helper card
        let tip_selector = Selector::parse(".bg-blue-50").unwrap();
        assert_eq!(
            html.select(&tip_selector).count(),
            0,
            "Should not have helper card"
        );
    }

    #[test]
    fn tag_name_displays_with_emoji() {
        let stat = create_test_stat("🍔 Food", 100.0, 100.0, 5, 50.0);
        let html = parse_html(&expense_card(&stat));

        let h4_selector = Selector::parse("h4").unwrap();
        let h4_text = html
            .select(&h4_selector)
            .next()
            .unwrap()
            .text()
            .collect::<String>();
        assert_eq!(h4_text.trim(), "🍔 Food");
    }

    #[test]
    fn displays_month_label_correctly() {
        let stats = vec![create_test_stat("Food", 100.0, 100.0, 5, 100.0)];
        let html = parse_html(&expense_cards_view(&stats, date!(2024 - 12 - 01)));

        // Check for month label in header
        let header_selector = Selector::parse(".text-sm.text-gray-600").unwrap();
        let month_label = html
            .select(&header_selector)
            .next()
            .unwrap()
            .text()
            .collect::<String>();
        assert_eq!(month_label.trim(), "December 2024");
    }

    #[test]
    fn progress_bar_has_correct_structure() {
        let html = parse_html(&progress_bar(50.0));

        // Check for progressbar role
        let bar_selector = Selector::parse("[role='progressbar']").unwrap();
        let bar = html
            .select(&bar_selector)
            .next()
            .expect("Should have progressbar role");

        // Check aria attributes
        let aria_valuenow = bar.value().attr("aria-valuenow").unwrap();
        assert_eq!(aria_valuenow, "50");

        let aria_valuemin = bar.value().attr("aria-valuemin").unwrap();
        assert_eq!(aria_valuemin, "0");

        let aria_valuemax = bar.value().attr("aria-valuemax").unwrap();
        assert_eq!(aria_valuemax, "100");
    }

    #[test]
    fn expense_card_has_proper_aria_label() {
        let stat = create_test_stat("Food", 150.0, 100.0, 5, 60.0);
        let html = parse_html(&expense_card(&stat));

        let card_selector = Selector::parse("[aria-label]").unwrap();
        let card = html
            .select(&card_selector)
            .next()
            .expect("Card should have aria-label");

        let aria_label = card.value().attr("aria-label").unwrap();
        assert!(aria_label.contains("Food expenses"));
        assert!(aria_label.contains("$150.00"));
        assert!(aria_label.contains("up 50%"));
    }

    #[test]
    fn card_displays_overspending_state_correctly() {
        let stat = create_test_stat("Food", 150.0, 100.0, 5, 60.0);
        let html = parse_html(&expense_card(&stat));

        // Check for red text (overspending indicator)
        let red_selector = Selector::parse(".text-red-600").unwrap();
        let red_elements: Vec<_> = html.select(&red_selector).collect();
        assert!(
            red_elements.len() >= 2,
            "Should have at least 2 red elements (trend and delta)"
        );

        // Check for up arrow
        let trend_text: String = html.select(&red_selector).next().unwrap().text().collect();
        assert!(trend_text.contains("↑") || trend_text.contains("above usual"));
    }

    #[test]
    fn card_displays_saving_state_correctly() {
        let stat = create_test_stat("Food", 80.0, 100.0, 5, 60.0);
        let html = parse_html(&expense_card(&stat));

        // Check for green text (saving indicator)
        let green_selector = Selector::parse(".text-green-600").unwrap();
        let green_elements: Vec<_> = html.select(&green_selector).collect();
        assert!(
            green_elements.len() >= 2,
            "Should have at least 2 green elements"
        );

        // Check for celebration emoji
        let text: String = html.html();
        assert!(text.contains("🎉"), "Should have celebration emoji");
    }

    #[test]
    fn card_grid_has_responsive_classes() {
        let stats = vec![create_test_stat("Food", 100.0, 100.0, 5, 100.0)];
        let html = parse_html(&expense_cards_view(&stats, date!(2024 - 12 - 01)));

        let grid_selector = Selector::parse(".grid").unwrap();
        let grid = html
            .select(&grid_selector)
            .next()
            .expect("Should have grid container");

        let classes = grid.value().attr("class").unwrap();
        assert!(classes.contains("grid-cols-1"), "Should have mobile layout");
        assert!(
            classes.contains("sm:grid-cols-2"),
            "Should have small breakpoint"
        );
        assert!(
            classes.contains("md:grid-cols-3"),
            "Should have medium breakpoint"
        );
        assert!(
            classes.contains("lg:grid-cols-4"),
            "Should have large breakpoint"
        );
    }
}
