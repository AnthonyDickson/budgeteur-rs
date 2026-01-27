//! Functions to parse CSV data from ASB and Kiwibank bank statements.

use time::{Date, format_description::BorrowedFormatItem, macros::format_description};

use crate::{
    Error,
    csv_import::account::ImportAccount,
    transaction::{Transaction, TransactionBuilder},
};

/// The transactions and accounts found after parsing a CSV statement.
pub struct ParseCSVResult {
    /// The transactions found in the CSV document, may be empty.
    pub transactions: Vec<TransactionBuilder>,
    /// The account found in the CSV document, if found.
    /// ASB credit card CSVs do not provide the balance, for example.
    pub account: Option<ImportAccount>,
}

/// Configuration for parsing CSV transaction rows.
struct TransactionParseConfig<'a> {
    /// Starting line number (for error messages)
    start_line: usize,
    /// Date format string
    date_format: &'a [BorrowedFormatItem<'a>],
    /// Column index for date
    date_column: usize,
    /// Column index for description
    description_column: usize,
    /// Column index for amount
    amount_column: usize,
    /// Minimum number of columns required
    min_columns: usize,
}

fn validate_header_line(
    lines: &[&str],
    line_num: usize,
    expected_prefix: &str,
    parser_name: &str,
) -> Result<(), Error> {
    lines
        .get(line_num)
        .filter(|line| line.starts_with(expected_prefix))
        .ok_or_else(|| {
            Error::InvalidCSV(format!(
                "{} missing header '{}' on line {}",
                parser_name, expected_prefix, line_num
            ))
        })?;
    Ok(())
}

/// Parse CSV transaction rows from lines.
///
/// This function extracts the common logic for parsing transactions across different CSV formats.
/// It handles CSV parsing, field extraction, date/amount parsing, and error reporting.
///
/// # Arguments
/// * `lines` - The CSV lines to parse (starting from transactions, not headers)
/// * `config` - Configuration specifying column indices and formats
/// * `transform_amount` - Function to transform the amount (e.g., negate for credit cards)
///
/// # Returns
/// Vector of parsed transactions, or an error if parsing fails.
fn parse_transaction_rows<F>(
    lines: &[&str],
    config: TransactionParseConfig,
    transform_amount: F,
) -> Result<Vec<TransactionBuilder>, Error>
where
    F: Fn(f64) -> f64,
{
    let mut transactions = Vec::new();

    for (idx, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }

        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(line.as_bytes());

        let record = match csv_reader.records().next() {
            Some(Ok(record)) => record,
            Some(Err(error)) => {
                return Err(Error::InvalidCSV(format!(
                    "Could not parse CSV on line {}: {error}",
                    config.start_line + idx
                )));
            }
            None => continue,
        };

        if record.len() < config.min_columns {
            continue;
        }

        let date_str = &record[config.date_column];
        let description = &record[config.description_column];
        let amount_str = &record[config.amount_column];

        let date = Date::parse(date_str, config.date_format).map_err(|error| {
            Error::InvalidCSV(format!(
                "Could not parse '{}' as date on line {}: {error}",
                date_str,
                config.start_line + idx
            ))
        })?;

        let amount: f64 = amount_str.parse().map_err(|error| {
            Error::InvalidCSV(format!(
                "Could not parse '{}' as amount on line {}: {error}",
                amount_str,
                config.start_line + idx
            ))
        })?;

        let amount = transform_amount(amount);

        // Use the original raw line for import_id to maintain compatibility
        let transaction =
            Transaction::build(amount, date, description).import_id(Some(create_import_id(line)));

        transactions.push(transaction);
    }

    Ok(transactions)
}

