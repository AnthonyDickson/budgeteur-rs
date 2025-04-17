//! Functions to parse CSV data from ASB and Kiwibank bank statements.

use time::{Date, format_description::BorrowedFormatItem, macros::format_description};

use crate::{
    Error,
    models::{TransactionBuilder, UserID},
};

/// Parses CSV data from ASB and Kiwibank bank statements.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Uses `user_id` to set the user ID for the transactions.
///
/// Returns a vector of `Transaction` objects found in the CSV data or an empty vector if no
/// transactions were found.
pub fn parse_csv(text: &str, user_id: UserID) -> Result<Vec<TransactionBuilder>, Error> {
    let transactions = parse_asb_bank_csv(text, user_id);

    match transactions {
        Ok(_) => {
            return transactions;
        }
        Err(error) => {
            tracing::debug!("Could not parse ASB bank statement: {error}");
        }
    }

    let transactions = parse_asb_cc_csv(text, user_id);

    match transactions {
        Ok(_) => {
            return transactions;
        }
        Err(error) => {
            tracing::debug!("Could not parse ASB credit card statement: {error}");
        }
    }

    let transactions = parse_kiwibank_bank_csv(text, user_id);

    match transactions {
        Ok(_) => {
            return transactions;
        }
        Err(error) => {
            tracing::debug!("Could not parse Kiwibank bank statement: {error}");
        }
    }

    Err(Error::InvalidCSV(
        "Could not parse CSV data from ASB or Kiwibank".to_owned(),
    ))
}

/// Parses ASB bank account CSV exported from FastNet Classic.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Uses `user_id` to set the user ID for the transactions.
///
/// Returns a vector of `Transaction` objects found in the CSV data or an empty vector if no
/// transactions were found.
fn parse_asb_bank_csv(text: &str, user_id: UserID) -> Result<Vec<TransactionBuilder>, Error> {
    // Header looks like:
    // Created date / time : 12 April 2025 / 11:10:19
    // Bank 12; Branch 3405; Account 0123456-50 (Streamline)
    // From date 20250101
    // To date 20250412
    // Avail Bal : 1020.00 as of 20250320
    // Ledger Balance : 20.00 as of 20250412
    // Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount
    //

    const DATE_COLUMN: usize = 0;
    const DESCRIPTION_COLUMN: usize = 5;
    const AMOUNT_COLUMN: usize = 6;
    const DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[year]/[month]/[day]");

    // Parse the header to get the account number
    let lines = text.lines().collect::<Vec<_>>();

    if lines.len() < 8 {
        return Err(Error::InvalidCSV("header too short".to_owned()));
    }

    let mut transactions = Vec::new();
    let mut account_number = String::new();

    for (line_number, line) in text.lines().enumerate() {
        match line_number {
            0 if !line.starts_with("Created date / time") => {
                return Err(Error::InvalidCSV(
                    "ASB bank statement missing header 'Created date / time' on line 0".to_owned(),
                ));
            }
            1 => {
                // Bank 12; Branch 3456; Account 0123456-50 (Streamline)
                let parts = line.split(';').collect::<Vec<_>>();
                let error = || {
                    Error::InvalidCSV(
                    "ASB bank statement missing header with format 'Bank XX; Branch XXXX; Account XXXXXXX-XX (Acount Name)' on line 1".to_owned(),
                )
                };

                let bank = parts[0].strip_prefix("Bank ").ok_or_else(error)?;
                let branch = parts[1].strip_prefix(" Branch ").ok_or_else(error)?;
                let account = parts[2].strip_prefix(" Account ").ok_or_else(error)?;
                account_number = [bank, branch, account].join("-");
            }
            2 if !line.starts_with("From date ") => {
                return Err(Error::InvalidCSV(
                    "ASB bank statement missing header 'From date' on line 2".to_owned(),
                ));
            }
            3 if !line.starts_with("To date ") => {
                return Err(Error::InvalidCSV(
                    "ASB bank statement missing header 'To date' on line 3".to_owned(),
                ));
            }
            4 if !line.starts_with("Avail Bal") => {
                return Err(Error::InvalidCSV(
                    "ASB bank statement missing header 'Avail Bal' on line 4".to_owned(),
                ));
            }
            5 if !line.starts_with("Ledger Balance") => {
                return Err(Error::InvalidCSV(
                    "ASB bank statement missing header 'Ledger Balance' on line 5".to_owned(),
                ));
            }
            6 if line != "Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount" => {
                return Err(Error::InvalidCSV(
                    "ASB bank statement missing header 'Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount' on line 6".to_owned(),
                ));
            }
            7 if !line.is_empty() => {
                return Err(Error::InvalidCSV(
                    "ASB bank statement missing empty line on line 7".to_owned(),
                ));
            }
            _ if line_number > 7 => {
                let parts: Vec<&str> = line.split(',').collect();

                if parts.len() < 7 {
                    continue;
                }

                let date = match Date::parse(parts[DATE_COLUMN], &DATE_FORMAT) {
                    Ok(date) => date,
                    Err(error) => {
                        return Err(Error::InvalidCSV(format!(
                            "Could not parse '{}' as date on line {line_number}: {error}",
                            parts[DATE_COLUMN]
                        )));
                    }
                };
                let description = parts[DESCRIPTION_COLUMN];
                let description = description.trim_matches('"');
                let amount: f64 = parts[AMOUNT_COLUMN].parse().map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Could not parse '{}' as amount on line {line_number}: {error}",
                        parts[AMOUNT_COLUMN]
                    ))
                })?;

                let transaction = TransactionBuilder::new(amount, user_id)
                    .date(date)
                    .map_err(|error| {
                        Error::InvalidCSV(format!(
                            "Date '{}' on line {line_number} is invalid: {error}",
                            parts[DATE_COLUMN]
                        ))
                    })?
                    .description(description)
                    .import_id(Some(create_import_id(
                        &account_number,
                        date,
                        description,
                        amount,
                    )));

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    Ok(transactions)
}

