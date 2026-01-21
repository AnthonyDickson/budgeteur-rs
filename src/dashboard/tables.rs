//! Table views for dashboard data display.
//!
//! Provides HTML table components for displaying monthly financial summaries.

use maud::{Markup, html};
use time::Date;

use crate::{
    dashboard::{
        aggregation::{
            aggregate_by_month, calculate_monthly_breakdown, calculate_running_balances,
            calculate_summary_statistics, format_month_labels,
        },
        transaction::Transaction,
    },
    html::{TABLE_CELL_STYLE, TABLE_ROW_STYLE, format_currency},
};

// Table cell styles for monthly summary
const TABLE_HEADER_CELL_STYLE: &str = "px-3 py-3 text-center min-w-[100px]";
const TABLE_HEADER_FIRST_CELL_STYLE: &str =
    "px-3 py-3 sticky left-0 bg-gray-100 dark:bg-gray-700 z-10 font-semibold";
const TABLE_STICKY_CELL_STYLE: &str = "px-3 py-4 font-medium text-gray-900 dark:text-white sticky left-0 bg-white dark:bg-gray-800 z-10";
const TABLE_DATA_CELL_STYLE: &str = "text-center whitespace-nowrap";
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
        div {
            h3 class="text-xl font-semibold mb-4" { "Summary Statistics" }

            div class="overflow-x-auto rounded-lg shadow" {
                table class="w-full text-sm text-left text-gray-500 dark:text-gray-400" {
                    thead class="text-xs text-gray-900 uppercase bg-gray-100 dark:bg-gray-700 dark:text-gray-400" {
                        tr {
                            th scope="col" class={(TABLE_HEADER_FIRST_CELL_STYLE) " text-left"} {
                                ""
                            }
                            th scope="col" class={(TABLE_HEADER_CELL_STYLE) " font-semibold"} {
                                "Weekly Avg"
                            }
                            th scope="col" class={(TABLE_HEADER_CELL_STYLE) " font-semibold"} {
                                "Monthly Avg"
                            }
                            th scope="col" class={(TABLE_HEADER_CELL_STYLE) " font-bold"} {
                                "Total"
                            }
                        }
                    }
                    tbody {
                        // Income row
                        tr class=(TABLE_ROW_STYLE) {
                            th scope="row" class={(TABLE_STICKY_CELL_STYLE) " text-left"} {
                                "Income"
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_GREEN_STYLE)} {
                                (format_currency(stats.weekly_avg_income))
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_GREEN_STYLE)} {
                                (format_currency(stats.monthly_avg_income))
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_GREEN_STYLE) " font-bold"} {
                                (format_currency(stats.total_income))
                            }
                        }

                        // Expenses row
                        tr class=(TABLE_ROW_STYLE) {
                            th scope="row" class={(TABLE_STICKY_CELL_STYLE) " text-left"} {
                                "Expenses"
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_RED_STYLE)} {
                                (format_currency(stats.weekly_avg_expenses))
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_RED_STYLE)} {
                                (format_currency(stats.monthly_avg_expenses))
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_RED_STYLE) " font-bold"} {
                                (format_currency(stats.total_expenses))
                            }
                        }

                        // Net Income row
                        tr class=(TABLE_ROW_STYLE) {
                            th scope="row" class={(TABLE_STICKY_CELL_STYLE) " text-left"} {
                                "Net Income"
                            }
                            td class=(TABLE_CELL_STYLE) {
                                div class={(TABLE_DATA_CELL_STYLE) " " (amount_color_class(stats.weekly_avg_net_income))} {
                                    (format_currency(stats.weekly_avg_net_income))
                                }
                            }
                            td class=(TABLE_CELL_STYLE) {
                                div class={(TABLE_DATA_CELL_STYLE) " " (amount_color_class(stats.monthly_avg_net_income))} {
                                    (format_currency(stats.monthly_avg_net_income))
                                }
                            }
                            td class=(TABLE_CELL_STYLE) {
                                div class={(TABLE_DATA_CELL_STYLE) " " (amount_color_class(stats.total_net_income)) " font-bold"} {
                                    (format_currency(stats.total_net_income))
                                }
                            }
                        }

                        // Balance row
                        tr class=(TABLE_ROW_STYLE) {
                            th scope="row" class={(TABLE_STICKY_CELL_STYLE) " text-left"} {
                                "Balance"
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE)} {
                                "—"
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE)} {
                                "—"
                            }
                            td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " font-bold"} {
                                (format_currency(total_account_balance))
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

    let mut sorted_months: Vec<Date> = monthly_totals.keys().copied().collect();
    sorted_months.sort_unstable();

    let labels = format_month_labels(&sorted_months);
    let (_, balances) = calculate_running_balances(total_account_balance, &monthly_totals);
    let breakdown = calculate_monthly_breakdown(transactions);

    html! {
        div {
            h3 class="text-xl font-semibold mb-4" { "Monthly Summary" }

            div
                id="monthly-summary-table"
                class="overflow-x-auto rounded-lg shadow"
                dir="rtl"
            {
                table class="w-full text-sm text-left text-gray-500 dark:text-gray-400" dir="ltr" {
                    thead class="text-xs text-gray-900 uppercase bg-gray-100 dark:bg-gray-700 dark:text-gray-400" {
                        tr {
                            th scope="col" class=(TABLE_HEADER_FIRST_CELL_STYLE) {
                                ""
                            }
                            @for label in &labels {
                                th scope="col" class={(TABLE_HEADER_CELL_STYLE) " font-semibold"} {
                                    (label)
                                }
                            }
                        }
                    }
                    tbody {
                        // Income row
                        tr class=(TABLE_ROW_STYLE) {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) {
                                "Income"
                            }
                            @for month in &sorted_months {
                                td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_GREEN_STYLE)} {
                                    (format_currency(breakdown.income_for_month(month)))
                                }
                            }
                        }

                        // Expenses row
                        tr class=(TABLE_ROW_STYLE) {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) {
                                "Expenses"
                            }
                            @for month in &sorted_months {
                                td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " " (TABLE_CELL_RED_STYLE)} {
                                    (format_currency(breakdown.expenses_for_month(month)))
                                }
                            }
                        }

                        // Net Income row
                        tr class=(TABLE_ROW_STYLE) {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) {
                                "Net Income"
                            }
                            @for month in &sorted_months {
                                td class=(TABLE_CELL_STYLE) {
                                    @let net = monthly_totals.get(month).copied().unwrap_or(0.0);
                                    div class={(TABLE_DATA_CELL_STYLE) " " (amount_color_class(net))} {
                                        (format_currency(net))
                                    }
                                }
                            }
                        }

                        // Balance row
                        tr class=(TABLE_ROW_STYLE) {
                            th scope="row" class=(TABLE_STICKY_CELL_STYLE) {
                                "Balance"
                            }
                            @for (i, _month) in sorted_months.iter().enumerate() {
                                td class={(TABLE_CELL_STYLE) " " (TABLE_DATA_CELL_STYLE) " font-semibold"} {
                                    (format_currency(balances[i]))
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
