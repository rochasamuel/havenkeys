//! Strength of a password the user is choosing (the master password on first
//! run), from zxcvbn, the same estimator Vault health uses for logins. The
//! draft is scored and dropped; nothing here stores or logs it.

use serde::Serialize;

/// zxcvbn reads at most this many characters. Its matcher is quadratic in
/// length, and anything longer than this is not what the score is for.
const MAX_SCORED_CHARS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PasswordStrength {
    /// zxcvbn's 0 (guessed at once) to 4 (very hard to guess).
    pub score: u8,
    /// log10 of the estimated number of guesses, for a gauge.
    pub guesses_log10: f64,
}

/// Scores `password`; `user_inputs` (the account email, say) count as easy
/// to guess.
pub fn estimate(password: &str, user_inputs: &[&str]) -> PasswordStrength {
    let scored: String = password.chars().take(MAX_SCORED_CHARS).collect();
    let e = zxcvbn::zxcvbn(&scored, user_inputs);
    PasswordStrength {
        score: u8::from(e.score()),
        guesses_log10: e.guesses_log10(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_passwords_score_low_and_long_random_ones_high() {
        assert!(estimate("password1", &[]).score < 3);
        assert!(estimate("", &[]).score == 0);
        assert_eq!(estimate("k7#Pq9!vLm2@xZr4&wB8", &[]).score, 4);
    }

    #[test]
    fn the_account_email_is_an_easy_guess() {
        let with = estimate("samuel.rocha2024", &["samuel.rocha@example.com"]);
        let without = estimate("samuel.rocha2024", &[]);
        assert!(with.guesses_log10 <= without.guesses_log10);
    }

    #[test]
    fn very_long_input_is_scored_without_stalling() {
        let long = "ab".repeat(5_000);
        let s = estimate(&long, &[]);
        assert!(s.score <= 4);
    }
}
