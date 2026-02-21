//! HTML rendering for the transactions page.

use axum::http::Uri;
use maud::{Markup, html};
use time::{Date, Month, format_description::BorrowedFormatItem, macros::format_description};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    endpoints,
    html::{
        LINK_STYLE, PAGE_CONTAINER_STYLE, TABLE_CELL_STYLE, TABLE_HEADER_STYLE, TABLE_ROW_STYLE,
        TAG_BADGE_STYLE, base, edit_delete_action_links, format_currency,
    },
    navigation::NavBar,
    tag::{ExcludedTagsViewConfig, TagWithExclusion, excluded_tags_controls},
};

use super::{
    grouping::{DayGroupRef, group_transactions_by_day},
    models::{CategorySummaryKind, DateInterval, TransactionTableRow, TransactionsViewOptions},
    range::{
        DateRange, IntervalPreset, RangeNavLink, RangeNavigation, RangePreset, compute_range,
        range_label, range_preset_can_contain_interval,
    },
    transactions_page::TransactionsQuery,
};

/// The max number of graphemes to display in the transaction table rows before
/// truncating and displaying ellipses.
const MAX_DESCRIPTION_GRAPHEMES: usize = 32;

fn amount_class(amount: f64) -> &'static str {
    if amount < 0.0 {
        "text-red-700 dark:text-red-300"
    } else {
        "text-green-700 dark:text-green-300"
    }
}

pub(crate) fn transactions_view(
    grouped_transactions: Vec<DateInterval>,
    range_nav: &RangeNavigation,
    latest_link: Option<&RangeNavLink>,
    has_any_transactions: bool,
    tags_with_status: &[TagWithExclusion],
    redirect_url: &str,
    options: TransactionsViewOptions,
) -> Markup {
    let create_transaction_route = Uri::from_static(endpoints::NEW_TRANSACTION_VIEW);
    let import_transaction_route = Uri::from_static(endpoints::IMPORT_VIEW);
    let transactions_page_route = Uri::from_static(endpoints::TRANSACTIONS_VIEW);
    let untagged_transactions_route = Uri::from_static(endpoints::QUICK_TAGGING_VIEW);
    let nav_bar = NavBar::new(endpoints::TRANSACTIONS_VIEW).into_html();
    // Cache this result so it can be accessed after `grouped_transactions` is moved by for loop.
    let transactions_empty = grouped_transactions.is_empty();
    let summary_has_rows = options.show_category_summary
        && grouped_transactions
            .iter()
            .any(|interval| !interval.summary.is_empty());
    let show_empty_state =
        transactions_empty || (options.show_category_summary && !summary_has_rows);
    let empty_message = if options.show_category_summary && !summary_has_rows && !transactions_empty
    {
        "No transactions in this summary after exclusions."
    } else {
        "No transactions in this range."
    };
    let excluded_tags_view = excluded_tags_controls(
        tags_with_status,
        ExcludedTagsViewConfig {
            heading: "Filter Out Tags",
            description: "Exclude transactions with these tags from summary totals and percentages:",
            endpoint: endpoints::TRANSACTIONS_EXCLUDED_TAGS,
            hx_target: Some("#transactions-content"),
            hx_swap: Some("innerHTML"),
            hx_trigger: Some("change"),
            redirect_url: Some(redirect_url),
            form_id: Some("transactions-excluded-tags"),
        },
    );
    let table_wrapper_class = "hidden lg:block";

    let content = html! {
        (nav_bar)

        main class=(PAGE_CONTAINER_STYLE)
        {
            section class="space-y-4" id="transactions-content"
            {
                header class="flex justify-between flex-wrap items-end"
                {
                    h1 class="text-xl font-bold" { "Transactions" }

                    a href=(untagged_transactions_route) class=(LINK_STYLE)
                    {
                        "Quick Tagging"
                    }

                    a href=(import_transaction_route) class=(LINK_STYLE)
                    {
                        "Import Transactions"
                    }

                    a href=(create_transaction_route) class=(LINK_STYLE)
                    {
                        "Create Transaction"
                    }
                }

                section class="rounded bg-gray-50 dark:bg-gray-800 overflow-hidden lg:max-w-5xl lg:w-full lg:mx-auto"
                {
                    @if has_any_transactions {
                        (range_navigation_html(
                            range_nav,
                            latest_link,
                            options.interval_preset,
                            options.show_category_summary,
                            &transactions_page_route,
                        ))
                    }

                    div class="mt-3 border-t border-gray-200 dark:border-gray-700" {}

                    (control_cluster_html(
                        options.range_preset,
                        options.interval_preset,
                        options.show_category_summary,
                        options.anchor_date,
                        &transactions_page_route,
                    ))

                    @if options.show_category_summary {
                        (summary_cards_view(&grouped_transactions, show_empty_state, empty_message))
                    } @else {
                        (transaction_cards_view(&grouped_transactions, show_empty_state, empty_message))
                    }

                    div class=(table_wrapper_class)
                    {
                        table class="w-full my-2 text-sm text-left rtl:text-right
                            text-gray-500 dark:text-gray-400"
                        {
                            thead class=(TABLE_HEADER_STYLE)
                            {
                                tr
                                {
                                    th scope="col" class="px-6 py-3 text-right"
                                    {
                                        "Amount"
                                    }
                                    th scope="col" class="sr-only"
                                    {
                                        "Date"
                                    }
                                    th scope="col" class=(TABLE_CELL_STYLE)
                                    {
                                        "Description"
                                    }
                                    th scope="col" class=(TABLE_CELL_STYLE)
                                    {
                                        "Tags"
                                    }
                                    th scope="col" class=(TABLE_CELL_STYLE)
                                    {
                                        "Actions"
                                    }
                                }
                            }

                            tbody
                            {
                                @for interval in grouped_transactions {
                                    (interval_header_row_view(&interval))

                                    @if options.show_category_summary {
                                        (category_summary_view(&interval))
                                    } @else {
                                        @for day in &interval.days {
                                            (day_header_row_view(day.date))

                                            @for transaction_row in &day.transactions {
                                                (transaction_row_view(transaction_row))
                                            }
                                        }
                                    }
                                }

                                @if show_empty_state {
                                    tr
                                    {
                                        td
                                            colspan="5"
                                            data-empty-state="true"
                                            class="px-6 py-4 text-center"
                                        {
                                            (empty_message)
                                        }
                                    }
                                }
                            }
                        }
                    }

                    @if has_any_transactions {
                        (range_navigation_html(
                            range_nav,
                            latest_link,
                            options.interval_preset,
                            options.show_category_summary,
                            &transactions_page_route,
                        ))
                    }

                }

                aside class="rounded bg-gray-50 dark:bg-gray-900"
                {
                    (excluded_tags_view)
                }
            }
        }
    };

    base("Transactions", &[], &content)
}