/// Parse Kiwibank CSV transaction rows, extracting account info from the last row.
///
/// Kiwibank CSVs include account number, balance, and date on every transaction row.
/// This function extracts transactions and returns the account info from the last row.
///
/// # Arguments
/// * `lines` - The CSV lines to parse (starting from transactions, not headers)
/// * `config` - Configuration specifying column indices and formats
/// * `account_column` - Column index for account number
/// * `balance_column` - Column index for balance
/// * `start_line` - Starting line number for error messages
///
/// # Returns
/// Tuple of (transactions, optional account info), or an error if parsing fails.
fn parse_kiwibank_transaction_rows(
    lines: &[&str],
    config: TransactionParseConfig,
    account_column: usize,
    balance_column: usize,
) -> Result<(Vec<TransactionBuilder>, Option<ImportAccount>), Error> {
    let mut transactions = Vec::new();
    let mut last_record_info: Option<(String, f64, Date)> = None;

    for (idx, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }

        let mut csv_reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(line.as_bytes());

        let record = match csv_reader.records().next() {
            Some(Ok(record)) => record,
            Some(Err(error)) => {
                return Err(Error::InvalidCSV(format!(
                    "Could not parse CSV on line {}: {error}",
                    config.start_line + idx
                )));
            }
            None => continue,
        };

        if record.len() < config.min_columns {
            return Err(Error::InvalidCSV(format!(
                "malformed CSV: expected {} columns but got {} on line {}: {}",
                config.min_columns,
                record.len(),
                config.start_line + idx,
                line
            )));
        }

        let date_str = &record[config.date_column];
        let description = &record[config.description_column];
        let amount_str = &record[config.amount_column];

        let date = Date::parse(date_str, config.date_format).map_err(|error| {
            Error::InvalidCSV(format!(
                "Could not parse '{}' as date on line {}: {error}",
                date_str,
                config.start_line + idx
            ))
        })?;

        let amount: f64 = amount_str.parse().map_err(|error| {
            Error::InvalidCSV(format!(
                "Could not parse '{}' as amount on line {}: {error}",
                amount_str,
                config.start_line + idx
            ))
        })?;

        // Extract account info (will be overwritten each iteration, keeping the last)
        let account_number = record[account_column].to_owned();

        let balance_str = &record[balance_column];
        let balance: f64 = balance_str.parse().map_err(|error| {
            Error::InvalidCSV(format!(
                "Could not parse '{}' as balance on line {}: {error}",
                balance_str,
                config.start_line + idx
            ))
        })?;

        last_record_info = Some((account_number, balance, date));

        // Use the original raw line for import_id to maintain compatibility
        let transaction =
            Transaction::build(amount, date, description).import_id(Some(create_import_id(line)));

        transactions.push(transaction);
    }

    let account = last_record_info.map(|(name, balance, date)| ImportAccount {
        name,
        balance,
        date,
    });

    Ok((transactions, account))
}

/// Parses CSV data from ASB and Kiwibank bank statements.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Dates are assumed to be in local time.
///
/// Returns a `ParseCSVResult` which consists of the transactions and account found in the CSV data.
/// Returns `Error::InvalidCSV` if the CSV data is not in an accepted format.
pub fn parse_csv(text: &str) -> Result<ParseCSVResult, Error> {
    parse_asb_bank_csv(text)
        .or_else(|err| {
            tracing::debug!("Could not parse ASB bank statement: {err}");
            parse_asb_cc_csv(text)
        })
        .or_else(|err| {
            tracing::debug!("Could not parse ASB credit card statement: {err}");
            parse_kiwibank_bank_csv(text)
        })
        .map_err(|err| {
            tracing::debug!("Could not parse Kiwibank simple bank statement: {err}");
            Error::InvalidCSV("Could not parse CSV data from ASB or Kiwibank".to_owned())
        })
}

