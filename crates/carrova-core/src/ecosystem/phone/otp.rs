//! One-time-code detection for notification text.

/// Words that mark a number as a code (`data/otp-words.txt`).
const WORDS: &str = include_str!("../../../data/otp-words.txt");
/// How far (in characters) a code word may sit from the digits.
const NEAR: usize = 30;

/// Whether `text` holds a 4-8 digit run within 30 characters of a code word.
pub(crate) fn looks_like_code(text: &str) -> bool {
    let chars: Vec<char> = text.to_lowercase().chars().collect();
    let runs = digit_runs(&chars);
    if runs.is_empty() {
        return false;
    }
    WORDS
        .lines()
        .map(str::trim)
        .filter(|w| !w.is_empty() && !w.starts_with('#'))
        .any(|word| {
            let word: Vec<char> = word.to_lowercase().chars().collect();
            chars
                .windows(word.len())
                .enumerate()
                .filter(|(_, w)| *w == word.as_slice())
                .any(|(at, _)| {
                    runs.iter().any(|&(start, end)| {
                        let gap = if start >= at + word.len() {
                            start - (at + word.len())
                        } else {
                            at.saturating_sub(end)
                        };
                        gap <= NEAR
                    })
                })
        })
}

/// `(start, end)` of every maximal run of 4-8 ASCII digits.
fn digit_runs(chars: &[char]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if (4..=8).contains(&(i - start)) {
                runs.push((start, i));
            }
        } else {
            i += 1;
        }
    }
    runs
}

#[cfg(test)]
#[path = "../../../tests/ecosystem/phone/otp/unit.rs"]
mod tests;