/// Parses ASB credit card CSV exported from FastNet Classic.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Uses `user_id` to set the user ID for the transactions.
///
/// Returns a vector of `Transaction` objects found in the CSV data or an empty vector if no
/// transactions were found.
fn parse_asb_cc_csv(text: &str, user_id: UserID) -> Result<Vec<TransactionBuilder>, Error> {
    // Header looks like:
    // Created date / time : 12 April 2025 / 11:09:26
    // Card Number XXXX-XXXX-XXXX-5023 (Visa Light)
    // From date 20250101
    // To date 20250412
    // Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount
    //
    const DATE_COLUMN: usize = 0;
    const DESCRIPTION_COLUMN: usize = 5;
    const AMOUNT_COLUMN: usize = 6;
    const DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[year]/[month]/[day]");

    // Parse the header to get the account number
    let lines = text.lines().collect::<Vec<_>>();

    if lines.len() < 6 {
        return Err(Error::InvalidCSV("header too short".to_owned()));
    }

    let mut transactions = Vec::new();
    let mut account_number = String::new();

    for (line_number, line) in text.lines().enumerate() {
        match line_number {
            0 if !line.starts_with("Created date / time") => {
                return Err(Error::InvalidCSV(
                    "ASB credit card statement missing header 'Created date / time' on line 0"
                        .to_owned(),
                ));
            }
            1 => {
                account_number = line
                    .strip_prefix("Card Number ")
                    .ok_or_else(|| {
                        Error::InvalidCSV(
                            "ASB credit card statement missing header 'Card Number' on line 1"
                                .to_owned(),
                        )
                    })?
                    .to_string();
            }
            2 if !line.starts_with("From date ") => {
                return Err(Error::InvalidCSV(
                    "ASB credit card statement missing header 'From date' on line 2".to_owned(),
                ));
            }
            3 if !line.starts_with("To date ") => {
                return Err(Error::InvalidCSV(
                    "ASB credit card statement missing header 'To date' on line 3".to_owned(),
                ));
            }
            4 if line
                != "Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount" =>
            {
                return Err(Error::InvalidCSV(
                    "ASB credit card statement missing header 'Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount' on line 4".to_owned(),
                ));
            }
            5 if !line.is_empty() => {
                return Err(Error::InvalidCSV(
                    "ASB credit card statement missing empty line on line 5".to_owned(),
                ));
            }
            _ if line_number > 5 => {
                let parts: Vec<&str> = line.split(',').collect();

                if parts.len() < 7 {
                    continue;
                }

                let date = Date::parse(parts[DATE_COLUMN], &DATE_FORMAT).map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Could not parse '{}' as date on line {line_number}: {error}",
                        parts[DATE_COLUMN]
                    ))
                })?;
                let description = parts[DESCRIPTION_COLUMN];
                let description = description.trim_matches('"');
                let amount: f64 = parts[AMOUNT_COLUMN].parse().map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Could not parse '{}' as amount on line {line_number}: {error}",
                        parts[AMOUNT_COLUMN]
                    ))
                })?;

                let transaction = TransactionBuilder::new(amount, user_id)
                    .date(date)
                    .map_err(|error| {
                        Error::InvalidCSV(format!(
                            "Date '{}' on line {line_number} is invalid: {error}",
                            parts[DATE_COLUMN]
                        ))
                    })?
                    .description(description)
                    .import_id(Some(create_import_id(
                        &account_number,
                        date,
                        description,
                        amount,
                    )));

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    Ok(transactions)
}

