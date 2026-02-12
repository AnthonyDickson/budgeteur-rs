//! HTML rendering for the transactions page.

use axum::http::Uri;
use maud::{Markup, html};
use time::{Date, Month};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    endpoints,
    html::{
        BUTTON_DELETE_STYLE, LINK_STYLE, PAGE_CONTAINER_STYLE, TABLE_CELL_STYLE,
        TABLE_HEADER_STYLE, TABLE_ROW_STYLE, TAG_BADGE_STYLE, base, format_currency,
    },
    navigation::NavBar,
};

use super::{
    grouping::group_transactions_by_day,
    models::{CategorySummaryKind, DateBucket, TransactionTableRow, TransactionsViewOptions},
    window::{BucketPreset, WindowNavLink, WindowNavigation, WindowPreset, window_range_label},
};

/// The max number of graphemes to display in the transaction table rows before
/// truncating and displaying ellipses.
const MAX_DESCRIPTION_GRAPHEMES: usize = 32;

pub(crate) fn transactions_view(
    grouped_transactions: Vec<DateBucket>,
    window_nav: &WindowNavigation,
    latest_link: Option<&WindowNavLink>,
    has_any_transactions: bool,
    options: TransactionsViewOptions,
) -> Markup {
    let create_transaction_route = Uri::from_static(endpoints::NEW_TRANSACTION_VIEW);
    let import_transaction_route = Uri::from_static(endpoints::IMPORT_VIEW);
    let transactions_page_route = Uri::from_static(endpoints::TRANSACTIONS_VIEW);
    let nav_bar = NavBar::new(endpoints::TRANSACTIONS_VIEW).into_html();
    // Cache this result so it can be accessed after `grouped_transactions` is moved by for loop.
    let transactions_empty = grouped_transactions.is_empty();
    let summary_has_rows = options.show_category_summary
        && grouped_transactions.iter().any(|bucket| !bucket.summary.is_empty());
    let show_empty_state =
        transactions_empty || (options.show_category_summary && !summary_has_rows);
    let empty_message = if options.show_category_summary && !summary_has_rows && !transactions_empty
    {
        "No transactions in this summary after exclusions."
    } else {
        "No transactions in this range."
    };

    let content = html! {
        (nav_bar)

        div class=(PAGE_CONTAINER_STYLE)
        {
            div class="relative"
            {
                div class="flex justify-between flex-wrap items-end mb-4"
                {
                    h1 class="text-xl font-bold" { "Transactions" }

                    a href=(import_transaction_route) class=(LINK_STYLE)
                    {
                        "Import Transactions"
                    }

                    a href=(create_transaction_route) class=(LINK_STYLE)
                    {
                        "Create Transaction"
                    }
                }

                div class="dark:bg-gray-800"
                {
                    @if has_any_transactions {
                        (window_navigation_html(
                            window_nav,
                            latest_link,
                            options.bucket_preset,
                            options.show_category_summary,
                            &transactions_page_route,
                        ))
                    }

                    div class="mt-3 border-t border-gray-200 dark:border-gray-700" {}

                    (control_cluster_html(
                        options.window_preset,
                        options.bucket_preset,
                        options.show_category_summary,
                        options.anchor_date,
                        &transactions_page_route,
                    ))

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
                            @for bucket in grouped_transactions {
                                (bucket_header_row_view(&bucket))

                                @if options.show_category_summary {
                                    (category_summary_view(&bucket))
                                } @else {
                                    @for day in &bucket.days {
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

                    @if has_any_transactions {
                        (window_navigation_html(
                            window_nav,
                            latest_link,
                            options.bucket_preset,
                            options.show_category_summary,
                            &transactions_page_route,
                        ))
                    }
                }
            }
        }
    };

    base("Transactions", &[], &content)
}

fn window_navigation_html(
    window_nav: &WindowNavigation,
    latest_link: Option<&WindowNavLink>,
    bucket_preset: BucketPreset,
    show_category_summary: bool,
    transactions_page_route: &Uri,
) -> Markup {
    let summary_param = if show_category_summary {
        "&summary=true"
    } else {
        ""
    };
    let current_label = window_range_label(window_nav.range);
    let row_classes = if latest_link.is_some() {
        "grid-rows-2 gap-y-0.5"
    } else {
        "grid-rows-1"
    };
    let prev_link = window_nav.prev.as_ref().map(|prev| {
        (
            window_range_label(prev.range),
            format!(
                "{route}?{href}&bucket={bucket}{summary}",
                route = transactions_page_route,
                href = prev.href,
                bucket = bucket_preset.as_query_value(),
                summary = summary_param
            ),
        )
    });
    let next_link = window_nav.next.as_ref().map(|next| {
        (
            window_range_label(next.range),
            format!(
                "{route}?{href}&bucket={bucket}{summary}",
                route = transactions_page_route,
                href = next.href,
                bucket = bucket_preset.as_query_value(),
                summary = summary_param
            ),
        )
    });
    let latest_href = latest_link.map(|latest| {
        format!(
            "{route}?{href}&bucket={bucket}{summary}",
            route = transactions_page_route,
            href = latest.href,
            bucket = bucket_preset.as_query_value(),
            summary = summary_param
        )
    });

    html! {
        nav class="pagination flex justify-center"
        {
            ul class={ "pagination grid grid-cols-3 gap-x-4 p-0 m-0 items-center w-full " (row_classes) }
            {
                @if let Some((label, href)) = prev_link {
                    li class="flex items-center justify-start row-start-1" {
                        a
                            href=(href)
                            role="button"
                            class="block px-3 py-2 rounded-sm text-blue-600 hover:underline"
                        { (label) }
                    }
                } @else {
                    li class="flex items-center justify-start row-start-1" {}
                }
                li class="flex items-center justify-center row-start-1" {
                    span
                        aria-current="page"
                        class="block px-3 py-2 rounded-sm font-bold text-black dark:text-white"
                    { (current_label) }
                }
                @if let Some((label, href)) = next_link {
                    li class="flex items-center justify-end row-start-1" {
                        a
                            href=(href)
                            role="button"
                            class="block px-3 py-2 rounded-sm text-blue-600 hover:underline"
                        { (label) }
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
    let (description, tooltip) = format_description(&row.description);

    html! {
        tr class=(row_class) data-transaction-row="true"
        {
            td class="px-6 py-4 text-right" { (amount_str) }
            td class="sr-only" { (row.date) }
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
                    a href=(row.edit_url) class=(LINK_STYLE)
                    {
                        "Edit"
                    }

                    button
                        hx-delete=(row.delete_url)
                        hx-confirm={
                            "Are you sure you want to delete the transaction '"
                            (row.description) "'? This cannot be undone."
                        }
                        hx-target="closest tr"
                        hx-target-error="#alert-container"
                        hx-swap="outerHTML"
                        class=(BUTTON_DELETE_STYLE)
                    {
                       "Delete"
                    }
                }
            }
        }
    }
}

fn bucket_header_row_view(bucket: &DateBucket) -> Markup {
    let label = window_range_label(bucket.range);
    let income = format_currency(bucket.totals.income);
    let expenses = format_currency(bucket.totals.expenses);

    html! {
        tr class="bg-gray-50 dark:bg-gray-700" data-bucket-header="true"
        {
            td colspan="5" class="px-6 py-3"
            {
                div class="flex items-center justify-between font-semibold text-gray-900 dark:text-white"
                {
                    span { (label) }
                    span class="flex items-center gap-4"
                    {
                        span class="text-green-700 dark:text-green-300" { (income) }
                        span class="text-red-700 dark:text-red-300" { (expenses) }
                    }
                }
            }
        }
    }
}

fn category_summary_view(bucket: &DateBucket) -> Markup {
    if bucket.summary.is_empty() {
        return html! {};
    }

    html! {
        @for category in &bucket.summary {
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
                                { (&category.label) }
                                span class="text-xs text-gray-500 dark:text-gray-400"
                                {
                                    (format!(
                                        "{}% of total {}",
                                        category.percent,
                                        match category.kind {
                                            CategorySummaryKind::Income => "income",
                                            CategorySummaryKind::Expense => "expenses",
                                        }
                                    ))
                                }
                            }
                            span class="flex items-center gap-3 text-sm"
                            {
                                span class={
                                    "text-right tabular-nums " (if category.kind == CategorySummaryKind::Income {
                                        "text-green-700 dark:text-green-300"
                                    } else {
                                        "text-red-700 dark:text-red-300"
                                    })
                                }
                                { (format_currency(category.total)) }
                                span class="text-gray-400 group-open:rotate-90 transition-transform" { "›" }
                            }
                        }

                        div class="mt-2"
                        {
                            table class="w-full text-sm text-left rtl:text-right text-gray-500 dark:text-gray-400"
                            {
                                tbody class="divide-y divide-gray-200 dark:divide-gray-700"
                                {
                                    @for day in group_transactions_by_day(&category.transactions) {
                                        (day_header_row_view(day.date))
                                        @for transaction in day.transactions {
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
    let label = format_day_label(date);

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

fn control_cluster_html(
    window_preset: WindowPreset,
    bucket_preset: BucketPreset,
    show_category_summary: bool,
    anchor_date: Date,
    transactions_page_route: &Uri,
) -> Markup {
    let summary_param = if show_category_summary {
        "&summary=true"
    } else {
        ""
    };
    let window_links = build_window_links(
        window_preset,
        bucket_preset,
        anchor_date,
        summary_param,
        transactions_page_route,
    );
    let bucket_links = build_bucket_links(
        window_preset,
        bucket_preset,
        anchor_date,
        summary_param,
        transactions_page_route,
    );
    let summary_href = format!(
        "{route}?window={window}&bucket={bucket}&anchor={anchor}{summary_param}",
        route = transactions_page_route,
        window = window_preset.as_query_value(),
        bucket = bucket_preset.as_query_value(),
        anchor = anchor_date,
        summary_param = if show_category_summary {
            ""
        } else {
            "&summary=true"
        }
    );
    let summary_label = if show_category_summary {
        "Summary on"
    } else {
        "Summary off"
    };

    html! {
        div class="flex flex-col gap-2 px-6 py-2 text-sm text-gray-600 dark:text-gray-300"
        {
            div class="flex flex-wrap items-center gap-3"
            {
                span class="font-semibold text-gray-900 dark:text-white min-w-[5.5rem]" { "Window:" }
                div class="flex flex-wrap items-center gap-2"
                {
                    @for link in window_links {
                        @match link.state {
                            ControlLinkState::Active => {
                            span class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white"
                            { (link.label) }
                            }
                            ControlLinkState::Disabled => {
                            span
                                class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded text-gray-400 dark:text-gray-500 cursor-not-allowed"
                                title="Select a smaller bucket size or a larger window to enable this option."
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
                span class="font-semibold text-gray-900 dark:text-white min-w-[5.5rem]" { "Bucket:" }
                div class="flex flex-wrap items-center gap-2"
                {
                    @for link in bucket_links {
                        @match link.state {
                            ControlLinkState::Active => {
                            span class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded bg-gray-200 dark:bg-gray-700 text-gray-900 dark:text-white"
                            { (link.label) }
                            }
                            ControlLinkState::Disabled => {
                            span
                                class="inline-flex min-w-[5rem] items-center justify-center px-2 py-1 rounded text-gray-400 dark:text-gray-500 cursor-not-allowed"
                                title="Select a larger window to enable this bucket size."
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
                span class={ "inline-flex h-3 w-3 rounded-full " (if show_category_summary { "bg-green-500" } else { "bg-gray-400" }) } {}
                span { (summary_label) }
            }
        }
    }
}

fn window_preset_label(preset: WindowPreset) -> &'static str {
    match preset {
        WindowPreset::Week => "Week",
        WindowPreset::Fortnight => "Fortnight",
        WindowPreset::Month => "Month",
        WindowPreset::Quarter => "Quarter",
        WindowPreset::HalfYear => "Half-year",
        WindowPreset::Year => "Year",
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

fn build_window_links(
    window_preset: WindowPreset,
    bucket_preset: BucketPreset,
    anchor_date: Date,
    summary_param: &str,
    transactions_page_route: &Uri,
) -> Vec<ControlLink> {
    let window_presets = [
        WindowPreset::Week,
        WindowPreset::Fortnight,
        WindowPreset::Month,
        WindowPreset::Quarter,
        WindowPreset::HalfYear,
        WindowPreset::Year,
    ];

    window_presets
        .iter()
        .map(|preset| {
            let disabled = !super::window::window_preset_can_contain_bucket(*preset, bucket_preset);
            let href = format!(
                "{route}?window={window}&bucket={bucket}&anchor={anchor}{summary_param}",
                route = transactions_page_route,
                window = preset.as_query_value(),
                bucket = bucket_preset.as_query_value(),
                anchor = anchor_date,
                summary_param = summary_param
            );
            let state = link_state(*preset == window_preset, disabled);

            ControlLink {
                label: window_preset_label(*preset),
                href,
                state,
            }
        })
        .collect()
}

fn build_bucket_links(
    window_preset: WindowPreset,
    bucket_preset: BucketPreset,
    anchor_date: Date,
    summary_param: &str,
    transactions_page_route: &Uri,
) -> Vec<ControlLink> {
    let bucket_presets = [
        BucketPreset::Week,
        BucketPreset::Fortnight,
        BucketPreset::Month,
        BucketPreset::Quarter,
        BucketPreset::HalfYear,
        BucketPreset::Year,
    ];

    bucket_presets
        .iter()
        .map(|preset| {
            let disabled = !super::window::window_preset_can_contain_bucket(window_preset, *preset);
            let href = format!(
                "{route}?window={window}&bucket={bucket}&anchor={anchor}{summary_param}",
                route = transactions_page_route,
                window = window_preset.as_query_value(),
                bucket = preset.as_query_value(),
                anchor = anchor_date,
                summary_param = summary_param
            );
            let state = link_state(*preset == bucket_preset, disabled);

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