fn range_navigation_html(
    range_nav: &RangeNavigation,
    latest_link: Option<&RangeNavLink>,
    interval_preset: IntervalPreset,
    show_category_summary: bool,
    transactions_page_route: &Uri,
) -> Markup {
    let current_range = range_nav.range;
    let row_classes = if latest_link.is_some() {
        "grid-rows-[auto_auto] gap-y-0"
    } else {
        "grid-rows-1"
    };
    let route = transactions_page_route.path();
    let prev_link = range_nav.prev.as_ref().map(|prev| {
        (
            prev.range,
            TransactionsQuery::new(
                range_nav.preset,
                interval_preset,
                prev.anchor_date,
                show_category_summary,
            )
            .to_url(route),
        )
    });
    let next_link = range_nav.next.as_ref().map(|next| {
        (
            next.range,
            TransactionsQuery::new(
                range_nav.preset,
                interval_preset,
                next.anchor_date,
                show_category_summary,
            )
            .to_url(route),
        )
    });
    let latest_href = latest_link.map(|latest| {
        TransactionsQuery::new(
            range_nav.preset,
            interval_preset,
            latest.anchor_date,
            show_category_summary,
        )
        .to_url(route)
    });

    html! {
        nav class="pagination flex justify-center"
        {
            ul class="pagination flex items-center justify-between w-full px-2 py-2 lg:hidden"
            {
                li class="flex items-center justify-start"
                {
                    @if let Some((_, ref href)) = prev_link {
                        a
                            href=(href)
                            role="button"
                            class="inline-flex items-center rounded px-2 py-1 text-sm text-blue-600 hover:underline"
                        { "Prev" }
                    } @else {
                        span class="inline-flex items-center rounded px-2 py-1 text-sm text-gray-400 dark:text-gray-500"
                        { "Prev" }
                    }
                }

                li class="flex-1 text-center font-semibold text-gray-900 dark:text-white px-2"
                {
                    span aria-current="page" { (range_time_label(current_range)) }
                }

                li class="flex items-center justify-end"
                {
                    @if let Some((_, ref href)) = next_link {
                        a
                            href=(href)
                            role="button"
                            class="inline-flex items-center rounded px-2 py-1 text-sm text-blue-600 hover:underline"
                        { "Next" }
                    } @else {
                        span class="inline-flex items-center rounded px-2 py-1 text-sm text-gray-400 dark:text-gray-500"
                        { "Next" }
                    }
                }
            }

            ul class={ "pagination hidden lg:grid grid-cols-3 gap-x-4 p-0 m-0 items-center w-full " (row_classes) }
            {
                @if let Some((range, href)) = prev_link {
                    li class="flex items-center justify-start row-start-1" {
                        a
                            href=(href)
                            role="button"
                            class="block px-3 py-2 rounded-sm text-blue-600 hover:underline"
                        { (range_time_label(range)) }
                    }
                } @else {
                    li class="flex items-center justify-start row-start-1" {}
                }
                li class="flex items-center justify-center row-start-1" {
                    span
                        aria-current="page"
                        class="block px-3 py-2 rounded-sm font-bold text-black dark:text-white"
                    { (range_time_label(current_range)) }
                }
                @if let Some((range, href)) = next_link {
                    li class="flex items-center justify-end row-start-1" {
                        a
                            href=(href)
                            role="button"
                            class="block px-3 py-2 rounded-sm text-blue-600 hover:underline"
                        { (range_time_label(range)) }
                    }
                } @else {
                    li class="flex items-center justify-end row-start-1" {}
                }

                @if let Some(href) = latest_href {
                    li class="flex items-center justify-center row-start-2 col-start-2" {
                        a
                            href=(href)
                            role="button"
                            class="block px-3 pb-1 text-blue-600 hover:underline"
                        { "Latest" }
                    }
                }
            }
        }
    }
}