/// Parses detailed Kiwibank account CSV exported from form ib.kiwibank.co.nz.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Uses `user_id` to set the user ID for the transactions.
///
/// Returns a vector of `Transaction` objects found in the CSV data or an empty vector if no
/// transactions were found.
fn parse_kiwibank_bank_csv(text: &str, user_id: UserID) -> Result<Vec<TransactionBuilder>, Error> {
    // Header looks like:
    // Account number,Date,Memo/Description,Source Code (payment type),TP ref,TP part,TP code,OP ref,OP part,OP code,OP name,OP Bank Account Number,Amount (credit),Amount (debit),Amount,Balance
    const ACCOUNT_NUMBER_COLUMN: usize = 0;
    const DATE_COLUMN: usize = 1;
    const DESCRIPTION_COLUMN: usize = 2;
    const AMOUNT_COLUMN: usize = 14;
    const DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[day]-[month]-[year]");

    // Parse the header to get the account number
    let lines = text.lines().collect::<Vec<_>>();

    if lines.is_empty() {
        return Err(Error::InvalidCSV("header too short".to_owned()));
    }

    let mut transactions = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        match line_number {
            0 if line
                != "Account number,Date,Memo/Description,Source Code (payment type),TP ref,TP part,TP code,OP ref,OP part,OP code,OP name,OP Bank Account Number,Amount (credit),Amount (debit),Amount,Balance" =>
            {
                return Err(Error::InvalidCSV(
                    "Kiwibank bank statement missing header 'Account number,Date,Memo/Description,Source Code (payment type),TP ref,TP part,TP code,OP ref,OP part,OP code,OP name,OP Bank Account Number,Amount (credit),Amount (debit),Amount,Balance' on line 0"
                        .to_owned(),
                ));
            }
            _ if line_number > 0 => {
                let parts: Vec<&str> = line.split(',').collect();

                if parts.len() < 16 {
                    continue;
                }

                let account_number = parts[ACCOUNT_NUMBER_COLUMN];
                let date = Date::parse(parts[DATE_COLUMN], &DATE_FORMAT).map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Could not parse '{}' as date on line {line_number}: {error}",
                        parts[DATE_COLUMN]
                    ))
                })?;
                let description = parts[DESCRIPTION_COLUMN];
                let description = description.trim_matches('"');
                let description = description.trim_end_matches(" ;");
                let amount: f64 = parts[AMOUNT_COLUMN].parse().map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Could not parse '{}' as amount on line {line_number}: {error}",
                        parts[AMOUNT_COLUMN]
                    ))
                })?;

                let transaction = TransactionBuilder::new(amount, user_id)
                    .date(date)
                    .map_err(|error| {
                        Error::InvalidCSV(format!(
                            "Date '{}' on line {line_number} is invalid: {error}",
                            parts[DATE_COLUMN]
                        ))
                    })?
                    .description(description)
                    .import_id(Some(create_import_id(
                        account_number,
                        date,
                        description,
                        amount,
                    )));

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    Ok(transactions)
}

