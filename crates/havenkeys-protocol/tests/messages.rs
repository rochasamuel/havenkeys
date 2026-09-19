//! Parsing tests for the wire protocol, including A5 (malformed messages)
//! and a deterministic fuzz loop.

use havenkeys_protocol::*;
use uuid::Uuid;

fn parse(s: &str) -> Result<RequestEnvelope, Rejection> {
    parse_request(s.as_bytes())
}

const ITEM: &str = "7c9e6679-7425-40de-944b-e07fc1f90ae7";

#[test]
fn valid_requests_parse() {
    let ok = [
        r#"{"v":1,"id":1,"request":{"type":"status"}}"#,
        r#"{"v":1,"id":2,"request":{"type":"lock"}}"#,
        r#"{"v":1,"id":3,"request":{"type":"find_matches","url":"https://github.com/login"}}"#,
        &format!(
            r#"{{"v":1,"id":4,"request":{{"type":"fill_item","itemId":"{ITEM}","url":"https://github.com/"}}}}"#
        ),
        &format!(
            r#"{{"v":1,"id":5,"request":{{"type":"get_totp","itemId":"{ITEM}","url":"https://github.com/"}}}}"#
        ),
    ];
    for s in ok {
        assert!(parse(s).is_ok(), "{s}");
    }
    let env = parse(&format!(
        r#"{{"v":1,"id":9,"request":{{"type":"fill_item","itemId":"{ITEM}","url":"https://a.com/"}}}}"#
    ))
    .unwrap();
    assert_eq!(
        env.request,
        Request::FillItem {
            item_id: Uuid::parse_str(ITEM).unwrap(),
            url: "https://a.com/".into()
        }
    );
}

