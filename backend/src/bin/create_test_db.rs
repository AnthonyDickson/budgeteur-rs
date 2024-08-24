use std::env;
use std::error::Error;
use std::path::Path;
use std::process::exit;

use common::{PasswordHash, RawPassword};
use rusqlite::Connection;

use backend::db::initialize;

/// Create and populate a database for manual testing.
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <output_path>", &args[0]);
        exit(1);
    }

    let output_path = Path::new(&args[1]);

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

    initialize(&conn)?;

    println!("Creating test user...");

    let password_hash =
        PasswordHash::new(unsafe { RawPassword::new_unchecked("test".to_owned()) })?;

    conn.execute(
        "INSERT INTO user (email, password) VALUES (?1, ?2)",
        (&"test@test.com", password_hash.to_string()),
    )?;

    Ok(())
}
