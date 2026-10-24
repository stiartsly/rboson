// Copyright (c) 2023 -      bosonnetwork.io
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

//! Serde adapters matching the Java date wire format.
//!
//! Text formats use RFC 3339 at second precision (`yyyy-MM-dd'T'HH:mm:ss'Z'`),
//! while CBOR stores epoch milliseconds, mirroring the Java `DateSerializer`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serializer};

/// Serializes a `SystemTime` using the protocol date representation.
pub(crate) fn serialize<S>(value: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if serializer.is_human_readable() {
        serializer.serialize_str(&format_rfc3339(*value))
    } else {
        serializer.serialize_i64(to_millis(*value))
    }
}

/// Deserializes a `SystemTime` using the protocol date representation.
pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let value = String::deserialize(deserializer)?;
        parse_rfc3339(&value).map_err(D::Error::custom)
    } else {
        let millis = i64::deserialize(deserializer)?;
        from_millis(millis).map_err(D::Error::custom)
    }
}

/// Optional variant of [`serialize`].
pub(crate) fn serialize_opt<S>(
    value: &Option<SystemTime>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serialize(value, serializer),
        None => serializer.serialize_none(),
    }
}

/// Optional variant of [`deserialize`].
pub(crate) fn deserialize_opt<'de, D>(deserializer: D) -> Result<Option<SystemTime>, D::Error>
where
    D: Deserializer<'de>,
{
    if deserializer.is_human_readable() {
        let value = Option::<String>::deserialize(deserializer)?;
        value
            .map(|value| parse_rfc3339(&value))
            .transpose()
            .map_err(D::Error::custom)
    } else {
        let value = Option::<i64>::deserialize(deserializer)?;
        value
            .map(from_millis)
            .transpose()
            .map_err(D::Error::custom)
    }
}

pub(crate) fn to_millis(value: SystemTime) -> i64 {
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(error) => -i64::try_from(error.duration().as_millis()).unwrap_or(i64::MAX),
    }
}

fn from_millis(millis: i64) -> Result<SystemTime, String> {
    if millis >= 0 {
        UNIX_EPOCH
            .checked_add(Duration::from_millis(millis as u64))
            .ok_or_else(|| format!("Date is out of range: {millis}"))
    } else {
        UNIX_EPOCH
            .checked_sub(Duration::from_millis(millis.unsigned_abs()))
            .ok_or_else(|| format!("Date is out of range: {millis}"))
    }
}

/// Formats a timestamp as `YYYY-MM-DDTHH:MM:SSZ`.
pub(crate) fn format_rfc3339(value: SystemTime) -> String {
    let seconds = seconds_since_epoch(value);
    let (days, time) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (time / 3_600, (time % 3_600) / 60, time % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Parses `YYYY-MM-DDTHH:MM:SSZ`, tolerating optional fractional seconds.
pub(crate) fn parse_rfc3339(value: &str) -> Result<SystemTime, String> {
    let value = value
        .strip_suffix('Z')
        .ok_or_else(|| format!("Invalid datetime string: {value}"))?;
    let (date, time) = value
        .split_once('T')
        .ok_or_else(|| format!("Invalid datetime string: {value}"))?;
    let time = time.split_once('.').map_or(time, |(whole, fraction)| {
        if fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            whole
        } else {
            time
        }
    });

    let mut date_parts = date.split('-');
    let year: i64 = parse_part(date_parts.next(), "year")?;
    let month: u32 = parse_part(date_parts.next(), "month")?;
    let day: u32 = parse_part(date_parts.next(), "day")?;
    if date_parts.next().is_some() {
        return Err(format!("Invalid datetime string: {value}"));
    }

    let mut time_parts = time.split(':');
    let hour: i64 = parse_part(time_parts.next(), "hour")?;
    let minute: i64 = parse_part(time_parts.next(), "minute")?;
    let second: i64 = parse_part(time_parts.next(), "second")?;
    if time_parts.next().is_some() {
        return Err(format!("Invalid datetime string: {value}"));
    }

    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return Err(format!("Invalid datetime string: {value}"));
    }

    let days = days_from_civil(year, month, day);
    let (_, expected_month, expected_day) = civil_from_days(days);
    if expected_month != month || expected_day != day {
        return Err(format!("Invalid datetime string: {value}"));
    }

    let seconds = days * 86_400 + hour * 3_600 + minute * 60 + second;
    from_seconds(seconds)
}

fn parse_part<T: std::str::FromStr>(part: Option<&str>, name: &str) -> Result<T, String> {
    part.ok_or_else(|| format!("Missing datetime {name}"))?
        .parse()
        .map_err(|_| format!("Invalid datetime {name}"))
}

fn seconds_since_epoch(value: SystemTime) -> i64 {
    match value.duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_secs()).unwrap_or(i64::MAX),
        Err(error) => -i64::try_from(error.duration().as_secs()).unwrap_or(i64::MAX),
    }
}

fn from_seconds(seconds: i64) -> Result<SystemTime, String> {
    if seconds >= 0 {
        UNIX_EPOCH
            .checked_add(Duration::from_secs(seconds as u64))
            .ok_or_else(|| format!("Date is out of range: {seconds}"))
    } else {
        UNIX_EPOCH
            .checked_sub(Duration::from_secs(seconds.unsigned_abs()))
            .ok_or_else(|| format!("Date is out of range: {seconds}"))
    }
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (if month <= 2 { y + 1 } else { y }, month as u32, day as u32)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i64;
    let day = day as i64;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses_rfc3339_seconds() {
        let value = UNIX_EPOCH + Duration::from_secs(1_739_645_781);
        assert_eq!(format_rfc3339(value), "2025-02-15T18:56:21Z");
        assert_eq!(parse_rfc3339("2025-02-15T18:56:21Z").unwrap(), value);
        assert_eq!(
            parse_rfc3339("2025-02-15T18:56:21.892Z").unwrap(),
            value
        );
    }

    #[test]
    fn rejects_invalid_dates() {
        assert!(parse_rfc3339("2025-02-30T18:56:21Z").is_err());
        assert!(parse_rfc3339("2025-02-15 18:56:21Z").is_err());
        assert!(parse_rfc3339("2025-02-15T24:00:00Z").is_err());
    }

    #[test]
    fn millis_round_trip() {
        let value = UNIX_EPOCH + Duration::from_millis(1_739_645_781_892);
        assert_eq!(to_millis(value), 1_739_645_781_892);
        assert_eq!(from_millis(to_millis(value)).unwrap(), value);
    }
}