fn transaction_row_view(row: &TransactionTableRow) -> Markup {
    transaction_row_view_with_class(row, TABLE_ROW_STYLE)
}

fn transaction_row_view_with_class(row: &TransactionTableRow, row_class: &str) -> Markup {
    let amount_str = format_currency(row.amount);
    let amount_class = amount_class(row.amount);
    let (description, tooltip) = format_description(&row.description);
    let confirm_message = format!(
        "Are you sure you want to delete the transaction '{}'? This cannot be undone.",
        row.description
    );

    html! {
        tr class=(row_class) data-transaction-row="true"
        {
            td class={ "px-6 py-4 text-right " (amount_class) } { (amount_str) }
            td class="sr-only" { time datetime=(date_datetime_attr(row.date)) { (row.date) } }
            td class=(TABLE_CELL_STYLE) title=[tooltip] { (description) }
            td class=(TABLE_CELL_STYLE)
            {
                @if let Some(ref tag_name) = row.tag_name {
                    span class=(TAG_BADGE_STYLE)
                    {
                        (tag_name)
                    }
                } @else {
                    span class="text-gray-400 dark:text-gray-500" { "-" }
                }
            }
            td class=(TABLE_CELL_STYLE)
            {
                div class="flex gap-4"
                {
                    (edit_delete_action_links(
                        &row.edit_url,
                        &row.delete_url,
                        &confirm_message,
                        "closest tr",
                        "delete",
                    ))
                }
            }
        }
    }
}

fn transaction_card_row(transaction_row: &TransactionTableRow) -> Markup {
    let amount_class = amount_class(transaction_row.amount);
    let description = transaction_row.description.as_str();

    html! {
        div class="rounded border border-gray-200 bg-gray-50 px-3 py-3 shadow-sm dark:border-gray-700 dark:bg-gray-900/30"
            data-transaction-card="true"
        {
            div class="flex items-start justify-between gap-3"
            {
                div class="min-w-0 flex-1 truncate text-sm font-medium text-gray-900 dark:text-white"
                    title=(description)
                { (description) }
                div class={ "shrink-0 text-sm tabular-nums text-right whitespace-nowrap " (amount_class) }
                { (format_currency(transaction_row.amount)) }
            }

            div class="mt-3 flex items-center justify-between gap-3 border-t border-gray-200 pt-2 text-xs text-gray-500 dark:border-gray-700/80 dark:text-gray-400"
            {
                div class="flex items-center gap-2"
                {
                    @if let Some(ref tag_name) = transaction_row.tag_name {
                        span class=(TAG_BADGE_STYLE) { (tag_name) }
                    } @else {
                        span { "-" }
                    }
                }

                div class="flex items-center gap-4 text-sm text-gray-900 dark:text-white"
                {
                    (edit_delete_action_links(
                        &transaction_row.edit_url,
                        &transaction_row.delete_url,
                        &format!(
                            "Are you sure you want to delete the transaction '{}'? This cannot be undone.",
                            transaction_row.description
                        ),
                        "closest [data-transaction-card='true']",
                        "delete",
                    ))
                }
            }
        }
    }
}

