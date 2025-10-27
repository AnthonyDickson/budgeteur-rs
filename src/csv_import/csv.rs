//! Functions to parse CSV data from ASB and Kiwibank bank statements.

use time::{
    Date, OffsetDateTime, UtcOffset, format_description::BorrowedFormatItem,
    macros::format_description,
};

use crate::{
    Error,
    csv_import::balance::ImportBalance,
    transaction::{Transaction, TransactionBuilder},
};

/// The transactions and accounts balances found after parsing a CSV statement.
pub struct ParseCSVResult {
    /// The transactions found in the CSV document, may be empty.
    pub transactions: Vec<TransactionBuilder>,
    /// The account balance found in the CSV document, if found.
    /// ASB credit card CSVs do not provide the balance, for example.
    pub balance: Option<ImportBalance>,
}

/// Parses CSV data from ASB and Kiwibank bank statements.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Dates are assumed to be in local time, `local_timezone` is used for getting the current local time.
///
/// Returns a `ParseCSVResult` which consists of the transactions and account
/// balance found in the CSV data.
/// Returns `Error::InvalidCSV` if the CSV data is not in an accepted format.
pub fn parse_csv(text: &str, local_timezone: UtcOffset) -> Result<ParseCSVResult, Error> {
    let parse_result = parse_asb_bank_csv(text, local_timezone);

    match parse_result {
        Ok(_) => {
            return parse_result;
        }
        Err(error) => {
            tracing::debug!("Could not parse ASB bank statement: {error}");
        }
    }

    let parse_result = parse_asb_cc_csv(text);

    match parse_result {
        Ok(_) => {
            return parse_result;
        }
        Err(error) => {
            tracing::debug!("Could not parse ASB credit card statement: {error}");
        }
    }

    let parse_result = parse_kiwibank_bank_simple_csv(text, local_timezone);

    match parse_result {
        Ok(_) => {
            return parse_result;
        }
        Err(error) => {
            tracing::debug!("Could not parse Kiwibank simple bank statement: {error}");
        }
    }

    Err(Error::InvalidCSV(
        "Could not parse CSV data from ASB or Kiwibank".to_owned(),
    ))
}

/// Parses ASB bank account CSV exported from FastNet Classic.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Dates are assumed to be in local time, `local_timezone` is used for getting the current local time.
///
/// Returns a `ParseCSVResult` which consists of the transactions and account
/// balances found in the CSV data.
/// Returns `Error::InvalidCSV` if the CSV data is not in an accepted format.
fn parse_asb_bank_csv(text: &str, local_timezone: UtcOffset) -> Result<ParseCSVResult, Error> {
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
    const BALANCE_DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[year][month][day]");

    // Parse the header to get the account number
    let lines = text.lines().collect::<Vec<_>>();

    if lines.len() < 8 {
        return Err(Error::InvalidCSV("header too short".to_owned()));
    }

    let mut transactions = Vec::new();
    let mut account = String::new();
    let mut balance = 0.0;
    let mut date = OffsetDateTime::now_utc().to_offset(local_timezone).date();

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
                let account_part = parts[2].strip_prefix(" Account ").ok_or_else(error)?;
                account = [bank, branch, account_part].join("-");
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
            5 => {
                // Example line
                // Ledger Balance : 20.00 as of 20250412
                let balance_string =
                    line.strip_prefix("Ledger Balance : ")
                        .ok_or(Error::InvalidCSV(format!(
                        "ASB bank ledger balance on line 6 should start with 'Ledger Balance : ', but got '{line}'.")
                    ))?;
                let (balance_string, date_string) = balance_string
                    .split_once(' ')
                    .ok_or(Error::InvalidCSV(
                        format!(
                        "ASB bank ledger balance on line 6 should have a space after the balance, but got '{line}'."),
                    ))?;
                balance = balance_string.parse().map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Balance found on line 6 '{balance_string}' cannot be parsed as a float: {error}."
                    ))
                })?;
                let date_string =
                    date_string
                        .split(' ')
                        .next_back()
                        .ok_or(Error::InvalidCSV(format!(
                            "ASB bank ledger should have a date on line 6, but got '{line}'."
                        )))?;
                date = match Date::parse(date_string, &BALANCE_DATE_FORMAT) {
                    Ok(date) => date,
                    Err(error) => {
                        return Err(Error::InvalidCSV(format!(
                            "Could not parse '{date_string}' as date on line 6: {error}"
                        )));
                    }
                }
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

                let transaction = Transaction::build(amount, date, description)
                    .import_id(Some(create_import_id(line)));

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    Ok(ParseCSVResult {
        transactions,
        balance: Some(ImportBalance {
            account,
            balance,
            date,
        }),
    })
}

