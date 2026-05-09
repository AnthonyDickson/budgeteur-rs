use time::{OffsetDateTime, UtcOffset};
use time_tz::{Offset, TimeZone};

pub fn get_local_offset(canonical_timezone: &str) -> Option<UtcOffset> {
    time_tz::timezones::get_by_name(canonical_timezone)
        .map(|tz| tz.get_offset_utc(&OffsetDateTime::now_utc()).to_utc())
}