/// Parses ASB bank account CSV exported from FastNet Classic.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
///
/// Returns a `ParseCSVResult` which consists of the transactions and account found in the CSV data.
/// Returns `Error::InvalidCSV` if the CSV data is not in an accepted format.
fn parse_asb_bank_csv(text: &str) -> Result<ParseCSVResult, Error> {
    // Header looks like:
    // Created date / time : 12 April 2025 / 11:10:19
    // Bank 12; Branch 3405; Account 0123456-50 (Streamline)
    // From date 20250101
    // To date 20250412
    // Avail Bal : 1020.00 as of 20250320
    // Ledger Balance : 20.00 as of 20250412
    // Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount
    //

    // Header line indices
    const HEADER_CREATED_DATE: usize = 0;
    const HEADER_ACCOUNT_INFO: usize = 1;
    const HEADER_FROM_DATE: usize = 2;
    const HEADER_TO_DATE: usize = 3;
    const HEADER_AVAIL_BAL: usize = 4;
    const HEADER_LEDGER_BALANCE: usize = 5;
    const HEADER_COLUMN_NAMES: usize = 6;
    const HEADER_EMPTY_LINE: usize = 7;
    const HEADER_LINE_COUNT: usize = 8;
    const TRANSACTIONS_START_LINE: usize = HEADER_LINE_COUNT;

    // CSV column indices
    const DATE_COLUMN: usize = 0;
    const DESCRIPTION_COLUMN: usize = 5;
    const AMOUNT_COLUMN: usize = 6;
    const MIN_COLUMNS: usize = 7;

    const DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[year]/[month]/[day]");
    const BALANCE_DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[year][month][day]");

    let lines = text.lines().collect::<Vec<_>>();
    let validate_header =
        |line_num, prefix| validate_header_line(&lines, line_num, prefix, "ASB Bank Statement");

    if lines.len() < HEADER_LINE_COUNT {
        return Err(Error::InvalidCSV(format!(
            "header too short: expected at least {} lines, got {}",
            HEADER_LINE_COUNT,
            lines.len()
        )));
    }

    validate_header(HEADER_CREATED_DATE, "Created date / time")?;

    let (bank, branch, account_suffix) = sscanf::sscanf!(
        lines[HEADER_ACCOUNT_INFO],
        "Bank {String}; Branch {String}; Account {String}"
    )
    .map_err(|_| {
        Error::InvalidCSV(
            format!("ASB bank statement missing header with format 'Bank XX; Branch XXXX; Account XXXXXXX-XX (Account Name)' on line {}", HEADER_ACCOUNT_INFO)
        )
    })?;
    let account = format!("{}-{}-{}", bank, branch, account_suffix);

    validate_header(HEADER_FROM_DATE, "From date ")?;
    validate_header(HEADER_TO_DATE, "To date ")?;
    validate_header(HEADER_AVAIL_BAL, "Avail Bal ")?;

    let (balance, date_string) = sscanf::sscanf!(
        lines[HEADER_LEDGER_BALANCE],
        "Ledger Balance : {f64} as of {String}"
    )
    .map_err(|_| {
        Error::InvalidCSV(format!(
            "ASB bank ledger balance on line {} should match 'Ledger Balance : <amount> as of <date>', but got '{}'",
            HEADER_LEDGER_BALANCE,
            lines[HEADER_LEDGER_BALANCE]
        ))
    })?;

    let date = Date::parse(&date_string, &BALANCE_DATE_FORMAT).map_err(|error| {
        Error::InvalidCSV(format!(
            "Could not parse '{}' as date on line {}: {error}",
            date_string, HEADER_LEDGER_BALANCE
        ))
    })?;

    if lines[HEADER_COLUMN_NAMES] != "Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount" {
        return Err(Error::InvalidCSV(format!(
            "ASB bank statement missing header 'Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount' on line {}",
            HEADER_COLUMN_NAMES
        )));
    }

    if !lines[HEADER_EMPTY_LINE].is_empty() {
        return Err(Error::InvalidCSV(format!(
            "ASB bank statement missing empty line on line {}",
            HEADER_EMPTY_LINE
        )));
    }

    let transactions = parse_transaction_rows(
        &lines[TRANSACTIONS_START_LINE..],
        TransactionParseConfig {
            start_line: TRANSACTIONS_START_LINE,
            date_format: DATE_FORMAT,
            date_column: DATE_COLUMN,
            description_column: DESCRIPTION_COLUMN,
            amount_column: AMOUNT_COLUMN,
            min_columns: MIN_COLUMNS,
        },
        |amount| amount, // No transformation needed
    )?;

    Ok(ParseCSVResult {
        transactions,
        account: Some(ImportAccount {
            name: account,
            balance,
            date,
        }),
    })
}

