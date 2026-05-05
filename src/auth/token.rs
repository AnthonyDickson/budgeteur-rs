//! Defines the token struct used in the auth cookies.

use serde::{Deserialize, Serialize};

use crate::auth::SessionId;

/// A token for authorization and authentication.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Token {
    pub session_id: SessionId,
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::auth::token::Token;

    #[test]
    fn serialise_token() {
        let session_id = Uuid::parse_str("6c84fb90-12c4-11e1-840d-7b25c5ee775a").unwrap();
        let token = Token { session_id };
        let expected = r#"{"session_id":"6c84fb90-12c4-11e1-840d-7b25c5ee775a"}"#;

        let actual = serde_json::to_string(&token).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn deserialise_token() {
        let token_string = r#"{"session_id":"6c84fb90-12c4-11e1-840d-7b25c5ee775a"}"#;

        let actual: Token = serde_json::from_str(token_string).unwrap();

        assert_eq!(
            actual.session_id,
            Uuid::parse_str("6c84fb90-12c4-11e1-840d-7b25c5ee775a").unwrap()
        );
    }
}