/// Creates a hash for a transaction based on the account number, date, description, and amount.
///
/// Not sure how likely collisions are, should be fine ¯\_(ツ)_/¯
fn create_import_id(account_number: &str, date: Date, description: &str, amount: f64) -> i64 {
    let (year, day) = date.to_ordinal_date();

    let mut bytes: Vec<u8> = [
        account_number.as_bytes(),
        &year.to_le_bytes(),
        &day.to_le_bytes(),
        description.as_bytes(),
        &amount.to_le_bytes(),
    ]
    .concat()
    .into_iter()
    .collect();

    while bytes.len() % 8 != 0 {
        bytes.push(0);
    }

    let mut hash: i64 = 0;

    for i in 0..(bytes.len() / 8) {
        let start = i * 8;
        let end = start + 8;
        let chunk = &bytes[start..end];

        let mut chunk_bytes: [u8; 8] = [0; 8];
        chunk_bytes.copy_from_slice(chunk);
        let bit_set = i64::from_le_bytes(chunk_bytes);

        hash ^= bit_set;
        hash = hash.wrapping_mul(0x5bd1e995);
        hash ^= hash >> 15;
        hash = hash.wrapping_mul(0x5bd1e995);
        hash ^= hash >> 13;
    }

    hash
}

#[cfg(test)]
mod parse_csv_tests {
    use time::macros::date;

    use crate::{
        csv::{create_import_id, parse_asb_bank_csv, parse_kiwibank_bank_csv},
        models::{TransactionBuilder, UserID},
    };

    use super::parse_asb_cc_csv;

    const ASB_BANK_STATEMENT_CSV: &str = "Created date / time : 12 April 2025 / 11:10:19\n\
        Bank 12; Branch 3405; Account 0123456-50 (Streamline)\n\
        From date 20250101\n\
        To date 20250412\n\
        Avail Bal : 1020.00 as of 20250320\n\
        Ledger Balance : 20.00 as of 20250412\n\
        Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount\n\
        \n\
        2025/01/18,2025011801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",1300.00\n\
        2025/01/18,2025011802,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  Credit Card\",-1300.00\n\
        2025/02/18,2025021801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",4400.00\n\
        2025/02/19,2025021901,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-4400.00\n\
        2025/03/20,2025032001,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",2750.00\n\
        2025/03/20,2025032002,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-2750.00";

    const ASB_CC_STATEMENT_CSV: &str = "Created date / time : 12 April 2025 / 11:09:26\n\
        Card Number XXXX-XXXX-XXXX-5023 (Visa Light)\n\
        From date 20250101\n\
        To date 20250412\n\
        Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount\n\
        \n\
        2025/03/20,2025/03/20,2025032002,CREDIT,5023,\"PAYMENT RECEIVED THANK YOU\",-2750.00\n\
        2025/04/09,2025/04/08,2025040902,DEBIT,5023,\"Birdy Bytes\",8.50\n\
        2025/04/10,2025/04/07,2025041001,DEBIT,5023,\"AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)\",10.63\n\
        2025/04/10,2025/04/07,2025041002,DEBIT,5023,\"OFFSHORE SERVICE MARGINS\",0.22\n\
        2025/04/11,2025/04/10,2025041101,DEBIT,5023,\"Buckstars\",11.50";

    const KIWIBANK_BANK_STATEMENT_CSV: &str = "Account number,Date,Memo/Description,Source Code (payment type),TP ref,TP part,TP code,OP ref,OP part,OP code,OP name,OP Bank Account Number,Amount (credit),Amount (debit),Amount,Balance\n\
        38-1234-0123456-01,31-01-2025,INTEREST EARNED ;,,,,,,,,,,0.25,,0.25,71.16\n\
        38-1234-0123456-01,31-01-2025,PIE TAX 10.500% ;,,,,,,,,,,,0.03,-0.03,71.13\n\
        38-1234-0123456-01,28-02-2025,INTEREST EARNED ;,,,,,,,,,,0.22,,0.22,71.35\n\
        38-1234-0123456-01,28-02-2025,PIE TAX 10.500% ;,,,,,,,,,,,0.02,-0.02,71.33\n\
        38-1234-0123456-01,31-03-2025,INTEREST EARNED ;,,,,,,,,,,0.22,,0.22,71.55\n\
        38-1234-0123456-01,31-03-2025,PIE TAX 10.500% ;,,,,,,,,,,,0.02,-0.02,71.53";

