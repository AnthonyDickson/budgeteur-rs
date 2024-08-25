use thiserror::Error;

use crate::DatabaseID;

#[derive(Debug, Error)]
#[error("{0} is not a valid ratio.")]
pub struct RatioError(String);

#[derive(Debug, PartialEq, Clone)]
pub struct Ratio(f64);

impl Ratio {
    /// Create a new ratio.
    ///
    /// # Errors
    ///
    /// This function will return an error if `value` is negative or greater than one.
    pub fn new(value: f64) -> Result<Self, RatioError> {
        if (0.0..=1.0).contains(&value) && value.is_sign_positive() {
            Ok(Self(value))
        } else {
            Err(RatioError(value.to_string()))
        }
    }

    // TODO: Remove `unsafe` from `::new_unchecked` functions that could result in incorrect behaviour, but not result in memory related issues.
    /// Create a new ratio without validation.
    ///
    /// # Safety
    ///
    /// This function should only be called on values from a trusted source of validated values such as the application's database.
    pub unsafe fn new_unchecked(value: f64) -> Self {
        Self(value)
    }

    pub fn as_f64(&self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod ratio_tests {
    use crate::savings_ratio::Ratio;

    #[test]
    fn new_succeeds_on_valid_values() {
        for value in 0..=100 {
            assert!(Ratio::new(value as f64 / 100.0).is_ok());
        }
    }

    #[test]
    fn new_fails_on_negative_zero() {
        assert!(Ratio::new(-0.0).is_err());
    }

    #[test]
    fn new_fails_on_negative_value() {
        assert!(Ratio::new(-0.01).is_err());
    }

    #[test]
    fn new_fails_on_value_greater_than_one() {
        assert!(Ratio::new(1.01).is_err());
    }
}

/// The amount of an income transaction that should counted as savings.
///
/// This object must be attached to an existing transaction and cannot exist independently.
///
/// New instances should be created through `SavingsRatio::insert(...)`.
#[derive(Debug, PartialEq)]
pub struct SavingsRatio {
    transaction_id: DatabaseID,
    ratio: Ratio,
}

impl SavingsRatio {
    /// Create a new savings ratio for a transaction.
    ///
    /// Note that this function does not add the created object to the application database, this must be done separately via `SavingsRatio::insert`.
    pub fn new(transaction_id: DatabaseID, ratio: Ratio) -> Self {
        Self {
            transaction_id,
            ratio,
        }
    }

    pub fn transaction_id(&self) -> DatabaseID {
        self.transaction_id
    }

    pub fn ratio(&self) -> &Ratio {
        &self.ratio
    }
}
