use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TuiClaims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}
