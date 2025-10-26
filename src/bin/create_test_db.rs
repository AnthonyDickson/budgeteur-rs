use std::error::Error;
use std::path::Path;
use std::process::exit;

use clap::Parser;
use rusqlite::Connection;

use budgeteur_rs::{PasswordHash, ValidatedPassword, initialize_db};

/// A utility for creating a test database for the REST API server of budgeteur_rs.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// File path to save the SQLite database to.
    #[arg(long, short)]
    output_path: String,
}

/// Create and populate a database for manual testing.
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let output_path = Path::new(&args.output_path);

    match output_path.extension() {
        None => {
            eprintln!("Output path must include a file extension (e.g., 'my_database.db').");
            exit(1);
        }
        Some(extension) if extension.is_empty() => {
            eprintln!("Output path must include a file extension (e.g., 'my_database.db').");
            exit(1);
        }
        _ => {}
    }

    if output_path.is_file() {
        eprintln!("File already exists at {output_path:#?}!");
        exit(1);
    }

    println!("Creating database at {output_path:#?}");
    let conn = Connection::open(output_path)?;

    initialize_db(&conn)?;

    println!("Creating test user...");

    let password_hash = PasswordHash::new(
        ValidatedPassword::new_unchecked("test"),
        PasswordHash::DEFAULT_COST,
    )?;

    conn.execute(
        "INSERT INTO user (password) VALUES (?1)",
        (password_hash.to_string(),),
    )?;

    println!("Success!");

    Ok(())
}
