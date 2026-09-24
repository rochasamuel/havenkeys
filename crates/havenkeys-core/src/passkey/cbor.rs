//! The few CBOR shapes WebAuthn needs (RFC 8949 §4.2.1 deterministic
//! encoding): unsigned/negative integers, byte strings, text strings and
//! maps. Encoder only — HavenKeys never parses CBOR. Map entries are written
//! in the order given; callers pass them in CTAP2 canonical order.

pub(crate) enum Value<'a> {
    Int(i64),
    Bytes(&'a [u8]),
    Text(&'a str),
    Map(Vec<(Value<'a>, Value<'a>)>),
}

pub(crate) fn encode(value: &Value<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    write(value, &mut out);
    out
}

fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    let m = major << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n <= 0xff {
        out.extend_from_slice(&[m | 24, n as u8]);
    } else if n <= 0xffff {
        out.push(m | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(m | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

fn write(value: &Value<'_>, out: &mut Vec<u8>) {
    match value {
        Value::Int(i) if *i >= 0 => head(out, 0, *i as u64),
        // -1 - n, computed without overflow for i64::MIN.
        Value::Int(i) => head(out, 1, !(*i as u64)),
        Value::Bytes(b) => {
            head(out, 2, b.len() as u64);
            out.extend_from_slice(b);
        }
        Value::Text(t) => {
            head(out, 3, t.len() as u64);
            out.extend_from_slice(t.as_bytes());
        }
        Value::Map(entries) => {
            head(out, 5, entries.len() as u64);
            for (k, v) in entries {
                write(k, out);
                write(v, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(v: &Value<'_>) -> String {
        encode(v).iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn rfc8949_appendix_a() {
        for (n, want) in [
            (0, "00"),
            (1, "01"),
            (23, "17"),
            (24, "1818"),
            (100, "1864"),
            (1000, "1903e8"),
            (1_000_000, "1a000f4240"),
            (1_000_000_000_000, "1b000000e8d4a51000"),
            (-1, "20"),
            (-10, "29"),
            (-100, "3863"),
            (-1000, "3903e7"),
        ] {
            assert_eq!(hex(&Value::Int(n)), want, "{n}");
        }
        assert_eq!(hex(&Value::Bytes(&[])), "40");
        assert_eq!(hex(&Value::Bytes(&[1, 2, 3, 4])), "4401020304");
        assert_eq!(hex(&Value::Text("")), "60");
        assert_eq!(hex(&Value::Text("a")), "6161");
        assert_eq!(hex(&Value::Text("IETF")), "6449455446");
        assert_eq!(hex(&Value::Map(vec![])), "a0");
        assert_eq!(
            hex(&Value::Map(vec![
                (Value::Int(1), Value::Int(2)),
                (Value::Int(3), Value::Int(4)),
            ])),
            "a201020304"
        );
        assert_eq!(hex(&Value::Int(i64::MIN)), "3b7fffffffffffffff");
    }
}
