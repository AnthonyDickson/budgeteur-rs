//! Defines the token struct used in the auth cookies and now to serialize/deserialize a token.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::auth::UserID;

mod datetime_format {
    //! Specifies how to serialize a [time::OffsetDateTime] in a custom format that
    //! avoids serialisations with datetimes containing midnight.
    //!
    //! The default serializer for [time::OffsetDateTime] will serialize
    //! "00:00:00.000000" as "0:00:00.0" and the deserializer would error out
    //! because it expects the hours to be two digits, not one.
    use serde::{Deserialize, Deserializer, Serializer};
    use time::{
        OffsetDateTime, format_description::BorrowedFormatItem, macros::format_description,
    };

    /// Date time format for the cookie expiry, e.g. "2021-01-01 00:00:00.000000 +00:00:00".
    const DATE_TIME_FORMAT: &[BorrowedFormatItem] = format_description!(
        "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond] [offset_hour \
             sign:mandatory]:[offset_minute]:[offset_second]"
    );

    pub fn serialize<S>(dt: &OffsetDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let formatted = dt
            .format(DATE_TIME_FORMAT)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&formatted)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        OffsetDateTime::parse(&s, DATE_TIME_FORMAT).map_err(serde::de::Error::custom)
    }
}

/// A token for authorization and authentication.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Token {
    pub user_id: UserID,

    #[serde(
        serialize_with = "datetime_format::serialize",
        deserialize_with = "datetime_format::deserialize"
    )]
    pub expires_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use time::{UtcOffset, macros::datetime};

    use crate::{UserID, auth::token::Token};

    #[test]
    fn serialise_token() {
        let user_id = UserID::new(1);
        let expires_at = datetime!(2025-12-21 03:54:00).assume_offset(UtcOffset::UTC);
        let token = Token {
            user_id,
            expires_at,
        };
        let expected = r#"{"user_id":1,"expires_at":"2025-12-21 03:54:00.0 +00:00:00"}"#;

        let actual = serde_json::to_string(&token).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn deserialise_token() {
        let user_id = UserID::new(1);
        let expires_at = datetime!(2025-12-21 03:54:00).assume_offset(UtcOffset::UTC);
        let expected = Token {
            user_id,
            expires_at,
        };
        let token_string = r#"{"user_id":1,"expires_at":"2025-12-21 03:54:00.0 +00:00:00"}"#;

        let actual = serde_json::from_str(token_string).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn deserialise_token_with_midnight_expiry() {
        let user_id = UserID::new(1);
        let expires_at = datetime!(2025-12-21 00:00:00).assume_offset(UtcOffset::UTC);
        let expected = Token {
            user_id,
            expires_at,
        };
        let token_string = r#"{"user_id":1,"expires_at":"2025-12-21 00:00:00.0 +00:00:00"}"#;

        let actual = serde_json::from_str(token_string).unwrap();

        assert_eq!(expected, actual);
    }
}
