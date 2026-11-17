//! Numbers and dates, as the screens show them.

pub fn group_thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn sats(n: u64) -> String {
    format!("{} sat", group_thousands(n))
}

pub fn signed_sats(n: u64, incoming: bool) -> String {
    format!("{}{}", if incoming { "+" } else { "-" }, sats(n))
}

/// `2026-11-16 14:03`, in UTC.
pub fn when(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year}-{month:02}-{day:02} {:02}:{:02}",
        secs / 3600,
        secs % 3600 / 60
    )
}

/// `5f1c9d0a…2e1f0a9b`.
pub fn short(text: &str) -> String {
    if text.chars().count() <= 20 {
        return text.to_owned();
    }
    let head: String = text.chars().take(10).collect();
    let tail: String = text.chars().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(group_thousands(1_234_567), "1,234,567");
        assert_eq!(group_thousands(12), "12");
        assert_eq!(when(0), "1970-01-01 00:00");
        assert_eq!(when(1_793_875_200), "2026-11-05 10:40");
    }
}