/// Parses ASB credit card CSV exported from FastNet Classic.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
///
/// Returns a `ParseCSVResult` which consists of the transactions and account
/// balances found in the CSV data.
/// Returns `Error::InvalidCSV` if the CSV data is not in an accepted format.
fn parse_asb_cc_csv(text: &str) -> Result<ParseCSVResult, Error> {
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

    for (line_number, line) in text.lines().enumerate() {
        match line_number {
            0 if !line.starts_with("Created date / time") => {
                return Err(Error::InvalidCSV(
                    "ASB credit card statement missing header 'Created date / time' on line 0"
                        .to_owned(),
                ));
            }
            1 if !line.starts_with("Card Number") => {
                return Err(Error::InvalidCSV(
                    "ASB credit card statement missing header 'Card Number' on line 1".to_owned(),
                ));
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
                // Credit card statements record debits as positive amounts, so
                // we need to negate them to match the other bank statements
                // which represent debits with negative numbers.
                let amount = -amount;

                let transaction = Transaction::build(amount, date, description)
                    .import_id(Some(create_import_id(line)));

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    Ok(ParseCSVResult {
        transactions,
        balance: None,
    })
}

/// Parses simple Kiwibank account CSV exported from form ib.kiwibank.co.nz.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Dates are assumed to be in local time, `local_timezone` is used for getting the current local time.
///
/// Returns a `ParseCSVResult` which consists of the transactions and account
/// balances found in the CSV data.
/// Returns `Error::InvalidCSV` if the CSV data is not in an accepted format.
fn parse_kiwibank_bank_simple_csv(
    text: &str,
    local_timezone: UtcOffset,
) -> Result<ParseCSVResult, Error> {
    const DATE_COLUMN: usize = 0;
    const MEMO_COLUMN: usize = 1;
    const DESCRIPTION_COLUMN: usize = 2;
    const AMOUNT_COLUMN: usize = 3;
    const BALANCE_COLUMN: usize = 4;
    const DATE_FORMAT: &[BorrowedFormatItem] =
        format_description!("[day] [month repr:short] [year]");

    // Parse the header to get the account number
    let lines = text.lines().collect::<Vec<_>>();

    if lines.is_empty() {
        return Err(Error::InvalidCSV("empty CSV file".to_owned()));
    }

    let mut transactions = Vec::new();
    let mut account_number = String::new();
    let mut balance = 0.0;
    let mut date = OffsetDateTime::now_utc().to_offset(local_timezone).date();

    for (line_number, line) in text.lines().enumerate() {
        let parts: Vec<&str> = line.split(',').collect();

        if parts.len() < 5 {
            return Err(Error::InvalidCSV(
                "malformed CSV: expected 5 columns".to_string(),
            ));
        }

        match line_number {
            // 47-8115-1482616-00,,,,
            0 => {
                account_number = parts[0].to_owned();
            }
            // 22 Jan 2025,POS W/D LOBSTER SEAFOO-19:47 ;,,-32.00,168.00
            _ if line_number > 0 => {
                date = Date::parse(parts[DATE_COLUMN], &DATE_FORMAT).map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Could not parse '{}' as date on line {line_number}: {error}",
                        parts[DATE_COLUMN]
                    ))
                })?;

                let description = format!(
                    "{} {}",
                    parts[MEMO_COLUMN].trim().trim_end_matches(" ;"),
                    parts[DESCRIPTION_COLUMN].trim().trim_end_matches(" ;"),
                );

                let amount: f64 = parts[AMOUNT_COLUMN].parse().map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Could not parse '{}' as amount on line {line_number}: {error}",
                        parts[AMOUNT_COLUMN]
                    ))
                })?;

                balance = parts[BALANCE_COLUMN].parse().map_err(|error| {
                    Error::InvalidCSV(format!(
                        "Could not parse '{}' as amount on line {line_number}: {error}",
                        parts[BALANCE_COLUMN]
                    ))
                })?;

                let transaction = Transaction::build(amount, date, &description)
                    .import_id(Some(create_import_id(line)));

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    Ok(ParseCSVResult {
        transactions,
        balance: Some(ImportBalance {
            account: account_number,
            balance,
            date,
        }),
    })
}

/// Creates a hash for a transaction based on the account number, date, description, and amount.
///
/// Not sure how likely collisions are, should be fine ¯\_(ツ)_/¯
pub fn create_import_id(csv_line: &str) -> i64 {
    let hash_128 = md5::compute(csv_line);
    let mut hash_64 = [0; 8];
    hash_64.copy_from_slice(&hash_128[0..8]);
    i64::from_le_bytes(hash_64)
}

