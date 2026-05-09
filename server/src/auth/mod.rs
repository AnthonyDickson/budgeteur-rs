mod cookie;
mod forgot_password;
mod log_in;
mod log_out;
mod middleware;
mod password;
mod redirect;
mod session;
mod token;
mod user;

use cookie::{invalidate_auth_cookie, set_auth_cookie};
pub use forgot_password::get_forgot_password_page;
pub use log_in::{get_log_in_page, post_log_in};
pub use log_out::get_log_out;
pub use middleware::{auth_guard, auth_guard_hx};
pub use password::{PasswordHash, ValidatedPassword};
use redirect::{build_log_in_redirect_url, normalize_redirect_url};
pub use session::{SessionId, SessionStore, start_session_actor};
use token::Token;
pub use user::{User, UserID, create_user_table, get_user_by_id};

#[cfg(test)]
use cookie::COOKIE_TOKEN;
