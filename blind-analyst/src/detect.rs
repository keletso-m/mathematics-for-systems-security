use regex::Regex;
use std::collections::HashSet;

pub struct Found {
    pub value: String,
    pub kind: &'static str,
}

const STOP: &[&str] = &[
    "The", "This", "That", "These", "Those", "Finance", "Team", "Monday", "Tuesday",
    "Wednesday", "Thursday", "Friday", "Saturday", "Sunday", "January", "February",
    "March", "April", "May", "June", "July", "August", "September", "October",
    "November", "December",
];

fn luhn(digits: &str) -> bool {
    let mut sum = 0;
    let mut alt = false;
    for c in digits.chars().rev() {
        let mut d = c.to_digit(10).unwrap();
        if alt {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
        alt = !alt;
    }
    sum % 10 == 0
}

/// Finds sensitive values in `text`, longest first (so redaction never
/// splits a longer secret). Extra terms: SECRETS="Acme Corp;Project X".
pub fn find(text: &str) -> Vec<Found> {
    let mut out: Vec<Found> = vec![];
    let mut seen: HashSet<String> = HashSet::new();
    let mut add = |v: &str, kind: &'static str| {
        let v = v.trim();
        if v.len() >= 3 && seen.insert(v.to_string()) {
            out.push(Found { value: v.to_string(), kind });
        }
    };

    // Structured secrets
    let rules: [(&str, &str); 4] = [
        ("api_key", r"\b(?:sk-[A-Za-z0-9_-]{16,}|AIza[A-Za-z0-9_-]{30,}|AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{30,})"),
        ("email", r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}"),
        ("iban", r"\b[A-Z]{2}\d{2}[A-Z0-9]{11,30}\b"),
        ("phone", r"(?:\+\d{1,3}|\b0)[ -]?\d{2}[ -]?\d{3}[ -]?\d{4}\b"),
    ];
    for (kind, pat) in rules {
        for m in Regex::new(pat).unwrap().find_iter(text) {
            add(m.as_str(), kind);
        }
    }

    // Card numbers / 13-digit ID numbers: digit runs that pass the Luhn check
    for m in Regex::new(r"\b\d(?:[ -]?\d){12,18}\b").unwrap().find_iter(text) {
        let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
        if (13..=19).contains(&digits.len()) && luhn(&digits) {
            add(m.as_str(), if digits.len() == 13 { "id_number" } else { "card" });
        }
    }

    // Names: capitalised words after a cue word (from, to, for, Mr, client...)
    let names = Regex::new(
        r"(?i:\b(?:from|to|for|by|customer|client|named|mr|mrs|ms|dr)\b\.?)\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)?)",
    )
    .unwrap();
    for c in names.captures_iter(text) {
        let name: Vec<&str> = c[1]
            .split_whitespace()
            .take_while(|w| !STOP.contains(w))
            .collect();
        if !name.is_empty() {
            add(&name.join(" "), "name");
        }
    }

    // User-supplied extras
    if let Ok(extra) = std::env::var("SECRETS") {
        for s in extra.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            if text.contains(s) {
                add(s, "custom");
            }
        }
    }

    out.sort_by(|a, b| b.value.len().cmp(&a.value.len()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(t: &str) -> Vec<String> {
        find(t).into_iter().map(|f| f.value).collect()
    }

    #[test]
    fn finds_card_email_name() {
        let v = values("Refund from Alice Smith, card 4111 1111 1111 1111, mail alice@example.com.");
        assert!(v.contains(&"4111 1111 1111 1111".to_string()));
        assert!(v.contains(&"alice@example.com".to_string()));
        assert!(v.contains(&"Alice Smith".to_string()));
    }

    #[test]
    fn luhn_rejects_random_digits() {
        assert!(!values("order 4111 1111 1111 1112 shipped").iter().any(|v| v.contains("4111")));
    }

    #[test]
    fn stop_words_are_not_names() {
        assert!(values("Send it to Finance Team today").is_empty());
    }
}