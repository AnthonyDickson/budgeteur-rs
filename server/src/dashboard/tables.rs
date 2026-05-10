//! Table views for dashboard data display.
//!
//! Provides HTML table components for displaying monthly financial summaries.

use maud::{Markup, PreEscaped, html};
use time::{Date, format_description::BorrowedFormatItem, macros::format_description};

use crate::{
    dashboard::{
        aggregation::{
            aggregate_by_month, calculate_monthly_breakdown, calculate_running_balances,
            calculate_summary_statistics, format_month_labels,
        },
        transaction::Transaction,
    },
    html::{TABLE_CELL_STYLE, TABLE_ROW_STYLE, currency_rounded_with_tooltip},
};

// Table cell styles for monthly summary
const TABLE_HEADER_CELL_STYLE: &str = "px-6 py-3 min-w-[100px] font-semibold";
const TABLE_HEADER_FIRST_CELL_STYLE: &str =
    "px-3 py-3 sticky left-0 bg-gray-100 dark:bg-gray-700 z-10 font-semibold";
const TABLE_STICKY_CELL_STYLE: &str = "px-6 py-3 text-left font-medium text-gray-900 dark:text-white sticky left-0 bg-white dark:bg-gray-800 z-10";
const TABLE_DATA_CELL_STYLE: &str = "whitespace-nowrap font-mono";
const TABLE_CELL_GREEN_STYLE: &str = "text-green-600 dark:text-green-400";
const TABLE_CELL_RED_STYLE: &str = "text-red-600 dark:text-red-400";

/// Gets the CSS class for coloring amounts (green for positive, red for negative).
fn amount_color_class(amount: f64) -> &'static str {
    if amount >= 0.0 {
        TABLE_CELL_GREEN_STYLE
    } else {
        TABLE_CELL_RED_STYLE
    }
}