/// Parses ASB credit card CSV exported from FastNet Classic.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
///
/// Returns a `ParseCSVResult` which consists of the transactions and account found in the CSV data.
/// Returns `Error::InvalidCSV` if the CSV data is not in an accepted format.
fn parse_asb_cc_csv(text: &str) -> Result<ParseCSVResult, Error> {
    // Header looks like:
    // Created date / time : 12 April 2025 / 11:09:26
    // Card Number XXXX-XXXX-XXXX-5023 (Visa Light)
    // From date 20250101
    // To date 20250412
    // Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount
    //

    // Header line indices
    const HEADER_CREATED_DATE: usize = 0;
    const HEADER_CARD_NUMBER: usize = 1;
    const HEADER_FROM_DATE: usize = 2;
    const HEADER_TO_DATE: usize = 3;
    const HEADER_COLUMN_NAMES: usize = 4;
    const HEADER_EMPTY_LINE: usize = 5;
    const HEADER_LINE_COUNT: usize = 6;
    const TRANSACTIONS_START_LINE: usize = HEADER_LINE_COUNT;

    // CSV column indices
    const DATE_COLUMN: usize = 0;
    const DESCRIPTION_COLUMN: usize = 5;
    const AMOUNT_COLUMN: usize = 6;
    const MIN_COLUMNS: usize = 7;

    const DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[year]/[month]/[day]");

    let lines = text.lines().collect::<Vec<_>>();
    let validate_header = |line_num, prefix| {
        validate_header_line(&lines, line_num, prefix, "ASB Credit Card Statement")
    };

    if lines.len() < HEADER_LINE_COUNT {
        return Err(Error::InvalidCSV(format!(
            "header too short: expected at least {} lines, got {}",
            HEADER_LINE_COUNT,
            lines.len()
        )));
    }

    validate_header(HEADER_CREATED_DATE, "Created date / time")?;
    validate_header(HEADER_CARD_NUMBER, "Card Number")?;
    validate_header(HEADER_FROM_DATE, "From date ")?;
    validate_header(HEADER_TO_DATE, "To date ")?;

    if lines[HEADER_COLUMN_NAMES]
        != "Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount"
    {
        return Err(Error::InvalidCSV(format!(
            "ASB credit card statement missing header 'Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount' on line {}",
            HEADER_COLUMN_NAMES
        )));
    }

    if !lines[HEADER_EMPTY_LINE].is_empty() {
        return Err(Error::InvalidCSV(format!(
            "ASB credit card statement missing empty line on line {}",
            HEADER_EMPTY_LINE
        )));
    }

    let transactions = parse_transaction_rows(
        &lines[TRANSACTIONS_START_LINE..],
        TransactionParseConfig {
            start_line: TRANSACTIONS_START_LINE,
            date_format: DATE_FORMAT,
            date_column: DATE_COLUMN,
            description_column: DESCRIPTION_COLUMN,
            amount_column: AMOUNT_COLUMN,
            min_columns: MIN_COLUMNS,
        },
        |amount| -amount, // Negate amounts for credit card statements
    )?;

    Ok(ParseCSVResult {
        transactions,
        account: None,
    })
}

