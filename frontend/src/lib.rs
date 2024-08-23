use common::User;

#[derive(Clone, Debug, PartialEq)]
pub struct AppContext {
    pub current_user: Option<User>,
    pub token: Option<String>,
}
