use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::{DatabaseID, UserID};

/// An expense or income, i.e. an event where money was either spent or earned.
///
/// New instances should be created through `Transaction::insert(...)`.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    id: DatabaseID,
    amount: f64,
    date: NaiveDate,
    description: String,
    category_id: DatabaseID,
    user_id: UserID,
}

impl Transaction {
    /// Create a new `Transaction`.
    ///
    /// Note that this does *not* add the transaction to the application database.
    pub fn new(
        id: DatabaseID,
        amount: f64,
        date: NaiveDate,
        description: String,
        category_id: DatabaseID,
        user_id: UserID,
    ) -> Self {
        Self {
            id,
            amount,
            date,
            description,
            category_id,
            user_id,
        }
    }

    pub fn id(&self) -> DatabaseID {
        self.id
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }

    pub fn date(&self) -> &NaiveDate {
        &self.date
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn category_id(&self) -> DatabaseID {
        self.category_id
    }

    pub fn user_id(&self) -> UserID {
        self.user_id
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NewTransaction {
    pub amount: f64,
    pub date: NaiveDate,
    pub description: String,
    pub category_id: DatabaseID,
    pub user_id: UserID,
}