/// Parses Kiwibank account CSV exported from form ib.kiwibank.co.nz.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
///
/// Returns a `ParseCSVResult` which consists of the transactions and account found in the CSV data.
/// Returns `Error::InvalidCSV` if the CSV data is not in an accepted format.
fn parse_kiwibank_bank_csv(text: &str) -> Result<ParseCSVResult, Error> {
    // Example entry with all fields:
    // Account:                 29-7105-1392716-11,
    // Effective Date:          2025-08-22,
    // Transaction Date:        2025-08-22,
    // Description:             PAY Alice Highbrow Food Bob,
    // Transaction Code:        DIRECT DEBIT,
    // Particulars:             Highbrow,
    // Code:                    Food,
    // Reference:               Bob,
    // Other Party Name:        Alice,
    // Other Party Account:     11-2165-2080436-12,
    // Other Party Particulars: Highbrow,
    // Other Party Code:        Food,
    // Other Party Reference:   Bob,
    // Amount:                  -15.00,
    // Balance:                 980.69

    // Example of card expense:
    // Account:                 29-7105-1392716-11,
    // Effective Date:          2025-08-23,
    // Transaction Date:        2025-08-23,
    // Description:             PAK N SAVE SYLVIA PARK AUCKLAND,
    // Transaction Code:        EFTPOS PURCHASE,
    // Particulars:             ,
    // Code:                    ,
    // Reference:               ,
    // Other Party Name:        ,
    // Other Party Account:     ,
    // Other Party Particulars: ,
    // Other Party Code:        ,
    // Other Party Reference:   ,
    // Amount:                  -43.21,
    // Balance:                 937.48

    // Header line indices
    const HEADER_LINE_COUNT: usize = 1;
    const TRANSACTIONS_START_LINE: usize = HEADER_LINE_COUNT;

    // CSV column indices
    const ACCOUNT: usize = 0;
    /// The date the transaction was initiated (e.g. card was swiped)
    const EFFECTIVE_DATE: usize = 1;
    // /// The date the transaction was cleared/processed
    // const TRANSACTION_DATE: usize = 2;
    const DESCRIPTION: usize = 3;
    // const TRANSACTION_CODE: usize = 4;
    // const PARTICULARS: usize = 5;
    // const CODE: usize = 6;
    // const REFERENCE: usize = 7;
    // const OTHER_PARTY_NAME: usize = 8;
    // const OTHER_PARTY_ACCOUNT_NUMBER: usize = 9;
    // const OTHER_PARTY_PARTICULARS: usize = 10;
    // const OTHER_PARTY_CODE: usize = 11;
    // const OTHER_PARTY_REFERENCE: usize = 12;
    const AMOUNT: usize = 13;
    const BALANCE: usize = 14;
    const COLUMN_COUNT: usize = 15;

    const DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[year]-[month]-[day]");

    let lines: Vec<&str> = text.lines().collect();

    if lines.is_empty() {
        return Err(Error::InvalidCSV("empty CSV file".to_owned()));
    }

    let (transactions, account) = parse_kiwibank_transaction_rows(
        &lines[TRANSACTIONS_START_LINE..], // Skip header line
        TransactionParseConfig {
            start_line: TRANSACTIONS_START_LINE,
            date_format: DATE_FORMAT,
            date_column: EFFECTIVE_DATE,
            description_column: DESCRIPTION,
            amount_column: AMOUNT,
            min_columns: COLUMN_COUNT,
        },
        ACCOUNT,
        BALANCE,
    )?;

    Ok(ParseCSVResult {
        transactions,
        account,
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
mod tests {
    use time::macros::date;

    use crate::{
        Error,
        csv_import::csv::{
            ImportAccount, create_import_id, parse_asb_bank_csv, parse_asb_cc_csv, parse_csv,
            parse_kiwibank_bank_csv,
        },
    };

    // ============================================================================
    // Test Data
    // ============================================================================

    const ASB_BANK_STATEMENT_CSV: &str = "\
Created date / time : 12 April 2025 / 11:10:19
Bank 12; Branch 3405; Account 0123456-50 (Streamline)
From date 20250101
To date 20250412
Avail Bal : 1020.00 as of 20250320
Ledger Balance : 20.00 as of 20250412
Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount

2025/01/18,2025011801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",1300.00
2025/01/18,2025011802,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  Credit Card\",-1300.00
2025/02/18,2025021801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",4400.00
2025/02/19,2025021901,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-4400.00
2025/03/20,2025032001,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",2750.00
2025/03/20,2025032002,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-2750.00";

    const ASB_CC_STATEMENT_CSV: &str = "\
Created date / time : 12 April 2025 / 11:09:26
Card Number XXXX-XXXX-XXXX-5023 (Visa Light)
From date 20250101
To date 20250412
Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount

2025/03/20,2025/03/20,2025032002,CREDIT,5023,\"PAYMENT RECEIVED THANK YOU\",-2750.00
2025/04/09,2025/04/08,2025040902,DEBIT,5023,\"Birdy Bytes\",8.50
2025/04/10,2025/04/07,2025041001,DEBIT,5023,\"AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)\",10.63
2025/04/10,2025/04/07,2025041002,DEBIT,5023,\"OFFSHORE SERVICE MARGINS\",0.22
2025/04/11,2025/04/10,2025041101,DEBIT,5023,\"Buckstars\",11.50";

    const KIWIBANK_BANK_STATEMENT_CSV: &str = "\
Account number,Effective Date,Transaction Date,Description,Transaction Code,Particulars,Code,Reference,Other Party Name,Other Party Account Number,Other Party Particulars,Other Party Code,Other Party Reference,Amount,Balance
38-8106-0601663-00,2025-08-21,2025-08-22,Sushi,EFTPOS PURCHASE,,,,,,,,,-9.00,895.69
38-8106-0601663-00,2025-08-22,2025-08-22,PAY Alice The Bar Drinks Bob,DIRECT DEBIT,The Bar,Drinks,Bob,Alice,01-2345-1080543-00,The Bar,Drinks,Bob,-15.00,880.69
38-8106-0601663-00,2025-08-23,2025-08-23,PAY Alice Pool Bob,DIRECT DEBIT,Pool,,Bob,Alice,01-2345-1080543-00,Pool,,Bob,-3.15,877.54
38-8106-0601663-00,2025-08-23,2025-08-23,PAK N SAVE SYLVIA PARK AUCKLAND,EFTPOS PURCHASE,,,,,,,,,-42.02,835.52";

    // ============================================================================
    // Import ID Tests
    // ============================================================================

    #[test]
    fn create_import_id_is_deterministic() {
        let line = "38-1234-0123456-01,31-01-2025,INTEREST EARNED ;,,,,,,,,,,0.25,,0.25,71.16";
        assert_eq!(create_import_id(line), create_import_id(line));
    }

    #[test]
    fn create_import_id_differs_for_different_inputs() {
        let line1 = "38-1234-0123456-01,31-01-2025,INTEREST EARNED ;,,,,,,,,,,0.25,,0.25,71.16";
        let line2 = "2025/04/10,2025/04/07,2025041001,DEBIT,5023,\"AMAZON DOWNLOADS\",10.63";
        assert_ne!(create_import_id(line1), create_import_id(line2));
    }

    // ============================================================================
    // ASB Bank Statement Tests
    // ============================================================================

    #[test]
    fn parse_asb_bank_statement_success() {
        let result = parse_asb_bank_csv(ASB_BANK_STATEMENT_CSV).unwrap();

        // Check account
        assert_eq!(
            result.account,
            Some(ImportAccount {
                name: "12-3405-0123456-50 (Streamline)".to_owned(),
                balance: 20.0,
                date: date!(2025 - 04 - 12),
            })
        );

        // Check transactions
        assert_eq!(result.transactions.len(), 6);

        // Spot check a few key transactions
        assert_eq!(result.transactions[0].amount, 1300.00);
        assert_eq!(result.transactions[0].date, date!(2025 - 01 - 18));
        assert_eq!(result.transactions[0].description, "Credit Card");

        assert_eq!(result.transactions[5].amount, -2750.00);
        assert_eq!(
            result.transactions[5].description,
            "TO CARD 5023  THANK YOU"
        );
    }

    #[test]
    fn parse_asb_bank_statement_with_comma_in_description() {
        let csv = "\
Created date / time : 12 April 2025 / 11:10:19
Bank 12; Branch 3405; Account 0123456-50 (Streamline)
From date 20250101
To date 20250412
Avail Bal : 1020.00 as of 20250320
Ledger Balance : 20.00 as of 20250412
Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount

2025/01/18,2025011801,D/C,,\"PAYEE, INC\",\"Description, with comma\",1300.00";

        let result = parse_asb_bank_csv(csv).unwrap();

        assert_eq!(result.transactions.len(), 1);
        assert_eq!(
            result.transactions[0].description,
            "Description, with comma"
        );
    }

    #[test]
    fn parse_asb_bank_statement_empty_lines_ignored() {
        let csv = "\
Created date / time : 12 April 2025 / 11:10:19
Bank 12; Branch 3405; Account 0123456-50 (Streamline)
From date 20250101
To date 20250412
Avail Bal : 1020.00 as of 20250320
Ledger Balance : 20.00 as of 20250412
Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount


2025/01/18,2025011801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",1300.00

2025/01/18,2025011802,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023\",-1300.00

";

        let result = parse_asb_bank_csv(csv).unwrap();
        assert_eq!(result.transactions.len(), 2);
    }

    #[test]
    fn parse_asb_bank_statement_missing_header_fails() {
        let csv = "Not a valid ASB statement";
        assert!(matches!(parse_asb_bank_csv(csv), Err(Error::InvalidCSV(_))));
    }

    #[test]
    fn parse_asb_bank_statement_malformed_balance_line_fails() {
        let csv = "\
Created date / time : 12 April 2025 / 11:10:19
Bank 12; Branch 3405; Account 0123456-50 (Streamline)
From date 20250101
To date 20250412
Avail Bal : 1020.00 as of 20250320
Ledger Balance MALFORMED
Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount
";

        assert!(matches!(parse_asb_bank_csv(csv), Err(Error::InvalidCSV(_))));
    }

    #[test]
    fn parse_asb_bank_statement_invalid_date_fails() {
        let csv = "\
Created date / time : 12 April 2025 / 11:10:19
Bank 12; Branch 3405; Account 0123456-50 (Streamline)
From date 20250101
To date 20250412
Avail Bal : 1020.00 as of 20250320
Ledger Balance : 20.00 as of 20250412
Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount

INVALID_DATE,2025011801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",1300.00";

        assert!(matches!(parse_asb_bank_csv(csv), Err(Error::InvalidCSV(_))));
    }

    // ============================================================================
    // ASB Credit Card Statement Tests
    // ============================================================================

    #[test]
    fn parse_asb_cc_statement_success() {
        let result = parse_asb_cc_csv(ASB_CC_STATEMENT_CSV).unwrap();

        // CC statements don't include account info
        assert_eq!(result.account, None);

        // Check transactions
        assert_eq!(result.transactions.len(), 5);

        // Check amount negation (CC debits are positive in CSV, should be negative)
        assert_eq!(result.transactions[0].amount, 2750.00); // CREDIT stays positive
        assert_eq!(result.transactions[1].amount, -8.50); // DEBIT becomes negative
    }

    #[test]
    fn parse_asb_cc_statement_with_special_chars() {
        let csv = "\
Created date / time : 12 April 2025 / 11:09:26
Card Number XXXX-XXXX-XXXX-5023 (Visa Light)
From date 20250101
To date 20250412
Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount

2025/04/10,2025/04/07,2025041001,DEBIT,5023,\"STORE \"\"QUOTES\"\" & COMMAS, INC\",10.63";

        let result = parse_asb_cc_csv(csv).unwrap();

        assert_eq!(result.transactions.len(), 1);
        // csv crate should handle escaped quotes properly
        assert!(result.transactions[0].description.contains("QUOTES"));
    }

    #[test]
    fn parse_asb_cc_statement_missing_header_fails() {
        let csv = "Not a valid credit card statement";
        assert!(matches!(parse_asb_cc_csv(csv), Err(Error::InvalidCSV(_))));
    }

    // ============================================================================
    // Kiwibank Statement Tests
    // ============================================================================

    #[test]
    fn parse_kiwibank_statement_success() {
        let result = parse_kiwibank_bank_csv(KIWIBANK_BANK_STATEMENT_CSV).unwrap();

        // Check account
        assert_eq!(
            result.account,
            Some(ImportAccount {
                name: "38-8106-0601663-00".to_owned(),
                balance: 835.52,
                date: date!(2025 - 08 - 23),
            })
        );

        // Check transactions
        assert_eq!(result.transactions.len(), 4);

        // Spot checks
        assert_eq!(result.transactions[0].amount, -9.00);
        assert_eq!(result.transactions[0].description, "Sushi");

        assert_eq!(result.transactions[3].amount, -42.02);
        assert_eq!(
            result.transactions[3].description,
            "PAK N SAVE SYLVIA PARK AUCKLAND"
        );
    }

    #[test]
    fn parse_kiwibank_statement_with_comma_in_description() {
        let csv = "\
Account number,Effective Date,Transaction Date,Description,Transaction Code,Particulars,Code,Reference,Other Party Name,Other Party Account Number,Other Party Particulars,Other Party Code,Other Party Reference,Amount,Balance
38-8106-0601663-00,2025-08-21,2025-08-22,\"STORE, INC\",EFTPOS PURCHASE,,,,,,,,,-9.00,895.69";

        let result = parse_kiwibank_bank_csv(csv).unwrap();

        assert_eq!(result.transactions.len(), 1);
        assert_eq!(result.transactions[0].description, "STORE, INC");
    }

    #[test]
    fn parse_kiwibank_statement_empty_csv_fails() {
        assert!(matches!(
            parse_kiwibank_bank_csv(""),
            Err(Error::InvalidCSV(_))
        ));
    }

    #[test]
    fn parse_kiwibank_statement_wrong_column_count_fails() {
        let csv = "\
Account number,Effective Date,Transaction Date,Description,Transaction Code,Particulars,Code,Reference,Other Party Name,Other Party Account Number,Other Party Particulars,Other Party Code,Other Party Reference,Amount,Balance
38-8106-0601663-00,2025-08-21,MISSING_COLUMNS";

        assert!(matches!(
            parse_kiwibank_bank_csv(csv),
            Err(Error::InvalidCSV(_))
        ));
    }

    // ============================================================================
    // Main parse_csv() Function Tests
    // ============================================================================

    #[test]
    fn parse_csv_detects_asb_bank_format() {
        let result = parse_csv(ASB_BANK_STATEMENT_CSV).unwrap();
        assert_eq!(result.transactions.len(), 6);
        assert!(result.account.is_some());
        assert!(result.account.unwrap().name.contains("12-3405"));
    }

    #[test]
    fn parse_csv_detects_asb_cc_format() {
        let result = parse_csv(ASB_CC_STATEMENT_CSV).unwrap();
        assert_eq!(result.transactions.len(), 5);
        assert!(result.account.is_none());
    }

    #[test]
    fn parse_csv_detects_kiwibank_format() {
        let result = parse_csv(KIWIBANK_BANK_STATEMENT_CSV).unwrap();
        assert_eq!(result.transactions.len(), 4);
        assert!(result.account.is_some());
        assert!(result.account.unwrap().name.contains("38-8106"));
    }

    #[test]
    fn parse_csv_rejects_invalid_format() {
        let invalid_csv = "\
Invalid Header Line 1
Invalid Header Line 2
Invalid Header Line 3
Invalid Header Line 4
Invalid Header Line 5
Invalid Header Line 6
Invalid Header Line 7
Invalid Header Line 8
Invalid CSV data that doesn't match any known format";

        let result = parse_csv(invalid_csv);
        assert!(result.is_err(), "Should fail to parse invalid format");

        if let Err(Error::InvalidCSV(msg)) = result {
            assert!(msg.contains("Could not parse CSV data from ASB or Kiwibank"));
        } else {
            panic!("Expected InvalidCSV error");
        }
    }

    // ============================================================================
    // Import ID Preservation Tests (Critical for migration)
    // ============================================================================

    #[test]
    fn import_ids_match_original_format() {
        // This is critical: import IDs must match the original implementation
        // to prevent duplicate transactions after migration

        let result = parse_asb_bank_csv(ASB_BANK_STATEMENT_CSV).unwrap();

        // These are the exact raw CSV lines from the test data
        let expected_ids = [
            create_import_id(
                "2025/01/18,2025011801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",1300.00",
            ),
            create_import_id(
                "2025/01/18,2025011802,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  Credit Card\",-1300.00",
            ),
            create_import_id(
                "2025/02/18,2025021801,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",4400.00",
            ),
            create_import_id(
                "2025/02/19,2025021901,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-4400.00",
            ),
            create_import_id(
                "2025/03/20,2025032001,D/C,,\"D/C FROM A B Cat\",\"Credit Card\",2750.00",
            ),
            create_import_id(
                "2025/03/20,2025032002,TFR OUT,,\"MB TRANSFER\",\"TO CARD 5023  THANK YOU\",-2750.00",
            ),
        ];

        for (i, tx) in result.transactions.iter().enumerate() {
            assert_eq!(
                tx.import_id,
                Some(expected_ids[i]),
                "Import ID mismatch at index {}",
                i
            );
        }
    }
}
