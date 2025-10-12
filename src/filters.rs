//! Defines custom filters for Askama templates

use std::sync::OnceLock;

use numfmt::{Formatter, Precision};

pub fn abs(number: f64, _: &dyn askama::Values) -> askama::Result<f64> {
    Ok(number.abs())
}

pub fn currency(number: f64, _: &dyn askama::Values) -> askama::Result<String> {
    static POSITIVE_FMT: OnceLock<Formatter> = OnceLock::new();

    let positive_fmt = POSITIVE_FMT.get_or_init(|| {
        Formatter::currency("$")
            .unwrap()
            .precision(Precision::Decimals(2))
    });

    static NEGATIVE_FMT: OnceLock<Formatter> = OnceLock::new();

    let negative_fmt = NEGATIVE_FMT.get_or_init(|| {
        Formatter::currency("-$")
            .unwrap()
            .precision(Precision::Decimals(2))
    });

    let mut formatted_string = if number < 0.0 {
        negative_fmt.fmt_string(number.abs())
    } else if number > 0.0 {
        positive_fmt.fmt_string(number)
    } else {
        // Zero is hardcoded as "0", so we must specify the formatted string for zero
        "$0.00".to_owned()
    };

    // numfmt omits the last trailing zero, so we must add it ourselves
    // For example, "12.30" is rendered as "12.3" so we append "0".
    if formatted_string.as_bytes()[formatted_string.len() - 3] != b'.' {
        formatted_string = format!("{formatted_string}0");
    }

    Ok(formatted_string)
}
