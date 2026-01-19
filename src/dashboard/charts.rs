//! Chart generation and rendering for the dashboard.
//!
//! This module creates interactive ECharts visualizations for financial data:
//! - **Net Income Chart**: Monthly income/expense totals over the last year
//! - **Net Balance Chart**: Running account balance over time
//! - **Monthly Expenses Chart**: Stacked bar chart of expenses grouped by tag
//!
//! Each chart is generated as JSON configuration for the ECharts library and
//! rendered with corresponding HTML containers and JavaScript initialization code.

use charming::{
    Chart,
    component::{Axis, Grid, Legend, Title, VisualMap, VisualMapPiece},
    element::{
        AxisLabel, AxisPointer, AxisPointerType, AxisType, Emphasis, EmphasisFocus, JsFunction,
        Tooltip, Trigger,
    },
    series::{Line, bar},
};
use maud::{Markup, PreEscaped, html};

use crate::{
    dashboard::{
        aggregation::{
            aggregate_by_month, calculate_running_balances, format_month_labels,
            get_monthly_label_and_value_pairs, get_sorted_months, group_monthly_expenses_by_tag,
        },
        transaction::Transaction,
    },
    html::HeadElement,
};

/// A dashboard chart with its HTML container ID and ECharts configuration.
pub(super) struct DashboardChart {
    /// The HTML element ID to use for the chart (kebab-case)
    pub id: &'static str,
    /// The ECharts configuration as a JSON string
    pub options: String,
}

/// Renders the HTML containers for dashboard charts.
///
/// # Arguments
/// * `charts` - The charts to render containers for
///
/// # Returns
/// Maud markup containing a grid of chart container divs.
pub(super) fn charts_view(charts: &[DashboardChart]) -> Markup {
    html!(
        section
            id="charts"
            class="w-full mx-auto mb-4"
        {
            div class="grid grid-cols-1 xl:grid-cols-2 gap-4"
            {
                @for chart in charts {
                    div
                        id=(chart.id)
                        class="min-h-[380px] rounded dark:bg-gray-100"
                    {}
                }
            }
        }
    )
}

/// Generates JavaScript initialization code for dashboard charts.
///
/// Creates scripts that initialize ECharts instances with dark mode support
/// and responsive resizing.
///
/// # Arguments
/// * `charts` - The charts to generate initialization scripts for
///
/// # Returns
/// HeadElement containing the initialization JavaScript.
pub(super) fn charts_script(charts: &[DashboardChart]) -> HeadElement {
    let script_content = charts
        .iter()
        .map(|chart| {
            format!(
                r#"(function() {{
                    const chartDom = document.getElementById("{}");
                    const chart = echarts.init(chartDom);
                    const option = {};
                    chart.setOption(option);

                    window.addEventListener('resize', chart.resize);

                    const darkModeMediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
                    const updateTheme = () => {{
                        const isDarkMode = darkModeMediaQuery.matches;
                        chart.setTheme(isDarkMode ? 'dark' : 'default');
                    }}
                    darkModeMediaQuery.addEventListener('change', updateTheme);
                    updateTheme();
                }})();"#,
                chart.id, chart.options
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let wrapped_script = format!(
        "document.addEventListener('DOMContentLoaded', function() {{\n{}\n}});",
        script_content
    );

    HeadElement::ScriptSource(PreEscaped(wrapped_script))
}

pub(super) fn net_income_chart(transactions: &[Transaction]) -> Chart {
    let monthly_totals = aggregate_by_month(transactions);
    let (labels, values) = get_monthly_label_and_value_pairs(&monthly_totals);

    Chart::new()
        .title(
            Title::new()
                .text("Net income")
                .subtext("Last twelve months"),
        )
        .tooltip(currency_tooltip())
        .grid(
            Grid::new()
                .left("3%")
                .right("4%")
                .bottom("3%")
                .contain_label(true),
        )
        .x_axis(Axis::new().type_(AxisType::Category).data(labels))
        .y_axis(
            Axis::new()
                .type_(AxisType::Value)
                .axis_label(AxisLabel::new().formatter(currency_formatter())),
        )
        .visual_map(VisualMap::new().show(false).pieces(vec![
            VisualMapPiece::new().lte(-1).color("red"),
            VisualMapPiece::new().gte(0).color("green"),
        ]))
        .series(Line::new().name("Net Income").data(values))
}

pub(super) fn balances_chart(total_account_balance: f64, transactions: &[Transaction]) -> Chart {
    let monthly_totals = aggregate_by_month(transactions);
    let (labels, values) = calculate_running_balances(total_account_balance, &monthly_totals);

    Chart::new()
        .title(
            Title::new()
                .text("Net Balance")
                .subtext("Last twelve months"),
        )
        .tooltip(currency_tooltip())
        .grid(
            Grid::new()
                .left("3%")
                .right("4%")
                .bottom("3%")
                .contain_label(true),
        )
        .x_axis(Axis::new().type_(AxisType::Category).data(labels))
        .y_axis(
            Axis::new()
                .type_(AxisType::Value)
                .axis_label(AxisLabel::new().formatter(currency_formatter())),
        )
        .series(Line::new().name("Balance").data(values))
}

pub(super) fn expenses_chart(transactions: &[Transaction]) -> Chart {
    // Get all unique months from transactions and sort them
    let sorted_months = get_sorted_months(transactions);
    let labels = format_month_labels(&sorted_months);
    let series_data = group_monthly_expenses_by_tag(transactions, &sorted_months);

    let mut chart = Chart::new()
        .title(
            Title::new()
                .text("Monthly Expenses")
                .subtext("Last twelve months, grouped by tag")
                .left(20)
                .top("1%"),
        )
        .tooltip(currency_tooltip())
        .legend(Legend::new().left(250).top("1%"))
        .grid(
            Grid::new()
                .left("3%")
                .right("4%")
                .bottom("3%")
                .top(90)
                .contain_label(true),
        )
        .x_axis(Axis::new().type_(AxisType::Category).data(labels))
        .y_axis(
            Axis::new()
                .type_(AxisType::Value)
                .axis_label(AxisLabel::new().formatter(currency_formatter())),
        );

    for (tag, data) in series_data {
        chart = chart.series(
            bar::Bar::new()
                .name(tag)
                .stack("Expenses")
                .emphasis(Emphasis::new().focus(EmphasisFocus::Series))
                .data(data),
        );
    }

    chart
}

#[inline]
fn currency_formatter() -> JsFunction {
    JsFunction::new_with_args(
        "number",
        // Use USD instead of NZD since it is easier to read (No 'NZ' prefix)
        "const currencyFormatter = new Intl.NumberFormat('en-US', {
              style: 'currency',
              currency: 'USD'
            });
            return (number) ? currencyFormatter.format(number) : \"-\";",
    )
}

/// Creates a tooltip configuration for currency values
fn currency_tooltip() -> Tooltip {
    Tooltip::new()
        .trigger(Trigger::Axis)
        .value_formatter(currency_formatter())
        .axis_pointer(AxisPointer::new().type_(AxisPointerType::Shadow))
}
