use serde::{Deserialize, Serialize};

pub const TUI_CLIENT_SUB: &str = "tui-client";

#[derive(Debug, Serialize, Deserialize)]
pub struct TuiClaims {
    pub sub: String,
    pub iat: usize,
    pub exp: usize,
}
