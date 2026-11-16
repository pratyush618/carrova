//! One-time-code detection for notification text.

use std::sync::LazyLock;

/// Words that mark a number as a code (`data/otp-words.txt`), lowercased.
static WORDS: LazyLock<Vec<Vec<char>>> = LazyLock::new(|| {
    include_str!("../../../data/otp-words.txt")
        .lines()
        .map(str::trim)
        .filter(|w| !w.is_empty() && !w.starts_with('#'))
        .map(|w| w.to_lowercase().chars().collect())
        .collect()
});
/// How far (in characters) a code word may sit from the digits.
const NEAR: usize = 30;

/// Whether `text` holds a 4-8 digit run within 30 characters of a code
/// word (matched whole: "pin" is not in "shipping").
pub(crate) fn looks_like_code(text: &str) -> bool {
    let chars: Vec<char> = text.to_lowercase().chars().collect();
    let runs = digit_runs(&chars);
    if runs.is_empty() {
        return false;
    }
    WORDS.iter().any(|word| {
        chars
            .windows(word.len())
            .enumerate()
            .filter(|(at, w)| *w == word.as_slice() && whole(&chars, *at, word.len()))
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

/// Neither neighbour of the match at `at..at + len` is a letter or digit.
fn whole(chars: &[char], at: usize, len: usize) -> bool {
    let edge = |i: Option<usize>| {
        i.and_then(|i| chars.get(i))
            .is_none_or(|c| !c.is_alphanumeric())
    };
    edge(at.checked_sub(1)) && edge(Some(at + len))
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
