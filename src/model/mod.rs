// TODO: Rename module to 'models'
mod category;
pub use category::{Category, CategoryName, NewCategory};

mod password;
pub use password::{PasswordError, PasswordHash, RawPassword};

mod recurring_transaction;
pub use recurring_transaction::{Frequency, NewRecurringTransaction, RecurringTransaction};

mod savings_ratio;
pub use savings_ratio::{NewSavingsRatio, Ratio, SavingsRatio};

mod transaction;
pub use transaction::{NewTransaction, Transaction};

mod user;
pub use user::{NewUser, User, UserID};

pub type DatabaseID = i64;