#[cfg(test)]
mod parse_csv_tests {
    use time::{UtcOffset, macros::date};

    use crate::{
        csv_import::csv::{
            ImportBalance, ParseCSVResult, create_import_id, parse_asb_bank_csv,
            parse_kiwibank_bank_simple_csv,
        },
        transaction::{Transaction, TransactionBuilder},
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

    const KIWIBANK_BANK_STATEMENT_SIMPLE_CSV: &str = "47-8115-1482616-00,,,,\n\
            22 Jan 2025,TRANSFER TO A R DICKSON - 01 ;,,-353.46,200.00\n\
            22 Jan 2025,POS W/D LOBSTER SEAFOO-19:47 ;,,-32.00,168.00\n\
            22 Jan 2025,TRANSFER FROM A R DICKSON - 01 ;,,32.00,200.00\n\
            26 Jan 2025,POS W/D BEAUTY CHINA -14:02 ;,,-18.00,182.00\n\
            26 Jan 2025,POS W/D LEE HONG BBQ -14:20 ;,,-60.00,122.00\n\
            26 Jan 2025,TRANSFER FROM A R DICKSON - 01 ;,,78.00,200.00";

    #[test]
    fn create_import_id_matching_inputs() {
        assert_eq!(
            create_import_id(
                "38-1234-0123456-01,31-01-2025,INTEREST EARNED ;,,,,,,,,,,0.25,,0.25,71.16",
            ),
            create_import_id(
                "38-1234-0123456-01,31-01-2025,INTEREST EARNED ;,,,,,,,,,,0.25,,0.25,71.16",
            )
        )
    }

    #[test]
    fn create_import_id_different_inputs() {
        assert_ne!(
            create_import_id(
                "38-1234-0123456-01,31-01-2025,INTEREST EARNED ;,,,,,,,,,,0.25,,0.25,71.16",
            ),
            create_import_id(
                "2025/04/10,2025/04/07,2025041001,DEBIT,5023,\"AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)\",10.63"
            )
        );
    }

    #[test]
    fn can_parse_asb_bank_statement() {
        let want_transactions = vec![
            Transaction::build(1300.00, date!(2025 - 01 - 18),  "Credit Card")
                .import_id(Some(create_import_id(
                    "2025/01/18,2025011801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",1300.00"
                ))),
            Transaction::build(-1300.00,date!(2025 - 01 - 18),  "TO CARD 5023  Credit Card")
                .import_id(Some(create_import_id(
                    "2025/01/18,2025011802,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  Credit Card\",-1300.00"
                ))),
           Transaction::build(4400.00,  date!(2025 - 02 - 18), "Credit Card")
                .import_id(Some(create_import_id(
                    "2025/02/18,2025021801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",4400.00"
                ))),
            Transaction::build(-4400.00, date!(2025 - 02 - 19)  ,"TO CARD 5023  THANK YOU")
                .import_id(Some(create_import_id(
                    "2025/02/19,2025021901,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-4400.00"
                ))),
            Transaction::build(2750.00, date!(2025 - 03 - 20), "Credit Card")
                .import_id(Some(create_import_id(
                    "2025/03/20,2025032001,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",2750.00"
                ))),
            Transaction::build(-2750.00, date!(2025 - 03 - 20),  "TO CARD 5023  THANK YOU")
                .import_id(Some(create_import_id(
                    "2025/03/20,2025032002,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-2750.00"
                ))),
        ];
        let want_balance = Some(ImportBalance {
            account: "12-3405-0123456-50 (Streamline)".to_owned(),
            balance: 20.0,
            date: date!(2025 - 04 - 12),
        });

        let ParseCSVResult {
            transactions: got_transactions,
            balance: got_balance,
        } = parse_asb_bank_csv(ASB_BANK_STATEMENT_CSV, UtcOffset::UTC)
            .expect("Could not parse CSV");

        assert_eq!(
            want_transactions.len(),
            got_transactions.len(),
            "want {} transactions, got {}",
            want_transactions.len(),
            got_transactions.len()
        );
        assert_eq!(want_transactions, got_transactions);
        assert_eq!(want_balance, got_balance);
    }

    #[test]
    fn can_parse_asb_cc_statement() {
        let want_transactions = vec![
            TransactionBuilder {
                amount: 2750.00,
                date: date!(2025 - 03 - 20),
                description: "PAYMENT RECEIVED THANK YOU".to_owned(),
                import_id: Some(create_import_id(
                    "2025/03/20,2025/03/20,2025032002,CREDIT,5023,\"PAYMENT RECEIVED THANK YOU\",-2750.00",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: -8.50,
                date: date!(2025 - 04 - 09),
                description: "Birdy Bytes".to_owned(),
                import_id: Some(create_import_id(
                    "2025/04/09,2025/04/08,2025040902,DEBIT,5023,\"Birdy Bytes\",8.50",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: -10.63,
                date: date!(2025 - 04 - 10),
                description:
                    "AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)"
                        .to_owned(),
                import_id: Some(create_import_id(
                    "2025/04/10,2025/04/07,2025041001,DEBIT,5023,\"AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)\",10.63",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: -0.22,
                date: date!(2025 - 04 - 10),
                description: "OFFSHORE SERVICE MARGINS".to_owned(),
                import_id: Some(create_import_id(
                    "2025/04/10,2025/04/07,2025041002,DEBIT,5023,\"OFFSHORE SERVICE MARGINS\",0.22",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: -11.50,
                date: date!(2025 - 04 - 11),
                description: "Buckstars".to_owned(),
                import_id: Some(create_import_id(
                    "2025/04/11,2025/04/10,2025041101,DEBIT,5023,\"Buckstars\",11.50",
                )),
                tag_id: None,
            },
        ];
        let want_balance = None;

        let ParseCSVResult {
            transactions: got_transactions,
            balance: got_balance,
        } = parse_asb_cc_csv(ASB_CC_STATEMENT_CSV).expect("Could not parse CSV");

        assert_eq!(
            want_transactions.len(),
            got_transactions.len(),
            "want {} transactions, got {}",
            want_transactions.len(),
            got_transactions.len()
        );
        assert_eq!(want_transactions, got_transactions);
        assert_eq!(want_balance, got_balance);
    }

    #[test]
    fn can_parse_kiwibank_simple_bank_statement() {
        let want_transactions = vec![
            TransactionBuilder {
                amount: -353.46,
                date: date!(2025 - 01 - 22),
                description: "TRANSFER TO A R DICKSON - 01 ".to_owned(),
                import_id: Some(create_import_id(
                    "22 Jan 2025,TRANSFER TO A R DICKSON - 01 ;,,-353.46,200.00",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: -32.00,
                date: date!(2025 - 01 - 22),
                description: "POS W/D LOBSTER SEAFOO-19:47 ".to_owned(),
                import_id: Some(create_import_id(
                    "22 Jan 2025,POS W/D LOBSTER SEAFOO-19:47 ;,,-32.00,168.00",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: 32.00,
                date: date!(2025 - 01 - 22),
                description: "TRANSFER FROM A R DICKSON - 01 ".to_owned(),
                import_id: Some(create_import_id(
                    "22 Jan 2025,TRANSFER FROM A R DICKSON - 01 ;,,32.00,200.00",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: -18.00,
                date: date!(2025 - 01 - 26),
                description: "POS W/D BEAUTY CHINA -14:02 ".to_owned(),
                import_id: Some(create_import_id(
                    "26 Jan 2025,POS W/D BEAUTY CHINA -14:02 ;,,-18.00,182.00",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: -60.00,
                date: date!(2025 - 01 - 26),
                description: "POS W/D LEE HONG BBQ -14:20 ".to_owned(),
                import_id: Some(create_import_id(
                    "26 Jan 2025,POS W/D LEE HONG BBQ -14:20 ;,,-60.00,122.00",
                )),
                tag_id: None,
            },
            TransactionBuilder {
                amount: 78.00,
                date: date!(2025 - 01 - 26),
                description: "TRANSFER FROM A R DICKSON - 01 ".to_owned(),
                import_id: Some(create_import_id(
                    "26 Jan 2025,TRANSFER FROM A R DICKSON - 01 ;,,78.00,200.00",
                )),
                tag_id: None,
            },
        ];

        let want_balance = Some(ImportBalance {
            account: "47-8115-1482616-00".to_owned(),
            balance: 200.00,
            date: date!(2025 - 01 - 26),
        });

        let ParseCSVResult {
            transactions: got_transactions,
            balance: got_balance,
        } = parse_kiwibank_bank_simple_csv(KIWIBANK_BANK_STATEMENT_SIMPLE_CSV, UtcOffset::UTC)
            .expect("Could not parse CSV");

        assert_eq!(
            want_transactions.len(),
            got_transactions.len(),
            "want {} transactions, got {}",
            want_transactions.len(),
            got_transactions.len()
        );
        for (want, got) in want_transactions.iter().zip(&got_transactions) {
            assert_eq!(want, got);
        }
        assert_eq!(want_transactions, got_transactions);
        assert_eq!(want_balance, got_balance);
    }
}