fn transaction_cards_view(
    grouped_transactions: &[DateInterval],
    show_empty_state: bool,
    empty_message: &str,
) -> Markup {
    html! {
        div class="lg:hidden space-y-6"
        {
            @for interval in grouped_transactions {
                div class="rounded border border-gray-200 bg-white shadow-sm overflow-hidden dark:border-gray-700 dark:bg-gray-800"
                {
                    div class="flex items-center justify-between px-4 py-3 text-sm font-semibold text-gray-900 bg-gray-50 dark:bg-gray-700/70 dark:text-white"
                    {
                        span { (range_time_label(interval.range)) }
                        span class="flex items-center gap-3 text-xs"
                        {
                            span class="text-green-700 dark:text-green-300 whitespace-nowrap"
                            { (format_currency(interval.totals.income)) }
                            span class="text-red-700 dark:text-red-300 whitespace-nowrap"
                            { (format_currency(interval.totals.expenses)) }
                        }
                    }

                    @for day in &interval.days {
                        div class="px-4 pt-4 text-[0.65rem] font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400"
                        {
                            (day_time_label(day.date))
                        }

                        div class="px-4 py-3 space-y-3"
                        {
                            @for transaction_row in &day.transactions {
                                (transaction_card_row(transaction_row))
                            }
                        }
                    }
                }
            }

            @if show_empty_state {
                div class="rounded-lg border border-dashed border-gray-300 bg-white px-4 py-6 text-center text-sm text-gray-500 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-400"
                {
                    (empty_message)
                }
            }
        }
    }
}

