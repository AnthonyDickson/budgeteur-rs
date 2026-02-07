use maud::{Markup, html};
use time::Date;

use crate::{
    html::{
        FORM_LABEL_STYLE, FORM_RADIO_GROUP_STYLE, FORM_RADIO_INPUT_STYLE, FORM_RADIO_LABEL_STYLE,
        FORM_TEXT_INPUT_STYLE,
    },
    tag::{Tag, TagId},
    transaction::core::TransactionType,
};

pub struct TransactionFormDefaults<'a> {
    pub transaction_type: TransactionType,
    pub amount: Option<f64>,
    pub date: Date,
    pub description: Option<&'a str>,
    pub tag_id: Option<TagId>,
    pub max_date: Date,
    pub autofocus_amount: bool,
}

pub fn transaction_form_fields(
    defaults: &TransactionFormDefaults<'_>,
    available_tags: &[Tag],
) -> Markup {
    let is_expense = matches!(defaults.transaction_type, TransactionType::Expense);
    let amount_str = defaults.amount.map(|amount| format!("{:.2}", amount.abs()));
    let amount_placeholder = amount_str.as_deref().unwrap_or("0.01");
    let description_placeholder = defaults.description.unwrap_or("Description");

    html! {
        fieldset class="space-y-2"
        {
            legend class=(FORM_LABEL_STYLE) { "Transaction type" }

            div class=(FORM_RADIO_GROUP_STYLE)
            {
                div class="flex items-center gap-3"
                {
                    input
                        name="type_"
                        id="transaction-type-expense"
                        type="radio"
                        value="expense"
                        checked[is_expense]
                        required
                        tabindex="0"
                        class=(FORM_RADIO_INPUT_STYLE);

                    label
                        for="transaction-type-expense"
                        class=(FORM_RADIO_LABEL_STYLE)
                    {
                        "Expense"
                    }
                }

                div class="flex items-center gap-3"
                {
                    input
                        name="type_"
                        id="transaction-type-income"
                        type="radio"
                        value="income"
                        checked[!is_expense]
                        required
                        tabindex="0"
                        class=(FORM_RADIO_INPUT_STYLE);

                    label
                        for="transaction-type-income"
                        class=(FORM_RADIO_LABEL_STYLE)
                    {
                        "Income"
                    }
                }
            }
        }

        div
        {
            label
                for="amount"
                class=(FORM_LABEL_STYLE)
            {
                "Amount"
            }

            div class="input-wrapper w-full"
            {
                input
                    name="amount"
                    id="amount"
                    type="number"
                    step="0.01"
                    placeholder=(amount_placeholder)
                    min="0.01"
                    required
                    value=[amount_str.as_deref()]
                    autofocus[defaults.autofocus_amount]
                    class=(FORM_TEXT_INPUT_STYLE);
            }
        }

        div
        {
            label
                for="date"
                class=(FORM_LABEL_STYLE)
            {
                "Date"
            }

            input
                name="date"
                id="date"
                type="date"
                max=(defaults.max_date)
                value=(defaults.date)
                required
                class=(FORM_TEXT_INPUT_STYLE);
        }

        div
        {
            label
                for="description"
                class=(FORM_LABEL_STYLE)
            {
                "Description"
            }

            input
                name="description"
                id="description"
                type="text"
                placeholder=(description_placeholder)
                value=[defaults.description]
                class=(FORM_TEXT_INPUT_STYLE);
        }

        @if !available_tags.is_empty() {
            div
            {
                label
                    for="tag_id"
                    class=(FORM_LABEL_STYLE)
                {
                    "Tag"
                }

                select
                    name="tag_id"
                    id="tag_id"
                    class=(FORM_TEXT_INPUT_STYLE)
                {
                    option value="" { "Select a tag" }

                    @for tag in available_tags {
                        @if Some(tag.id) == defaults.tag_id {
                            option value=(tag.id) selected { (tag.name) }
                        } @else {
                            option value=(tag.id) { (tag.name) }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use scraper::{Html, Selector};
    use time::OffsetDateTime;

    use super::{TransactionFormDefaults, transaction_form_fields};
    use crate::transaction::core::TransactionType;

    #[test]
    fn transaction_form_fields_checks_selected_type() {
        let cases = [
            (TransactionType::Expense, "expense"),
            (TransactionType::Income, "income"),
        ];

        for (transaction_type, expected) in cases {
            let html = render_fields(transaction_type);
            assert_checked_value(&html, expected);
        }
    }

    fn render_fields(transaction_type: TransactionType) -> Html {
        let max_date = OffsetDateTime::now_utc().date();
        let fields = transaction_form_fields(
            &TransactionFormDefaults {
                transaction_type,
                amount: None,
                date: max_date,
                description: None,
                tag_id: None,
                max_date,
                autofocus_amount: false,
            },
            &[],
        );
        let markup = maud::html! { form { (fields) } };
        Html::parse_document(&markup.into_string())
    }

    fn assert_checked_value(document: &Html, expected: &str) {
        let selector = Selector::parse("input[type=radio][name=type_]").unwrap();
        let inputs = document.select(&selector).collect::<Vec<_>>();
        assert_eq!(
            inputs.len(),
            2,
            "want 2 transaction type inputs, got {}",
            inputs.len()
        );

        let checked = inputs
            .iter()
            .find(|input| input.value().attr("checked").is_some())
            .and_then(|input| input.value().attr("value"));
        assert_eq!(
            checked,
            Some(expected),
            "want checked transaction type to be {expected}, got {checked:?}"
        );
    }
}
