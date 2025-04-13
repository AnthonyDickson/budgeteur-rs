//! Functions to parse CSV data from ASB and Kiwibank bank statements.

use time::{Date, format_description::BorrowedFormatItem, macros::format_description};

use crate::models::{TransactionBuilder, UserID};

/// Parses CSV data from ASB and Kiwibank bank statements.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Uses `user_id` to set the user ID for the transactions.
///
/// Returns a vector of `Transaction` objects found in the CSV data or an empty vector if no
/// transactions were found.
pub fn parse_csv(text: &str, user_id: UserID) -> Vec<TransactionBuilder> {
    let transactions = parse_asb_bank_csv(text, user_id);

    if !transactions.is_empty() {
        return transactions;
    }

    let transactions = parse_asb_cc_csv(text, user_id);

    if !transactions.is_empty() {
        return transactions;
    }

    parse_kiwibank_bank_csv(text, user_id)
}

/// Parses ASB bank account CSV exported from FastNet Classic.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Uses `user_id` to set the user ID for the transactions.
///
/// Returns a vector of `Transaction` objects found in the CSV data or an empty vector if no
/// transactions were found.
fn parse_asb_bank_csv(text: &str, user_id: UserID) -> Vec<TransactionBuilder> {
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
        // TODO: Return an error instead of an empty vector indicating that the
        // CSV format is invalid
        return Vec::new();
    }

    let mut transactions = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        match line_number {
            0 if !line.starts_with("Created date / time") => {
                return Vec::new();
            }
            1 if !line.starts_with("Bank ") => {
                return Vec::new();
            }
            // 1 => {
            //     let parts = line.split(';').collect::<Vec<_>>();
            //
            //     // TODO: return an error instead unwrap and get rid of the previous match arm.
            //     let bank = parts[0].strip_prefix("Bank ").unwrap();
            //     let branch = parts[1].strip_prefix(" Branch ").unwrap();
            //     let account = parts[2].strip_prefix(" Account ").unwrap();
            //     let account_number = &[bank, branch, account].join("-");
            // }
            2 if !line.starts_with("From date ") => {
                return Vec::new();
            }
            3 if !line.starts_with("To date ") => {
                return Vec::new();
            }
            4 if !line.starts_with("Avail Bal") => {
                return Vec::new();
            }
            5 if !line.starts_with("Ledger Balance") => {
                return Vec::new();
            }
            6 if line != "Date,Unique Id,Tran Type,Cheque Number,Payee,Memo,Amount" => {
                return Vec::new();
            }
            7 if !line.is_empty() => {
                return Vec::new();
            }
            _ if line_number > 7 => {
                let parts: Vec<&str> = line.split(',').collect();

                if parts.len() < 7 {
                    continue;
                }

                // TODO: Return an error instead of panicing.
                let date = match Date::parse(parts[DATE_COLUMN], &DATE_FORMAT) {
                    Ok(date) => date,
                    Err(error) => {
                        panic!("Error parsing {} as date: {}", parts[DATE_COLUMN], error);
                    }
                };
                let description = parts[DESCRIPTION_COLUMN];
                let description = description.trim_matches('"');
                // TODO: Return an error instead of panicing.
                let amount: f64 = parts[AMOUNT_COLUMN].parse().unwrap();

                let transaction = TransactionBuilder::new(amount, user_id)
                    .date(date)
                    // TODO: Return an error instead of panicing.
                    .expect("Got future date")
                    .description(description);

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    transactions
}

/// Parses ASB credit card CSV exported from FastNet Classic.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Uses `user_id` to set the user ID for the transactions.
///
/// Returns a vector of `Transaction` objects found in the CSV data or an empty vector if no
/// transactions were found.
fn parse_asb_cc_csv(text: &str, user_id: UserID) -> Vec<TransactionBuilder> {
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
        // TODO: Return an error instead of an empty vector indicating that the
        // CSV format is invalid
        return Vec::new();
    }

    let mut transactions = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        match line_number {
            0 if !line.starts_with("Created date / time") => {
                return Vec::new();
            }
            1 if !line.starts_with("Card Number ") => {
                return Vec::new();
            }
            // 1 => {
            //     // TODO: return an error instead unwrap: let account_number line.strip_prefix("Card Number ")?;
            //     // Also get rid of the previous match arm.
            //     let account_number = line.strip_prefix("Card Number ").unwrap();
            // }
            2 if !line.starts_with("From date ") => {
                return Vec::new();
            }
            3 if !line.starts_with("To date ") => {
                return Vec::new();
            }
            4 if line
                != "Date Processed,Date of Transaction,Unique Id,Tran Type,Reference,Description,Amount" =>
            {
                return Vec::new();
            }
            5 if !line.is_empty() => {
                return Vec::new();
            }
            _ if line_number > 5 => {
                let parts: Vec<&str> = line.split(',').collect();

                if parts.len() < 7 {
                    continue;
                }

                // TODO: Return an error instead of panicing.
                let date = match Date::parse(parts[DATE_COLUMN], &DATE_FORMAT) {
                    Ok(date) => date,
                    Err(error) => {
                        panic!("Error parsing {} as date: {}", parts[DATE_COLUMN], error);
                    }
                };
                let description = parts[DESCRIPTION_COLUMN];
                let description = description.trim_matches('"');
                // TODO: Return an error instead of panicing.
                let amount: f64 = parts[AMOUNT_COLUMN].parse().unwrap();

                let transaction = TransactionBuilder::new(amount, user_id)
                    .date(date)
                    // TODO: Return an error instead of panicing.
                    .expect("Got future date")
                    .description(description);

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    transactions
}

/// Parses detailed Kiwibank account CSV exported from form ib.kiwibank.co.nz.
///
/// Expects `text` to be a string containing comma separated values with lines separated by `\n`.
/// Uses `user_id` to set the user ID for the transactions.
///
/// Returns a vector of `Transaction` objects found in the CSV data or an empty vector if no
/// transactions were found.
fn parse_kiwibank_bank_csv(text: &str, user_id: UserID) -> Vec<TransactionBuilder> {
    // Header looks like:
    // Account number,Date,Memo/Description,Source Code (payment type),TP ref,TP part,TP code,OP ref,OP part,OP code,OP name,OP Bank Account Number,Amount (credit),Amount (debit),Amount,Balance
    const DATE_COLUMN: usize = 1;
    const DESCRIPTION_COLUMN: usize = 2;
    const AMOUNT_COLUMN: usize = 14;
    const DATE_FORMAT: &[BorrowedFormatItem] = format_description!("[day]-[month]-[year]");

    // Parse the header to get the account number
    let lines = text.lines().collect::<Vec<_>>();

    if lines.is_empty() {
        // TODO: Return an error instead of an empty vector indicating that the
        // CSV format is invalid
        return Vec::new();
    }

    let mut transactions = Vec::new();

    for (line_number, line) in text.lines().enumerate() {
        match line_number {
            0 if line
                != "Account number,Date,Memo/Description,Source Code (payment type),TP ref,TP part,TP code,OP ref,OP part,OP code,OP name,OP Bank Account Number,Amount (credit),Amount (debit),Amount,Balance" =>
            {
                return Vec::new();
            }
            _ if line_number > 0 => {
                let parts: Vec<&str> = line.split(',').collect();

                if parts.len() < 16 {
                    continue;
                }

                // TODO: Return an error instead of panicing.
                let date = match Date::parse(parts[DATE_COLUMN], &DATE_FORMAT) {
                    Ok(date) => date,
                    Err(error) => {
                        panic!("Error parsing {} as date: {}", parts[DATE_COLUMN], error);
                    }
                };
                let description = parts[DESCRIPTION_COLUMN];
                let description = description.trim_matches('"');
                let description = description.trim_end_matches(" ;");
                // TODO: Return an error instead of panicing.
                let amount: f64 = parts[AMOUNT_COLUMN].parse().unwrap();

                let transaction = TransactionBuilder::new(amount, user_id)
                    .date(date)
                    // TODO: Return an error instead of panicing.
                    .expect("Got future date")
                    .description(description);

                transactions.push(transaction);
            }
            _ => {}
        }
    }

    transactions
}

#[cfg(test)]
mod parse_csv_tests {
    use time::macros::date;

    use crate::{
        csv::{parse_asb_bank_csv, parse_kiwibank_bank_csv},
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
    fn can_parse_asb_bank_statement() {
        let user_id = UserID::new(42);
        let want = vec![
            TransactionBuilder::new(1300.00, user_id)
                .date(date!(2025 - 01 - 18))
                .expect("Could not set date")
                .description("Credit Card"),
            TransactionBuilder::new(-1300.00, user_id)
                .date(date!(2025 - 01 - 18))
                .expect("Could not set date")
                .description("TO CARD 5023  Credit Card"),
            TransactionBuilder::new(4400.00, user_id)
                .date(date!(2025 - 02 - 18))
                .expect("Could not set date")
                .description("Credit Card"),
            TransactionBuilder::new(-4400.00, user_id)
                .date(date!(2025 - 02 - 19))
                .expect("Could not set date")
                .description("TO CARD 5023  THANK YOU"),
            TransactionBuilder::new(2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not set date")
                .description("Credit Card"),
            TransactionBuilder::new(-2750.00, user_id)
                .date(date!(2025 - 03 - 20))
                .expect("Could not set date")
                .description("TO CARD 5023  THANK YOU"),
        ];

        let result = parse_asb_bank_csv(ASB_BANK_STATEMENT_CSV, user_id);

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
                .description("PAYMENT RECEIVED THANK YOU"),
            TransactionBuilder::new(8.50, user_id)
                .date(date!(2025 - 04 - 09))
                .expect("Could not parse date")
                .description("Birdy Bytes"),
            TransactionBuilder::new(10.63, user_id)
                .date(date!(2025 - 04 - 10))
                .expect("Could not parse date")
                .description(
                    "AMAZON DOWNLOADS TOKYO 862.00 YEN at a Conversion Rate  of 81.0913 (NZ$10.63)",
                ),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 04 - 10))
                .expect("Could not parse date")
                .description("OFFSHORE SERVICE MARGINS"),
            TransactionBuilder::new(11.50, user_id)
                .date(date!(2025 - 04 - 11))
                .expect("Could not parse date")
                .description("Buckstars"),
        ];

        let result = parse_asb_cc_csv(ASB_CC_STATEMENT_CSV, user_id);

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
                .description("INTEREST EARNED"),
            TransactionBuilder::new(-0.03, user_id)
                .date(date!(2025 - 01 - 31))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%"),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 02 - 28))
                .expect("Could not parse date")
                .description("INTEREST EARNED"),
            TransactionBuilder::new(-0.02, user_id)
                .date(date!(2025 - 02 - 28))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%"),
            TransactionBuilder::new(0.22, user_id)
                .date(date!(2025 - 03 - 31))
                .expect("Could not parse date")
                .description("INTEREST EARNED"),
            TransactionBuilder::new(-0.02, user_id)
                .date(date!(2025 - 03 - 31))
                .expect("Could not parse date")
                .description("PIE TAX 10.500%"),
        ];

        let result = parse_kiwibank_bank_csv(KIWIBANK_BANK_STATEMENT_CSV, user_id);

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