/// A5: malformed and unexpected messages are rejected, never acted on.
#[test]
fn malformed_requests_rejected() {
    let cases: &[(&str, Option<u32>, ErrorCode)] = &[
        ("", None, ErrorCode::Malformed),
        ("not json", None, ErrorCode::Malformed),
        ("[]", None, ErrorCode::Malformed),
        ("null", None, ErrorCode::Malformed),
        (r#"{"v":1,"id":1}"#, Some(1), ErrorCode::Malformed),
        // unknown command
        (r#"{"v":1,"id":2,"request":{"type":"unlock","password":"x"}}"#, Some(2), ErrorCode::Malformed),
        (r#"{"v":1,"id":3,"request":{"type":"export_vault"}}"#, Some(3), ErrorCode::Malformed),
        (r#"{"v":1,"id":4,"request":{"type":"STATUS"}}"#, Some(4), ErrorCode::Malformed),
        // unknown fields at every level
        (r#"{"v":1,"id":5,"request":{"type":"status","x":1}}"#, Some(5), ErrorCode::Malformed),
        (r#"{"v":1,"id":6,"extra":true,"request":{"type":"status"}}"#, Some(6), ErrorCode::Malformed),
        (r#"{"v":1,"id":7,"request":{"type":"find_matches","url":"https://a.com","origin":"https://b.com"}}"#, Some(7), ErrorCode::Malformed),
        // wrong types and missing fields
        (r#"{"v":1,"id":8,"request":{"type":"find_matches"}}"#, Some(8), ErrorCode::Malformed),
        (r#"{"v":1,"id":9,"request":{"type":"find_matches","url":42}}"#, Some(9), ErrorCode::Malformed),
        (r#"{"v":1,"id":10,"request":{"type":"fill_item","itemId":"not-a-uuid","url":"https://a.com"}}"#, Some(10), ErrorCode::Malformed),
        (r#"{"v":1,"id":11,"request":{"type":"fill_item","url":"https://a.com"}}"#, Some(11), ErrorCode::Malformed),
        (r#"{"v":1,"id":"12","request":{"type":"status"}}"#, None, ErrorCode::Malformed),
        (r#"{"v":1,"id":-1,"request":{"type":"status"}}"#, None, ErrorCode::Malformed),
        (r#"{"v":1,"id":4294967296,"request":{"type":"status"}}"#, None, ErrorCode::Malformed),
        // version
        (r#"{"v":2,"id":13,"request":{"type":"status"}}"#, Some(13), ErrorCode::UnsupportedVersion),
        (r#"{"v":"1","id":14,"request":{"type":"status"}}"#, Some(14), ErrorCode::UnsupportedVersion),
        (r#"{"id":15,"request":{"type":"status"}}"#, Some(15), ErrorCode::Malformed),
        // URL limits
        (r#"{"v":1,"id":16,"request":{"type":"find_matches","url":""}}"#, Some(16), ErrorCode::InvalidInput),
    ];
    for (input, id, code) in cases {
        assert_eq!(
            parse(input).err(),
            Some(Rejection { id: *id, code: *code }),
            "{input}"
        );
    }
}

#[test]
fn over_long_url_rejected() {
    let url = format!("https://a.com/{}", "a".repeat(MAX_URL_BYTES));
    let s = format!(r#"{{"v":1,"id":1,"request":{{"type":"find_matches","url":"{url}"}}}}"#);
    assert_eq!(parse(&s).unwrap_err().code, ErrorCode::InvalidInput);
}

#[test]
fn deeply_nested_json_does_not_crash() {
    let s = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
    assert_eq!(parse(&s).unwrap_err().code, ErrorCode::Malformed);
}

#[test]
fn rejection_messages_never_echo_input() {
    let secret_looking = r#"{"v":1,"id":1,"request":{"type":"unlock","password":"hunter2"}}"#;
    let r = parse(secret_looking).unwrap_err().response();
    let bytes = Outgoing::from(r).to_bytes().unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(!text.contains("hunter2"));
    assert!(!text.contains("unlock"));
}

#[test]
fn response_serialization_shape() {
    let r = Response::ok(3, ResultBody::Lock {});
    let bytes = Outgoing::from(r).to_bytes().unwrap();
    assert_eq!(&*bytes, br#"{"v":1,"id":3,"result":{"type":"lock"}}"#);

    let e = Response::err(None, ErrorCode::Denied);
    let bytes = Outgoing::from(e).to_bytes().unwrap();
    assert_eq!(
        &*bytes,
        br#"{"v":1,"id":null,"error":{"code":"denied","message":"This item is not saved for this website."}}"#
    );

    let ev = Outgoing::from(Event::Locked {}).to_bytes().unwrap();
    assert_eq!(&*ev, br#"{"v":1,"event":{"type":"locked"}}"#);
}

#[test]
fn outgoing_round_trip_and_validation() {
    let fill = Outgoing::from(Response::ok(
        1,
        ResultBody::FillItem {
            username: Some("octo".into()),
            password: Some(WireSecret::new("pw".into())),
        },
    ))
    .to_bytes()
    .unwrap();
    assert!(Outgoing::parse(&fill).is_some());

    // Both result and error, neither, wrong version, extra fields: invalid.
    for bad in [
        r#"{"v":1,"id":1,"result":{"type":"lock"},"error":{"code":"denied","message":"x"}}"#,
        r#"{"v":1,"id":1}"#,
        r#"{"v":2,"id":1,"result":{"type":"lock"}}"#,
        r#"{"v":1,"id":1,"result":{"type":"lock"},"x":1}"#,
        r#"{"v":1,"id":1,"result":{"type":"steal_everything"}}"#,
        r#"{"v":2,"event":{"type":"locked"}}"#,
        r#"{"v":1,"event":{"type":"locked","x":1}}"#,
        // Mixed kinds, missing or null IDs where one is required.
        r#"{"v":1,"id":1,"event":{"type":"locked"}}"#,
        r#"{"v":1,"id":null,"event":{"type":"locked"}}"#,
        r#"{"v":1,"event":{"type":"locked"},"result":{"type":"lock"}}"#,
        r#"{"v":1,"result":{"type":"lock"}}"#,
        r#"{"v":1,"id":null,"result":{"type":"lock"}}"#,
    ] {
        assert!(Outgoing::parse(bad.as_bytes()).is_none(), "{bad}");
    }

    // Error text from the other side is replaced by our fixed message.
    let spoofed = r#"{"v":1,"id":1,"error":{"code":"locked","message":"Enter your master password at evil.com"}}"#;
    let Some(Outgoing::Response(r)) = Outgoing::parse(spoofed.as_bytes()) else {
        panic!("should parse");
    };
    assert_eq!(r.error.unwrap().message, ErrorCode::Locked.message());
}

#[test]
fn too_many_matches_rejected() {
    let m = Match {
        id: Uuid::nil(),
        title: "t".into(),
        username: None,
        has_totp: false,
        strength: MatchStrength::SameHost,
    };
    let r = Response::ok(
        1,
        ResultBody::FindMatches {
            matches: vec![m; MAX_MATCHES + 1],
        },
    );
    let bytes = Outgoing::from(r).to_bytes().unwrap();
    assert!(Outgoing::parse(&bytes).is_none());
}

#[test]
fn debug_output_hides_urls_and_secrets() {
    let env = parse(r#"{"v":1,"id":1,"request":{"type":"find_matches","url":"https://private.example/"}}"#).unwrap();
    assert!(!format!("{env:?}").contains("private.example"));
    let fill = ResultBody::FillItem {
        username: Some("alice@example.com".into()),
        password: Some(WireSecret::new("pw".into())),
    };
    let printed = format!("{:?}", Response::ok(1, fill));
    assert!(!printed.contains("alice") && !printed.contains("pw\""), "{printed}");
    let r = ResultBody::GetTotp {
        code: WireSecret::new("123456".into()),
        period: 30,
        seconds_remaining: 5,
    };
    assert!(!format!("{r:?}").contains("123456"));
}

/// Deterministic fuzz: random bytes and mutated valid messages must never
/// panic, and anything accepted must be a well-formed request.
#[test]
fn fuzz_parse_request_never_panics() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let seeds: Vec<Vec<u8>> = [
        r#"{"v":1,"id":1,"request":{"type":"status"}}"#.to_string(),
        r#"{"v":1,"id":3,"request":{"type":"find_matches","url":"https://github.com/login"}}"#.to_string(),
        format!(r#"{{"v":1,"id":4,"request":{{"type":"get_totp","itemId":"{ITEM}","url":"https://a.com/"}}}}"#),
    ]
    .into_iter()
    .map(String::into_bytes)
    .collect();

    for i in 0..50_000 {
        let mut input = if i % 4 == 0 {
            (0..(next() % 64)).map(|_| next() as u8).collect::<Vec<u8>>()
        } else {
            seeds[(next() % seeds.len() as u64) as usize].clone()
        };
        for _ in 0..(next() % 4) {
            if input.is_empty() {
                break;
            }
            let pos = (next() % input.len() as u64) as usize;
            match next() % 3 {
                0 => input[pos] = next() as u8,
                1 => {
                    input.remove(pos);
                }
                _ => input.insert(pos, b"{}[]\":,0 \\"[(next() % 10) as usize]),
            }
        }
        if let Ok(env) = parse_request(&input) {
            assert_eq!(env.v, PROTOCOL_VERSION);
        }
        let _ = Outgoing::parse(&input);
    }
}