fn summary_cards_view(
    grouped_transactions: &[DateInterval],
    show_empty_state: bool,
    empty_message: &str,
) -> Markup {
    struct SummaryCardView<'a> {
        category: &'a super::models::CategorySummary,
        percent_label: String,
        total_display: String,
        total_class: &'static str,
        grouped_days: Vec<DayGroupRef<'a>>,
    }

    let summaries_by_interval = grouped_transactions
        .iter()
        .map(|interval| {
            interval
                .summary
                .iter()
                .map(|category| {
                    let percent_label = format!(
                        "{}% of total {}",
                        category.percent,
                        match category.kind {
                            CategorySummaryKind::Income => "income",
                            CategorySummaryKind::Expense => "expenses",
                        }
                    );
                    let total_class = match category.kind {
                        CategorySummaryKind::Income => {
                            "text-right tabular-nums text-green-700 dark:text-green-300"
                        }
                        CategorySummaryKind::Expense => {
                            "text-right tabular-nums text-red-700 dark:text-red-300"
                        }
                    };

                    SummaryCardView {
                        category,
                        percent_label,
                        total_display: format_currency(category.total),
                        total_class,
                        grouped_days: group_transactions_by_day(&category.transactions),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    html! {
        div class="lg:hidden space-y-6"
        {
            @for (interval, summaries) in grouped_transactions.iter().zip(&summaries_by_interval) {
                div class="rounded border border-gray-200 bg-white shadow-sm overflow-hidden dark:border-gray-700 dark:bg-gray-800"
                {
                    div class="flex items-center justify-between px-4 py-3 text-sm font-semibold text-gray-900 bg-gray-50 dark:bg-gray-700/70 dark:text-white"
                    {
                        span { (range_time_label(interval.range)) }
                        span class="flex items-center gap-3 text-xs"
                        {
                            span class="text-green-700 dark:text-green-300 whitespace-nowrap"
                            { (format_currency(interval.totals.income)) }
                            span class="text-red-700 dark:text-red-300 whitespace-nowrap"
                            { (format_currency(interval.totals.expenses)) }
                        }
                    }

                    @for summary in summaries {
                        details class="group border-t border-gray-200 px-4 py-3 dark:border-gray-700"
                        {
                            summary class="flex items-center justify-between cursor-pointer select-none list-none"
                            {
                                span class="flex flex-col"
                                {
                                    span class="text-sm font-medium text-gray-900 dark:text-white"
                                    { (&summary.category.label) }
                                    span class="text-xs text-gray-500 dark:text-gray-400"
                                    { (&summary.percent_label) }
                                }
                                span class="flex items-center gap-3 text-sm"
                                {
                                    span class=(summary.total_class) { (&summary.total_display) }
                                    span class="text-gray-400 group-open:rotate-90 transition-transform" { "›" }
                                }
                            }

                            div class="mt-3 space-y-4"
                            {
                                @for day in &summary.grouped_days {
                                    div class="text-[0.65rem] font-semibold uppercase tracking-wide text-gray-500 dark:text-gray-400"
                                    { (day_time_label(day.date)) }

                                    div class="space-y-3"
                                    {
                                        @for transaction_row in &day.transactions {
                                            (transaction_card_row(transaction_row))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            @if show_empty_state {
                div class="rounded-lg border border-dashed border-gray-300 bg-white px-4 py-6 text-center text-sm text-gray-500 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-400"
                {
                    (empty_message)
                }
            }
        }
    }
}

fn interval_header_row_view(interval: &DateInterval) -> Markup {
    let income = format_currency(interval.totals.income);
    let expenses = format_currency(interval.totals.expenses);

    html! {
        tr class="bg-gray-50 dark:bg-gray-700" data-interval-header="true"
        {
            td colspan="5" class="px-6 py-3"
            {
                div class="flex items-center justify-between font-semibold text-gray-900 dark:text-white"
                {
                    span { (range_time_label(interval.range)) }
                    span class="flex items-center gap-4"
                    {
                        span class="text-green-700 dark:text-green-300 whitespace-nowrap" { (income) }
                        span class="text-red-700 dark:text-red-300 whitespace-nowrap" { (expenses) }
                    }
                }
            }
        }
    }
}

fn category_summary_view(interval: &DateInterval) -> Markup {
    if interval.summary.is_empty() {
        return html! {};
    }

    struct CategorySummaryView<'a> {
        category: &'a super::models::CategorySummary,
        percent_label: String,
        total_display: String,
        total_class: &'static str,
        grouped_days: Vec<DayGroupRef<'a>>,
    }

    let summaries = interval
        .summary
        .iter()
        .map(|category| {
            let percent_label = format!(
                "{}% of total {}",
                category.percent,
                match category.kind {
                    CategorySummaryKind::Income => "income",
                    CategorySummaryKind::Expense => "expenses",
                }
            );
            let total_class = match category.kind {
                CategorySummaryKind::Income => {
                    "text-right tabular-nums text-green-700 dark:text-green-300"
                }
                CategorySummaryKind::Expense => {
                    "text-right tabular-nums text-red-700 dark:text-red-300"
                }
            };

            CategorySummaryView {
                category,
                percent_label,
                total_display: format_currency(category.total),
                total_class,
                grouped_days: group_transactions_by_day(&category.transactions),
            }
        })
        .collect::<Vec<_>>();

    html! {
        @for summary in summaries {
            tr class="border-b border-gray-200 dark:border-gray-700" data-category-summary="true"
            {
                td colspan="5" class="px-6 py-2"
                {
                    details class="group"
                    {
                        summary class="flex items-center justify-between cursor-pointer select-none list-none px-1 py-2"
                        {
                            span class="flex flex-col"
                            {
                                span class="font-medium text-gray-900 dark:text-white"
                                { (&summary.category.label) }
                                span class="text-xs text-gray-500 dark:text-gray-400"
                                {
                                    (&summary.percent_label)
                                }
                            }
                            span class="flex items-center gap-3 text-sm"
                            {
                                span class=(summary.total_class)
                                { (&summary.total_display) }
                                span class="text-gray-400 group-open:rotate-90 transition-transform" { "›" }
                            }
                        }

                        div class="mt-2"
                        {
                            table class="w-full text-sm text-left rtl:text-right text-gray-500 dark:text-gray-400"
                            {
                                tbody class="divide-y divide-gray-200 dark:divide-gray-700"
                                {
                                    @for day in &summary.grouped_days {
                                        (day_header_row_view(day.date))
                                        @for transaction in &day.transactions {
                                            (transaction_row_view_with_class(
                                                transaction,
                                                "bg-white dark:bg-gray-800"
                                            ))
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn day_header_row_view(date: Date) -> Markup {
    let label = day_time_label(date);

    html! {
        tr class="bg-gray-50 dark:bg-gray-800" data-day-header="true"
        {
            td colspan="5" class="px-6 py-2 text-xs font-semibold uppercase text-gray-600 dark:text-gray-300"
            {
                (label)
            }
        }
    }
}

fn format_day_label(date: Date) -> String {
    format!("{:02} {}", date.day(), month_abbrev(date.month()))
}

fn month_abbrev(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

const DATE_ATTRIBUTE_FORMAT: &[BorrowedFormatItem] =
    format_description!("[year]-[month repr:numerical padding:zero]-[day padding:zero]");

fn date_datetime_attr(date: Date) -> String {
    date.format(DATE_ATTRIBUTE_FORMAT)
        .unwrap_or_else(|_| date.to_string())
}

fn range_datetime_attr(range: DateRange) -> String {
    format!(
        "{}/{}",
        date_datetime_attr(range.start),
        date_datetime_attr(range.end)
    )
}

fn range_time_label(range: DateRange) -> Markup {
    let datetime = range_datetime_attr(range);
    let label = range_label(range);

    html! {
        time datetime=(datetime) { (label) }
    }
}

fn day_time_label(date: Date) -> Markup {
    let datetime = date_datetime_attr(date);
    let label = format_day_label(date);

    html! {
        time datetime=(datetime) { (label) }
    }
}

fn control_cluster_html(
    range_preset: RangePreset,
    interval_preset: IntervalPreset,
    show_category_summary: bool,
    anchor_date: Date,
    transactions_page_route: &Uri,
) -> Markup {
    let summary_dot_class = if show_category_summary {
        "inline-flex h-3 w-3 rounded-full bg-green-500"
    } else {
        "inline-flex h-3 w-3 rounded-full bg-gray-400"
    };
    let base_query = TransactionsQuery::new(
        range_preset,
        interval_preset,
        anchor_date,
        show_category_summary,
    );
    let range_links = build_range_links(base_query, transactions_page_route);
    let interval_links = build_interval_links(base_query, transactions_page_route);
    let summary_href = base_query
        .with_summary(!show_category_summary)
        .to_url(transactions_page_route.path());
    let summary_label = if show_category_summary {
        "Summary on"
    } else {
        "Summary off"
    };
    let range = compute_range(range_preset, anchor_date);
    let range_label = range_time_label(range);
    let interval_label = interval_preset.label();

    let controls = html! {
        div class="flex flex-col gap-2 text-sm text-gray-600 dark:text-gray-300"
        {
            div class="flex flex-wrap items-center gap-3"
            {
                span
                    class="font-semibold text-gray-900 dark:text-white min-w-[5.5rem]"
                    title="The overall date span shown in the table."
                {
                    "Range:"
                }
                div class="flex flex-wrap items-center gap-2"
                {
                    @for link in range_links {
                        @match link.state {
                            ControlLinkState::Active => {
                            span class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white"
                            { (link.label) }
                            }
                            ControlLinkState::Disabled => {
                            span
                                class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded text-gray-400 dark:text-gray-500 cursor-not-allowed"
                                title="Select a smaller interval or a larger range to enable this option."
                            { (link.label) }
                            }
                            ControlLinkState::Enabled => {
                            a
                                class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded text-blue-600 hover:underline"
                                href=(link.href)
                            { (link.label) }
                            }
                        }
                    }
                }
            }

            div class="flex flex-wrap items-center gap-3"
            {
                span
                    class="font-semibold text-gray-900 dark:text-white min-w-[5.5rem]"
                    title="How transactions are grouped within the range."
                {
                    "Interval:"
                }
                div class="flex flex-wrap items-center gap-2"
                {
                    @for link in interval_links {
                        @match link.state {
                            ControlLinkState::Active => {
                            span class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white"
                            { (link.label) }
                            }
                            ControlLinkState::Disabled => {
                            span
                                class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded text-gray-400 dark:text-gray-500 cursor-not-allowed"
                                title="Select a larger range to enable this interval."
                            { (link.label) }
                            }
                            ControlLinkState::Enabled => {
                            a
                                class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded text-blue-600 hover:underline"
                                href=(link.href)
                            { (link.label) }
                            }
                        }
                    }
                }
            }

            a
                class="inline-flex items-center gap-2 rounded border border-gray-300 px-3 py-2 text-gray-900 dark:text-white hover:bg-gray-100 dark:border-gray-600 dark:hover:bg-gray-700 self-start"
                href=(summary_href)
            {
                span class=(summary_dot_class) {}
                span { (summary_label) }
            }
        }
    };

    html! {
        div class="lg:hidden border-b border-gray-200 dark:border-gray-700"
        {
            details class="bg-transparent"
            {
                summary class="list-none [&::-webkit-details-marker]:hidden flex items-center justify-between gap-3 px-6 py-3 cursor-pointer"
                {
                    div class="flex flex-col gap-1"
                    {
                        span class="text-sm font-semibold text-gray-900 dark:text-white"
                        {
                            (range_label)
                        }
                        span class="text-xs text-gray-500 dark:text-gray-400"
                        {
                            "Interval: " (interval_label) " · " (summary_label)
                        }
                    }
                    span class="text-xs font-semibold text-blue-600 dark:text-blue-400"
                    {
                        "Filters"
                    }
                }
                div class="border-t border-gray-200 dark:border-gray-700 px-6 py-3"
                {
                    (controls)
                }
            }
        }

        div class="hidden lg:block px-6 py-2"
        {
            (controls)
        }
    }
}

fn range_preset_label(preset: RangePreset) -> &'static str {
    match preset {
        RangePreset::Week => "Week",
        RangePreset::Fortnight => "Fortnight",
        RangePreset::Month => "Month",
        RangePreset::Quarter => "Quarter",
        RangePreset::HalfYear => "Half-year",
        RangePreset::Year => "Year",
    }
}

struct ControlLink {
    label: &'static str,
    href: String,
    state: ControlLinkState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ControlLinkState {
    Active,
    Enabled,
    Disabled,
}

fn build_range_links(
    base_query: TransactionsQuery,
    transactions_page_route: &Uri,
) -> Vec<ControlLink> {
    let range_presets = [
        RangePreset::Week,
        RangePreset::Fortnight,
        RangePreset::Month,
        RangePreset::Quarter,
        RangePreset::HalfYear,
        RangePreset::Year,
    ];

    range_presets
        .iter()
        .map(|preset| {
            let disabled =
                !range_preset_can_contain_interval(*preset, base_query.interval_preset());
            let href = base_query
                .with_range_preset(*preset)
                .to_url(transactions_page_route.path());
            let state = link_state(*preset == base_query.range_preset(), disabled);

            ControlLink {
                label: range_preset_label(*preset),
                href,
                state,
            }
        })
        .collect()
}

fn build_interval_links(
    base_query: TransactionsQuery,
    transactions_page_route: &Uri,
) -> Vec<ControlLink> {
    let interval_presets = [
        IntervalPreset::Week,
        IntervalPreset::Fortnight,
        IntervalPreset::Month,
        IntervalPreset::Quarter,
        IntervalPreset::HalfYear,
        IntervalPreset::Year,
    ];

    interval_presets
        .iter()
        .map(|preset| {
            let disabled = !range_preset_can_contain_interval(base_query.range_preset(), *preset);
            let href = base_query
                .with_interval_preset(*preset)
                .to_url(transactions_page_route.path());
            let state = link_state(*preset == base_query.interval_preset(), disabled);

            ControlLink {
                label: preset.label(),
                href,
                state,
            }
        })
        .collect()
}

fn link_state(is_active: bool, is_disabled: bool) -> ControlLinkState {
    if is_active {
        ControlLinkState::Active
    } else if is_disabled {
        ControlLinkState::Disabled
    } else {
        ControlLinkState::Enabled
    }
}

fn format_description(description: &str) -> (String, Option<&str>) {
    let description_length = description.graphemes(true).count();

    if description_length <= MAX_DESCRIPTION_GRAPHEMES {
        (description.to_owned(), None)
    } else {
        let truncated: String = description
            .graphemes(true)
            .take(MAX_DESCRIPTION_GRAPHEMES - 3)
            .collect();
        let truncated = truncated + "...";
        (truncated, Some(description))
    }
}
