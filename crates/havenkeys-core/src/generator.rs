//! Password generator backed by the OS CSPRNG.
//!
//! Characters are chosen with `rand::distr::Uniform`, which uses rejection
//! sampling (no modulo bias). One character from each enabled class is
//! guaranteed, then the result is shuffled with Fisher–Yates using the same
//! unbiased sampler.

use crate::error::{Error, Result};
use crate::secret::SecretString;
use rand::distr::{Distribution, Uniform};
use rand::rand_core::UnwrapErr;
use rand::rngs::SysRng;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const MIN_LENGTH: usize = 8;
pub const MAX_LENGTH: usize = 128;

const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.?/~";
const AMBIGUOUS: &[u8] = b"Il1O0o";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratorOptions {
    pub length: usize,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
    #[serde(default)]
    pub avoid_ambiguous: bool,
}

impl Default for GeneratorOptions {
    fn default() -> Self {
        Self {
            length: 24,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            avoid_ambiguous: false,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedPassword {
    pub password: SecretString,
    /// Upper-bound estimate: length × log2(alphabet size).
    pub entropy_bits: f64,
}

impl std::fmt::Debug for GeneratedPassword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GeneratedPassword(<redacted>)")
    }
}

fn class(chars: &[u8], avoid_ambiguous: bool) -> Vec<u8> {
    chars
        .iter()
        .copied()
        .filter(|c| !avoid_ambiguous || !AMBIGUOUS.contains(c))
        .collect()
}

fn pick<R: rand::Rng>(rng: &mut R, set: &[u8]) -> Result<u8> {
    let dist =
        Uniform::new(0, set.len()).map_err(|_| Error::InvalidInput("empty character set"))?;
    Ok(set[dist.sample(rng)])
}

pub fn generate(options: &GeneratorOptions) -> Result<GeneratedPassword> {
    if !(MIN_LENGTH..=MAX_LENGTH).contains(&options.length) {
        return Err(Error::InvalidInput("password length out of range"));
    }
    let classes: Vec<Vec<u8>> = [
        (options.uppercase, UPPER),
        (options.lowercase, LOWER),
        (options.digits, DIGITS),
        (options.symbols, SYMBOLS),
    ]
    .into_iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, chars)| class(chars, options.avoid_ambiguous))
    .collect();
    if classes.is_empty() {
        return Err(Error::InvalidInput("select at least one character type"));
    }
    let alphabet: Vec<u8> = classes.concat();

    let mut rng = UnwrapErr(SysRng);
    let mut out = Zeroizing::new(Vec::with_capacity(options.length));
    for set in &classes {
        out.push(pick(&mut rng, set)?);
    }
    while out.len() < options.length {
        out.push(pick(&mut rng, &alphabet)?);
    }
    // Fisher–Yates with unbiased index selection.
    for i in (1..out.len()).rev() {
        let j = Uniform::new_inclusive(0, i)
            .map_err(|_| Error::InvalidInput("shuffle"))?
            .sample(&mut rng);
        out.swap(i, j);
    }

    let password = String::from_utf8(out.to_vec()).map_err(|_| Error::InvalidInput("charset"))?;
    Ok(GeneratedPassword {
        password: SecretString::new(password),
        entropy_bits: options.length as f64 * (alphabet.len() as f64).log2(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn opts(length: usize, u: bool, l: bool, d: bool, s: bool) -> GeneratorOptions {
        GeneratorOptions {
            length,
            uppercase: u,
            lowercase: l,
            digits: d,
            symbols: s,
            avoid_ambiguous: false,
        }
    }

    #[test]
    fn respects_length_and_classes() {
        for _ in 0..200 {
            let p = generate(&opts(12, true, true, true, true))
                .unwrap()
                .password;
            let p = p.expose();
            assert_eq!(p.len(), 12);
            assert!(p.bytes().any(|c| UPPER.contains(&c)));
            assert!(p.bytes().any(|c| LOWER.contains(&c)));
            assert!(p.bytes().any(|c| DIGITS.contains(&c)));
            assert!(p.bytes().any(|c| SYMBOLS.contains(&c)));
        }
        let digits_only = generate(&opts(20, false, false, true, false))
            .unwrap()
            .password;
        assert!(digits_only.expose().bytes().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn rejects_invalid_options() {
        assert!(generate(&opts(7, true, true, true, true)).is_err());
        assert!(generate(&opts(129, true, true, true, true)).is_err());
        assert!(generate(&opts(16, false, false, false, false)).is_err());
    }

    #[test]
    fn avoid_ambiguous() {
        let o = GeneratorOptions {
            avoid_ambiguous: true,
            length: 128,
            ..Default::default()
        };
        for _ in 0..50 {
            let p = generate(&o).unwrap().password;
            assert!(!p.expose().bytes().any(|c| AMBIGUOUS.contains(&c)));
        }
    }

    #[test]
    fn outputs_are_unique() {
        let mut seen = HashSet::new();
        for _ in 0..2000 {
            assert!(seen.insert(
                generate(&GeneratorOptions::default())
                    .unwrap()
                    .password
                    .expose()
                    .to_owned()
            ));
        }
    }

    #[test]
    fn distribution_is_roughly_uniform() {
        // 10 digits, 200k samples: each should be ~10%. Loose bound (±0.6%) to
        // stay non-flaky while still catching gross bias.
        let mut counts: HashMap<u8, usize> = HashMap::new();
        let o = opts(100, false, false, true, false);
        for _ in 0..2000 {
            for c in generate(&o).unwrap().password.expose().bytes() {
                *counts.entry(c).or_default() += 1;
            }
        }
        let total: usize = counts.values().sum();
        for (_, n) in counts {
            let share = n as f64 / total as f64;
            assert!((share - 0.1).abs() < 0.006, "share {share}");
        }
    }

    #[test]
    fn entropy_estimate() {
        let g = generate(&opts(10, false, false, true, false)).unwrap();
        assert!((g.entropy_bits - 10.0 * 10f64.log2()).abs() < 1e-9);
    }
}
