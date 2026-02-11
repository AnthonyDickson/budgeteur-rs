//! Grouping logic for transactions (buckets, day groups, summaries).

use std::collections::HashMap;

use crate::tag::TagId;

use super::{
    models::{CategorySummary, CategorySummaryKind, DateBucket, DayGroup, TransactionTableRow},
    window::{BucketPreset, compute_bucket_range},
};

pub(crate) struct GroupingOptions<'a> {
    pub(crate) bucket_preset: BucketPreset,
    pub(crate) excluded_tag_ids: &'a [TagId],
    pub(crate) show_category_summary: bool,
}

pub(crate) fn group_transactions(
    transactions: Vec<TransactionTableRow>,
    options: GroupingOptions<'_>,
) -> Vec<DateBucket> {
    let mut buckets: Vec<DateBucket> = Vec::new();

    for transaction in transactions {
        let bucket_range = compute_bucket_range(options.bucket_preset, transaction.date);
        let bucket = match buckets.last_mut() {
            Some(current) if current.range == bucket_range => current,
            _ => {
                buckets.push(DateBucket::new(bucket_range));
                buckets.last_mut().expect("bucket just added")
            }
        };

        if transaction
            .tag_id
            .map(|tag_id| !options.excluded_tag_ids.contains(&tag_id))
            .unwrap_or(true)
        {
            if transaction.amount < 0.0 {
                bucket.totals.expenses += transaction.amount;
            } else {
                bucket.totals.income += transaction.amount;
            }
        }

        let day_group = match bucket.days.last_mut() {
            Some(current) if current.date == transaction.date => current,
            _ => {
                bucket.days.push(DayGroup {
                    date: transaction.date,
                    transactions: Vec::new(),
                });
                bucket.days.last_mut().expect("day group just added")
            }
        };

        day_group.transactions.push(transaction);
    }

    if options.show_category_summary {
        apply_category_summaries(&mut buckets, options.excluded_tag_ids);
    }

    buckets
}

pub(crate) struct DayGroupRef<'a> {
    pub(crate) date: time::Date,
    pub(crate) transactions: Vec<&'a TransactionTableRow>,
}

pub(crate) fn group_transactions_by_day<'a>(
    transactions: &'a [TransactionTableRow],
) -> Vec<DayGroupRef<'a>> {
    let mut days: Vec<DayGroupRef<'a>> = Vec::new();

    for transaction in transactions {
        let day_group = match days.last_mut() {
            Some(current) if current.date == transaction.date => current,
            _ => {
                days.push(DayGroupRef {
                    date: transaction.date,
                    transactions: Vec::new(),
                });
                days.last_mut().expect("day group just added")
            }
        };

        day_group.transactions.push(transaction);
    }

    days
}

fn apply_category_summaries(buckets: &mut [DateBucket], excluded_tag_ids: &[TagId]) {
    for bucket in buckets {
        bucket.summary = build_category_summary(bucket, excluded_tag_ids);
    }
}

