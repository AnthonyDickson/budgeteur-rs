use crate::DatabaseID;

/// The amount of an income transaction that should counted as savings.
///
/// This object must be attached to an existing transaction and cannot exist independently.
///
/// New instances should be created through `SavingsRatio::insert(...)`.
#[derive(Debug, PartialEq)]
pub struct SavingsRatio {
    transaction_id: DatabaseID,
    // TODO: Create newtype for ratio that restricts values to the interval [0.0, 1.0] and remove validation code from `SavingsRatio::insert`.
    ratio: f64,
}

impl SavingsRatio {
    pub fn new(transaction_id: DatabaseID, ratio: f64) -> Self {
        Self {
            transaction_id,
            ratio,
        }
    }

    pub fn transaction_id(&self) -> DatabaseID {
        self.transaction_id
    }

    pub fn ratio(&self) -> f64 {
        self.ratio
    }
}
