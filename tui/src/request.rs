use budgeteur_shared::auth::{TUI_CLIENT_SUB, TuiClaims};

#[derive(Clone)]
pub struct RequestContext {
    pub base_url: String,
    pub signing_key_der: Vec<u8>,
}

const TOKEN_EXPIRY_SECONDS: usize = 300;

pub fn sign_auth_header(signing_key_der: &[u8]) -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock error: {e}"))?
        .as_secs() as usize;

    let claims = TuiClaims {
        sub: TUI_CLIENT_SUB.into(),
        iat: now,
        exp: now + TOKEN_EXPIRY_SECONDS,
    };

    let encoding_key = jsonwebtoken::EncodingKey::from_ed_der(signing_key_der);

    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::EdDSA),
        &claims,
        &encoding_key,
    )
    .map_err(|e| format!("could not sign JWT: {e}"))?;

    Ok(format!("Bearer {token}"))
}
