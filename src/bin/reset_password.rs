use std::{
    error::Error,
    io::{self},
    path::Path,
    process::exit,
};

use bcrypt::DEFAULT_COST;
use budgeteur_rs::user::{User, UserID, get_user_by_id};
use clap::Parser;
use rusqlite::Connection;

use budgeteur_rs::{PasswordHash, ValidatedPassword};

/// A utility for changing the password for a registered user.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// File path to the application SQLite database.
    #[arg(long)]
    db_path: String,
}

/// Create and populate a database for manual testing.
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let db_path = Path::new(&args.db_path);
    validate_db_path(db_path);

    let user = get_user(db_path);
    println!("Resetting password for {}", user.email);

    let password_hash = match get_new_password_hash() {
        Some(password_hash) => password_hash,
        None => return Ok(()),
    };
    update_password(db_path, user, password_hash)?;

    Ok(())
}

fn get_user(db_path: &Path) -> User {
    println!("Loading user from from {db_path:#?}");

    let conn = Connection::open(db_path)
        .unwrap_or_else(|_| panic!("Could not open the database at {db_path:?}"));

    get_user_by_id(UserID::new(1), &conn).expect("Could not get user with ID=1 in {db_path}.")
}

fn validate_db_path(db_path: &Path) {
    match db_path.extension() {
        None => {
            print_error("Database path must include a file extension (e.g., 'my_database.db').");
            exit(1);
        }
        Some(extension) if extension.is_empty() => {
            print_error("Database path must include a file extension (e.g., 'my_database.db').");
            exit(1);
        }
        _ => {}
    }

    if !db_path.is_file() {
        eprintln!("File does not exist at {db_path:#?}!");
        exit(1);
    }
}

fn get_new_password_hash() -> Option<PasswordHash> {
    loop {
        println!();

        let first_password = match rpassword::prompt_password("Enter a new password: ") {
            Ok(string) => string,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return None;
            }
            Err(error) => {
                print_error(format!("Could not read password from stdin: {error}"));
                return None;
            }
        };

        if let Err(error) = ValidatedPassword::new(&first_password) {
            print_error(error);
            continue;
        }

        let second_password = match rpassword::prompt_password("Enter the same password again: ") {
            Ok(string) => string,
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return None;
            }
            Err(error) => {
                print_error(format!("Could not read password from stdin: {error}"));
                return None;
            }
        };

        if first_password != second_password {
            print_error("Passwords must match, try again.");
            continue;
        }

        let password_hash = match PasswordHash::from_raw_password(&first_password, DEFAULT_COST) {
            Ok(password_hash) => password_hash,
            Err(error) => {
                print_error(format!("Could not hash password: {error}. Try again."));
                continue;
            }
        };

        return Some(password_hash);
    }
}

fn print_error(error: impl ToString) {
    eprintln!(
        "\x1b[31;1m{}\x1b[0m",
        capitalise_first_char(&error.to_string())
    )
}

/// From https://crates.io/crates/capitalize
fn capitalise_first_char(string: &str) -> String {
    let mut chars = string.chars();
    let Some(first) = chars.next() else {
        return String::with_capacity(0);
    };
    first.to_uppercase().chain(chars).collect()
}

fn update_password(
    db_path: &Path,
    user: User,
    password: PasswordHash,
) -> Result<(), rusqlite::Error> {
    let mut conn = Connection::open(db_path)?;
    let transaction = conn.transaction()?;

    let rows_affected = transaction.execute(
        "UPDATE user SET password = ?1 WHERE user.id = ?2;",
        (&password.to_string(), &user.id.as_i64()),
    )?;

    if rows_affected != 1 {
        print_error(format!(
            "Updating password affected {rows_affected} user(s), expected 1. Rolling back..."
        ));
        transaction.rollback()?;
        return Err(rusqlite::Error::StatementChangedRows(rows_affected));
    }

    transaction.commit()?;

    println!("Password updated successfully!");

    Ok(())
}
