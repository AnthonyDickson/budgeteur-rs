use std::collections::HashMap;

use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
};
use axum_extra::extract::Form;
use maud::{PreEscaped, html};
use serde::{Deserialize, Deserializer};

use crate::{
    Error,
    alert::Alert,
    tag::{TagId, get_all_tags},
    transaction::TransactionId,
};

use super::{
    page::{QuickTaggingQueueState, quick_tagging_queue_content},
    queue::{
        QUICK_TAGGING_QUEUE_PAGE_SIZE, apply_quick_tagging_updates, dismiss_untagged_transactions,
        get_untagged_transactions,
    },
};

type TagUpdates = Vec<(TransactionId, TagId)>;
type InvalidTagUpdates = Vec<(String, String)>;

#[derive(Debug, Deserialize)]
pub struct QuickTaggingBatchForm {
    #[serde(default, deserialize_with = "deserialize_dismiss")]
    dismiss: Vec<TransactionId>,
    #[serde(flatten)]
    fields: HashMap<String, String>,
}

pub async fn apply_quick_tagging_endpoint(
    State(state): State<QuickTaggingQueueState>,
    Form(form): Form<QuickTaggingBatchForm>,
) -> Response {
    let connection = match state.db_connection.lock() {
        Ok(connection) => connection,
        Err(error) => {
            tracing::error!("could not acquire database lock: {error}");
            return Error::DatabaseLockError.into_alert_response();
        }
    };

    let (tag_updates, invalid_updates) = collect_tag_updates(&form.fields);
    if !invalid_updates.is_empty() {
        for (key, value) in invalid_updates {
            tracing::warn!(key = %key, value = %value, "Invalid tag update field");
        }
        return Alert::ErrorSimple {
            message: "Invalid tag selection".to_owned(),
        }
        .into_response();
    }

    let mut dismiss_ids = form.dismiss;
    if !tag_updates.is_empty() && !dismiss_ids.is_empty() {
        dismiss_ids.retain(|id| !tag_updates.iter().any(|(tagged_id, _)| tagged_id == id));
    }

    if tag_updates.is_empty() && dismiss_ids.is_empty() {
        return Alert::ErrorSimple {
            message: "Select at least one tag or dismiss a transaction".to_owned(),
        }
        .into_response();
    }

    let tx = match connection.unchecked_transaction() {
        Ok(tx) => tx,
        Err(error) => {
            tracing::error!("could not start transaction: {error}");
            return Alert::ErrorSimple {
                message: "Could not apply changes".to_owned(),
            }
            .into_response();
        }
    };

    let tagged_count = match apply_quick_tagging_updates(&tag_updates, &tx) {
        Ok(count) => count,
        Err(error) => {
            tracing::error!("could not apply tag updates: {error}");
            return error.into_alert_response();
        }
    };

    let dismissed_count = match dismiss_untagged_transactions(&dismiss_ids, &tx) {
        Ok(count) => count,
        Err(error) => {
            tracing::error!("could not dismiss queue rows: {error}");
            return error.into_alert_response();
        }
    };

    if let Err(error) = tx.commit() {
        tracing::error!("could not commit transaction: {error}");
        return Alert::ErrorSimple {
            message: "Could not apply changes".to_owned(),
        }
        .into_response();
    }

    let queue_rows = match get_untagged_transactions(QUICK_TAGGING_QUEUE_PAGE_SIZE, &connection) {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!("could not fetch queue rows: {error}");
            return error.into_alert_response();
        }
    };
    let tags = match get_all_tags(&connection) {
        Ok(tags) => tags,
        Err(error) => {
            tracing::error!("could not get tags: {error}");
            return error.into_alert_response();
        }
    };

    let message = "Changes applied".to_owned();
    let details =
        format!("Applied tags to {tagged_count} transactions, dismissed {dismissed_count}");
    let alert_html = Alert::Success { message, details }.into_html().0;
    let content = quick_tagging_queue_content(&queue_rows, &tags);

    let response_body = html! {
        (content)
        (PreEscaped(alert_html))
    };

    Html(response_body.into_string()).into_response()
}

fn collect_tag_updates(fields: &HashMap<String, String>) -> (TagUpdates, InvalidTagUpdates) {
    let mut tag_updates: TagUpdates = Vec::new();
    let mut invalid_updates: InvalidTagUpdates = Vec::new();

    for (key, value) in fields {
        if let Some(transaction_id) = key.strip_prefix("tag_id_") {
            if let (Ok(transaction_id), Ok(tag_id)) = (
                transaction_id.parse::<TransactionId>(),
                value.parse::<TagId>(),
            ) {
                tag_updates.push((transaction_id, tag_id));
            } else {
                invalid_updates.push((key.clone(), value.clone()));
            }
        }
    }

    (tag_updates, invalid_updates)
}

fn deserialize_dismiss<'de, D>(deserializer: D) -> Result<Vec<TransactionId>, D::Error>
where
    D: Deserializer<'de>,
{
    struct DismissVisitor;

    impl<'de> serde::de::Visitor<'de> for DismissVisitor {
        type Value = Vec<TransactionId>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a transaction id or a list of transaction ids")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(vec![value as TransactionId])
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            value
                .parse::<TransactionId>()
                .map(|id| vec![id])
                .map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            self.visit_str(&value)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element::<TransactionId>()? {
                values.push(value);
            }
            Ok(values)
        }
    }

    deserializer.deserialize_any(DismissVisitor)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::QuickTaggingBatchForm;

    #[test]
    fn form_deserializes_from_payload() {
        let payload = "tag_id_1030=1&tag_id_1024=1&tag_id_1022=1&tag_id_1015=4&dismiss=1010";

        let form: QuickTaggingBatchForm =
            serde_urlencoded::from_str(payload).expect("Could not parse form payload");

        assert_eq!(form.fields.get("tag_id_1030"), Some(&"1".to_owned()));
        assert_eq!(form.fields.get("tag_id_1024"), Some(&"1".to_owned()));
        assert_eq!(form.fields.get("tag_id_1022"), Some(&"1".to_owned()));
        assert_eq!(form.fields.get("tag_id_1015"), Some(&"4".to_owned()));
        assert_eq!(form.dismiss, vec![1010]);
    }

    #[test]
    fn collect_tag_updates_reports_parse_errors() {
        let mut fields = HashMap::new();
        fields.insert("tag_id_12".to_owned(), "3".to_owned());
        fields.insert("tag_id_bad".to_owned(), "3".to_owned());
        fields.insert("tag_id_9".to_owned(), "nope".to_owned());
        fields.insert("other".to_owned(), "1".to_owned());

        let (updates, mut invalid_updates) = super::collect_tag_updates(&fields);

        assert_eq!(updates, vec![(12, 3)]);
        invalid_updates.sort();
        assert_eq!(
            invalid_updates,
            vec![
                ("tag_id_9".to_owned(), "nope".to_owned()),
                ("tag_id_bad".to_owned(), "3".to_owned()),
            ]
        );
    }
}