fn build_category_summary(bucket: &DateBucket, excluded_tag_ids: &[TagId]) -> Vec<CategorySummary> {
    let mut income_categories: HashMap<String, CategorySummaryBuilder> = HashMap::new();
    let mut expense_categories: HashMap<String, CategorySummaryBuilder> = HashMap::new();

    for day in &bucket.days {
        for transaction in &day.transactions {
            if transaction
                .tag_id
                .map(|tag_id| excluded_tag_ids.contains(&tag_id))
                .unwrap_or(false)
            {
                continue;
            }

            let label = transaction
                .tag_name
                .as_ref()
                .map(|name| name.to_string())
                .unwrap_or_else(|| "Other".to_owned());
            let entry = if transaction.amount >= 0.0 {
                income_categories.entry(label).or_default()
            } else {
                expense_categories.entry(label).or_default()
            };
            entry.total += transaction.amount;
            entry.transactions.push(transaction.clone());
        }
    }

    let mut income = Vec::new();
    let mut expenses = Vec::new();
    let total_income = bucket.totals.income;
    let total_expenses = bucket.totals.expenses;

    for (label, builder) in income_categories {
        let percent = percent_of(builder.total, total_income);
        let summary = CategorySummary {
            label,
            total: builder.total,
            percent,
            kind: CategorySummaryKind::Income,
            transactions: builder.transactions,
        };

        income.push(summary);
    }

    for (label, builder) in expense_categories {
        let percent = percent_of(builder.total, total_expenses);
        let summary = CategorySummary {
            label,
            total: builder.total,
            percent,
            kind: CategorySummaryKind::Expense,
            transactions: builder.transactions,
        };

        expenses.push(summary);
    }

    income.sort_by(|a, b| {
        b.total
            .partial_cmp(&a.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    expenses.sort_by(|a, b| {
        b.total
            .abs()
            .partial_cmp(&a.total.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    income.extend(expenses);
    income
}

#[derive(Default)]
struct CategorySummaryBuilder {
    total: f64,
    transactions: Vec<TransactionTableRow>,
}

fn percent_of(value: f64, total: f64) -> i64 {
    if total == 0.0 {
        0
    } else {
        ((value / total) * 100.0).round() as i64
    }
}

#[cfg(test)]
mod tests {
    use time::macros::date;

    use crate::tag::{TagId, TagName};

    use super::{GroupingOptions, group_transactions};
    use crate::transaction::{
        models::{CategorySummaryKind, TransactionTableRow},
        window::BucketPreset,
    };

    fn row(
        amount: f64,
        date: time::Date,
        tag_name: Option<&str>,
        tag_id: Option<TagId>,
    ) -> TransactionTableRow {
        TransactionTableRow {
            amount,
            date,
            description: "test".to_owned(),
            tag_name: tag_name.map(TagName::new_unchecked),
            tag_id,
            edit_url: "/edit".to_owned(),
            delete_url: "/delete".to_owned(),
        }
    }

    #[test]
    fn grouping_excludes_tags_from_bucket_totals() {
        let excluded_tag: TagId = 1;
        let included_tag: TagId = 2;
        let transactions = vec![
            row(
                100.0,
                date!(2025 - 10 - 05),
                Some("Income"),
                Some(included_tag),
            ),
            row(
                -50.0,
                date!(2025 - 10 - 05),
                Some("Bills"),
                Some(excluded_tag),
            ),
        ];

        let buckets = group_transactions(
            transactions,
            GroupingOptions {
                bucket_preset: BucketPreset::Week,
                excluded_tag_ids: &[excluded_tag],
                show_category_summary: true,
            },
        );

        assert_eq!(buckets.len(), 1);
        let bucket = &buckets[0];
        assert_eq!(bucket.totals.income, 100.0);
        assert_eq!(bucket.totals.expenses, 0.0);
    }

    #[test]
    fn grouping_splits_income_and_expenses_per_tag() {
        let tag: TagId = 1;
        let transactions = vec![
            row(200.0, date!(2025 - 10 - 05), Some("Other"), Some(tag)),
            row(-75.0, date!(2025 - 10 - 05), Some("Other"), Some(tag)),
        ];

        let buckets = group_transactions(
            transactions,
            GroupingOptions {
                bucket_preset: BucketPreset::Week,
                excluded_tag_ids: &[],
                show_category_summary: true,
            },
        );

        assert_eq!(buckets.len(), 1);
        let summary = &buckets[0].summary;
        assert_eq!(summary.len(), 2);
        assert!(
            summary
                .iter()
                .any(|item| { item.label == "Other" && item.kind == CategorySummaryKind::Income })
        );
        assert!(
            summary
                .iter()
                .any(|item| { item.label == "Other" && item.kind == CategorySummaryKind::Expense })
        );
    }
}