/// Renders a table showing summary statistics (weekly avg, monthly avg, totals).
///
/// # Arguments
/// * `transactions` - All transactions to analyze (should span the last year)
/// * `total_account_balance` - Current total balance across all accounts
///
/// # Returns
/// Maud markup containing a table with summary statistics.
pub(super) fn summary_statistics_table(
    transactions: &[Transaction],
    total_account_balance: f64,
) -> Markup {
    let monthly_totals = aggregate_by_month(transactions);

    if monthly_totals.is_empty() {
        return html! {};
    }

    let breakdown = calculate_monthly_breakdown(transactions);
    let stats = calculate_summary_statistics(&breakdown);

    html! {
        div
        {
            h3 class="text-xl font-semibold mb-4" { "Summary Statistics" }

            div class="overflow-x-auto rounded shadow" {
                table class="w-full text-sm text-right text-gray-500 dark:text-gray-400"
                {
                    thead class="text-xs text-gray-900 uppercase bg-gray-100 dark:bg-gray-700 dark:text-gray-400"
                    {
                        tr {
                            th scope="col" class=(TABLE_HEADER_FIRST_CELL_STYLE) { "" }
                            th scope="col" class=(TABLE_HEADER_CELL_STYLE) { "Weekly Avg" }
                            th scope="col" class=(TABLE_HEADER_CELL_STYLE) { "Monthly Avg" }
                            th scope="col" class=(TABLE_HEADER_CELL_STYLE) { "Total" }
                        }
                    }

                    tbody {
                        // Income row
                        tr class=(TABLE_ROW_STYLE)
                        {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) { "Income" }

                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_GREEN_STYLE)}
                            {
                                (currency_rounded_with_tooltip(stats.weekly_avg_income))
                            }

                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_GREEN_STYLE)}
                            {
                                (currency_rounded_with_tooltip(stats.monthly_avg_income))
                            }

                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_GREEN_STYLE)}
                            {
                                (currency_rounded_with_tooltip(stats.total_income))
                            }
                        }

                        // Expenses row
                        tr class=(TABLE_ROW_STYLE)
                        {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) { "Expenses" }

                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_RED_STYLE)}
                            {
                                (currency_rounded_with_tooltip(stats.weekly_avg_expenses))
                            }

                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_RED_STYLE)}
                            {
                                (currency_rounded_with_tooltip(stats.monthly_avg_expenses))
                            }

                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_RED_STYLE)}
                            {
                                (currency_rounded_with_tooltip(stats.total_expenses))
                            }
                        }

                        // Net Income row
                        tr class=(TABLE_ROW_STYLE)
                        {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) { "Net Income" }

                            td class=(TABLE_CELL_STYLE)
                            {
                                div class={(TABLE_DATA_CELL_STYLE) " " (amount_color_class(stats.weekly_avg_net_income))}
                                {
                                    (currency_rounded_with_tooltip(stats.weekly_avg_net_income))
                                }
                            }

                            td class=(TABLE_CELL_STYLE)
                            {
                                div class={(TABLE_DATA_CELL_STYLE) " " (amount_color_class(stats.monthly_avg_net_income))}
                                {
                                    (currency_rounded_with_tooltip(stats.monthly_avg_net_income))
                                }
                            }

                            td class=(TABLE_CELL_STYLE)
                            {
                                div class={(TABLE_DATA_CELL_STYLE) " " (amount_color_class(stats.total_net_income))}
                                {
                                    (currency_rounded_with_tooltip(stats.total_net_income))
                                }
                            }
                        }

                        // Balance row
                        tr class=(TABLE_ROW_STYLE)
                        {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) { "Balance" }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE)} { "—" }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE)} { "—" }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " font-bold"}
                            {
                                (currency_rounded_with_tooltip(total_account_balance))
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Renders a table showing income, expenses, net income, and balance for each month.
///
/// # Arguments
/// * `transactions` - All transactions to analyze (should span the last year)
/// * `total_account_balance` - Current total balance across all accounts
///
/// # Returns
/// Maud markup containing a responsive table with monthly financial summaries.
pub(super) fn monthly_summary_table(
    transactions: &[Transaction],
    total_account_balance: f64,
) -> Markup {
    let monthly_totals = aggregate_by_month(transactions);

    if monthly_totals.is_empty() {
        return html! {};
    }

    let sorted_months: Vec<Date> = monthly_totals.keys().copied().collect();

    let labels = format_month_labels(&sorted_months);
    let (_, balances) = calculate_running_balances(total_account_balance, &monthly_totals);
    let breakdown = calculate_monthly_breakdown(transactions);

    html! {
        div
        {
            h3 class="text-xl font-semibold mb-4" { "Monthly Summary" }

            div
                id="monthly-summary-table"
                class="overflow-x-auto rounded shadow"
            {
                table class="w-full text-right text-sm text-gray-500 dark:text-gray-400"
                {
                    thead class="text-xs text-gray-900 uppercase bg-gray-100 dark:bg-gray-700 dark:text-gray-400"
                    {
                        tr
                        {
                            th scope="col" class=(TABLE_HEADER_FIRST_CELL_STYLE) { "" }

                            @for (month, label) in sorted_months.iter().zip(labels.iter()) {
                                th scope="col" class={(TABLE_HEADER_CELL_STYLE) " font-semibold"} {
                                    time datetime=(month_datetime_attr(*month)) { (label) }
                                }
                            }
                        }
                    }

                    tbody
                    {
                        // Income row
                        tr class=(TABLE_ROW_STYLE)
                        {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) { "Income" }

                            @for month in &sorted_months {
                                td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_GREEN_STYLE)}
                                {
                                    (currency_rounded_with_tooltip(breakdown.income_for_month(month)))
                                }
                            }
                        }

                        // Expenses row
                        tr class=(TABLE_ROW_STYLE)
                        {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) { "Expenses" }

                            @for month in &sorted_months {
                                td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_RED_STYLE)}
                                {
                                    (currency_rounded_with_tooltip(breakdown.expenses_for_month(month)))
                                }
                            }
                        }

                        // Net Income row
                        tr class=(TABLE_ROW_STYLE)
                        {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE)
                            {
                                "Net Income"
                            }

                            @for month in &sorted_months {
                                td class=(TABLE_CELL_STYLE)
                                {
                                    @let net = monthly_totals.get(month).copied().unwrap_or(0.0);

                                    div class={(TABLE_DATA_CELL_STYLE) " " (amount_color_class(net))}
                                    {
                                        (currency_rounded_with_tooltip(net))
                                    }
                                }
                            }
                        }

                        // Balance row
                        tr class=(TABLE_ROW_STYLE)
                        {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) { "Balance" }

                            @for (i, _month) in sorted_months.iter().enumerate() {
                                td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " font-semibold"}
                                {
                                    (currency_rounded_with_tooltip(balances[i]))
                                }
                            }
                        }
                    }
                }
            }

            // Scroll to right-most column on load, inline so it is triggered on HTMX swap
            // Fixes flickering sticky column bug with iOS Safari by avoiding
            // dir="rtl" on the container and dir="ltr" on the table to
            // automatically scroll to the right-most column.
            script {
                (PreEscaped(r#"
                (function() {
                    const container = document.getElementById('monthly-summary-table');
                    if (container) {
                        // Use requestAnimationFrame to ensure DOM is ready
                        requestAnimationFrame(() => {
                            container.scrollLeft = container.scrollWidth - container.clientWidth;
                        });
                    }
                })();
                "#))
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