    #[test]
    fn create_import_id_matching_inputs() {
        assert_eq!(
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.00
            ),
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.00
            )
        )
    }

    #[test]
    fn create_import_id_different_amounts() {
        assert_ne!(
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.00
            ),
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.01
            )
        );
    }

    #[test]
    fn create_import_id_different_dates() {
        assert_ne!(
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.00
            ),
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 19),
                "Credit Card",
                1300.00
            )
        );
    }

    #[test]
    fn create_import_id_different_descriptions() {
        assert_ne!(
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.00
            ),
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card 2",
                1300.00
            )
        );
    }

    #[test]
    fn create_import_id_different_account_numbers() {
        assert_ne!(
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.00
            ),
            create_import_id(
                "12-3405-0123456-51 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.00
            )
        );
    }

    #[test]
    fn create_import_id_different_everything() {
        assert_ne!(
            create_import_id(
                "12-3405-0123456-50 (Streamline)",
                date!(2025 - 01 - 18),
                "Credit Card",
                1300.00
            ),
            create_import_id(
                "12-3405-0123456-51 (Streamline)",
                date!(2025 - 01 - 19),
                "Credit Card 2",
                1300.01
            )
        );
    }

    #[test]
    fn can_parse_asb_bank_statement() {
        let user_id = UserID::new(42);
        let want = vec![
            TransactionBuilder::new(1300.00, user_id)
                .date(date!(2025 - 01 - 18))
                .expect("Could not set date")
                .description("Credit Card")
                .import_id(Some(create_import_id(
                    "12-3405-0123456-50 (Streamline)",
                    date!(2025 - 01 - 18),
                    "Credit Card",
                    1300.00,
                ))),
            TransactionBuilder::new(-1300.00, user_id)
                .date(date!(2025 - 01 - 18))
                .expect("Could not set date")
                .description("TO CARD 5023  Credit Card")
                .import_id(Some(create_import_id(
                    "12-3405-0123456-50 (Streamline)",
                    date!(2025 - 01 - 18),
                    "TO CARD 5023  Credit Card",
                    -1300.00,
                ))),
            TransactionBuilder::new(4400.00, user_id)
                .date(date!(2025 - 02 - 18))
                .expect("Could not set date")
                .description("Credit Card")
                .import_id(Some(create_import_id(
                    "12-3405-0123456-50 (Streamline)",
                    date!(2025 - 02 - 18),
                    "Credit Card",
                    4400.00,
                ))),
            TransactionBuilder::new(-4400.00, user_id)
                .date(date!(2025 - 02 - 19))
                .expect("Could not set date")
                .description("TO CARD 5023  THANK YOU")
                .import_id(Some(create_import_id(
                    "12-3405-0123456-50 (Streamline)",
                    date!(2025 - 02 - 19),
                    "TO CARD 5023  THANK YOU",
                    -4400.00,
                ))),
            TransactionBuilder::new(2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not set date")
                .description("Credit Card")
                .import_id(Some(create_import_id(
                    "12-3405-0123456-50 (Streamline)",
                    date!(2025 - 03 - 20),
                    "Credit Card",
                    2750.00,
                ))),
            TransactionBuilder::new(-2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not set date")
                .description("TO CARD 5023  THANK YOU")
                .import_id(Some(create_import_id(
                    "12-3405-0123456-50 (Streamline)",
                    date!(2025 - 03 - 20),
                    "TO CARD 5023  THANK YOU",
                    -2750.00,
                ))),
        ];

        let result =
            parse_asb_bank_csv(ASB_BANK_STATEMENT_CSV, user_id).expect("Could not parse CSV");

        assert_eq!(
            want.len(),
            result.len(),
            "want {} transactions, got {}",
            want.len(),
            result.len()
        );
        assert_eq!(want, result);
    }

    #[test]
    fn can_parse_asb_cc_statement() {
        let user_id = UserID::new(42);
        let want = vec![
            TransactionBuilder::new(-2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not parse date")
                .description("PAYMENT RECEIVED THANK YOU")
                .import_id(Some(create_import_id(
                    "XXXX-XXXX-XXXX-5023 (Visa Light)",
                    date!(2025 - 03 - 20),
                    "PAYMENT RECEIVED THANK YOU",
                    -2750.00,
                ))),
            TransactionBuilder::new(8.50, user_id)
                .date(date!(2025 - 04 - 09))
                .expect("Could not parse date")
                .description("Birdy Bytes")
                .import_id(Some(create_import_id(
                    "XXXX-XXXX-XXXX-5023 (Visa Light)",
                    date!(2025 - 04 - 09),
                    "Birdy Bytes",
                    8.50,
                ))),
            TransactionBuilder::new(10.63, user_id)
                .date(date!(2025 - 04 - 10))
                .expect("Could not parse date")
                .description(
                    "AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)",
                )
                .import_id(Some(create_import_id(
                    "XXXX-XXXX-XXXX-5023 (Visa Light)",
                    date!(2025 - 04 - 10),
                    "AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)",
                    10.63,
                ))),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 04 - 10))
                .expect("Could not parse date")
                .description("OFFSHORE SERVICE MARGINS")
                .import_id(Some(create_import_id(
                    "XXXX-XXXX-XXXX-5023 (Visa Light)",
                    date!(2025 - 04 - 10),
                    "OFFSHORE SERVICE MARGINS",
                    0.22,
                ))),
            TransactionBuilder::new(11.50, user_id)
                .date(date!(2025 - 04 - 11))
                .expect("Could not parse date")
                .description("Buckstars")
                .import_id(Some(create_import_id(
                    "XXXX-XXXX-XXXX-5023 (Visa Light)",
                    date!(2025 - 04 - 11),
                    "Buckstars",
                    11.50,
                ))),
        ];

        let result = parse_asb_cc_csv(ASB_CC_STATEMENT_CSV, user_id).expect("Could not parse CSV");

        assert_eq!(
            want.len(),
            result.len(),
            "want {} transactions, got {}",
            want.len(),
            result.len()
        );
        assert_eq!(want, result);
    }

    #[test]
    fn can_parse_kiwibank_bank_statement() {
        let user_id = UserID::new(42);
        let want = vec![
            TransactionBuilder::new(0.25, user_id)
                .date(date!(2025 - 01 - 31))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .import_id(Some(create_import_id(
                    "38-1234-0123456-01",
                    date!(2025 - 01 - 31),
                    "INTEREST EARNED",
                    0.25,
                ))),
            TransactionBuilder::new(-0.03, user_id)
                .date(date!(2025 - 01 - 31))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .import_id(Some(create_import_id(
                    "38-1234-0123456-01",
                    date!(2025 - 01 - 31),
                    "PIE TAX 10.500%",
                    -0.03,
                ))),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 02 - 28))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .import_id(Some(create_import_id(
                    "38-1234-0123456-01",
                    date!(2025 - 02 - 28),
                    "INTEREST EARNED",
                    0.22,
                ))),
            TransactionBuilder::new(-0.02, user_id)
                .date(date!(2025 - 02 - 28))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .import_id(Some(create_import_id(
                    "38-1234-0123456-01",
                    date!(2025 - 02 - 28),
                    "PIE TAX 10.500%",
                    -0.02,
                ))),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 03 - 31))
                .expect("Could not parse date")
                .description("INTEREST EARNED")
                .import_id(Some(create_import_id(
                    "38-1234-0123456-01",
                    date!(2025 - 03 - 31),
                    "INTEREST EARNED",
                    0.22,
                ))),
            TransactionBuilder::new(-0.02, user_id)
                .date(date!(2025 - 03 - 31))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%")
                .import_id(Some(create_import_id(
                    "38-1234-0123456-01",
                    date!(2025 - 03 - 31),
                    "PIE TAX 10.500%",
                    -0.02,
                ))),
        ];

        let result = parse_kiwibank_bank_csv(KIWIBANK_BANK_STATEMENT_CSV, user_id)
            .expect("Could not parse CSV");

        assert_eq!(
            want.len(),
            result.len(),
            "want {} transactions, got {}",
            want.len(),
            result.len()
        );
        assert_eq!(want, result);
    }
}
