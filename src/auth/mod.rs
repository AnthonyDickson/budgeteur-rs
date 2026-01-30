mod cookie;
mod forgot_password;
mod log_in;
mod log_out;
mod middleware;
mod password;
mod register_user;
mod token;
mod user;

pub use cookie::{DEFAULT_COOKIE_DURATION, invalidate_auth_cookie, set_auth_cookie};
pub use forgot_password::get_forgot_password_page;
pub use log_in::{get_log_in_page, post_log_in};
pub use log_out::get_log_out;
pub use middleware::{auth_guard, auth_guard_hx};
pub use password::{PasswordHash, ValidatedPassword};
pub use register_user::{get_register_page, register_user};
pub(super) use token::Token;
pub use user::{User, UserID, create_user_table, get_user_by_id};
pub(super) use user::{count_users, create_user};

#[cfg(test)]
pub use cookie::COOKIE_TOKEN;

#[cfg(test)]
pub use middleware::AuthState;
