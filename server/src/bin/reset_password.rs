use std::{
    error::Error,
    io::{self, Write},
    path::Path,
    process::exit,
};

use bcrypt::DEFAULT_COST;
use clap::Parser;
use rusqlite::Connection;

use budgeteur_rs::{PasswordHash, ValidatedPassword, initialize_db};

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

    if !db_path.is_file() {
        eprintln!("File does not exist at {db_path:#?}!");
        if ask_to_create_db(db_path) {
            create_db(db_path);
        } else {
            exit(0);
        }
    }

    let password_hash = match get_new_password_hash() {
        Some(password_hash) => password_hash,
        None => return Ok(()),
    };
    update_password(db_path, password_hash)?;

    Ok(())
}

fn ask_to_create_db(db_path: &Path) -> bool {
    print!("Create a new database at {db_path:#?}? (Y/n) ");
    std::io::stdout().flush().expect("Failed to flush stdout");

    let mut response = String::new();
    std::io::stdin()
        .read_line(&mut response)
        .expect("Failed to read stdin");

    matches!(response.trim(), "y" | "Y")
}

fn create_db(db_path: &Path) {
    let conn = Connection::open(db_path)
        .unwrap_or_else(|error| panic!("Could not open database file at {db_path:#?}: {error:?}"));

    if let Err(error) = initialize_db(&conn) {
        eprintln!("Could not initialize database: {error}");
        exit(1);
    }
}

fn get_new_password_hash() -> Option<PasswordHash> {
    loop {
        println!();

        let first_password = rpassword::prompt_password("Enter a new password: ")
            .inspect_err(|error| {
                if error.kind() != io::ErrorKind::UnexpectedEof {
                    print_error(format!("Could not read password from stdin: {error}"));
                }
            })
            .ok()?;

        if let Err(error) = ValidatedPassword::new(&first_password) {
            print_error(error);
            continue;
        }

        let second_password = rpassword::prompt_password("Enter the same password again: ")
            .inspect_err(|error| {
                if error.kind() != io::ErrorKind::UnexpectedEof {
                    print_error(format!("Could not read password from stdin: {error}"));
                }
            })
            .ok()?;

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

/// From <https://crates.io/crates/capitalize>
fn capitalise_first_char(string: &str) -> String {
    let mut chars = string.chars();
    let Some(first) = chars.next() else {
        return String::with_capacity(0);
    };
    first.to_uppercase().chain(chars).collect()
}

fn update_password(db_path: &Path, password: PasswordHash) -> Result<(), rusqlite::Error> {
    let mut conn = Connection::open(db_path)?;
    let transaction = conn.transaction()?;

    let rows_affected = transaction.execute(
        "INSERT OR REPLACE INTO user (id, password) VALUES (?1, ?2);",
        (1, &password.to_string()),
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
