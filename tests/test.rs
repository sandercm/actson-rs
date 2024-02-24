mod feeder;
mod prettyprinter;
mod tokio;

use std::fs;

use actson::event::ParseErrorKind;
use actson::feeder::PushJsonFeeder;
use actson::{JsonEvent, JsonParser};
use prettyprinter::PrettyPrinter;
use serde_json::Value;

/// Parse a JSON string and return a new JSON string generated by
/// [`PrettyPrinter`]. Assert that the input JSON string is valid.
fn parse(json: &str) -> String {
    let feeder = PushJsonFeeder::new();
    parse_with_parser(json, &mut JsonParser::new(feeder))
}

fn parse_with_parser(json: &str, parser: &mut JsonParser<PushJsonFeeder>) -> String {
    let buf = json.as_bytes();

    let mut prettyprinter = PrettyPrinter::new();
    let mut i: usize = 0;
    loop {
        // feed as many bytes as possible to the parser
        let mut e = parser.next_event();
        while e == JsonEvent::NeedMoreInput {
            i += parser.feeder.push_bytes(&buf[i..]);
            if i == json.len() {
                parser.feeder.done();
            }
            e = parser.next_event();
        }

        assert!(!matches!(e, JsonEvent::Error(_)));

        prettyprinter.on_event(e, parser).unwrap();

        if e == JsonEvent::Eof {
            break;
        }
    }

    prettyprinter.get_result().to_string()
}

/// Parse a JSON string and expect parsing to fail
fn parse_fail(json: &[u8]) -> ParseErrorKind {
    let feeder = PushJsonFeeder::new();
    parse_fail_with_parser(json, &mut JsonParser::new(feeder))
}

fn parse_fail_with_parser(json: &[u8], parser: &mut JsonParser<PushJsonFeeder>) -> ParseErrorKind {
    let mut i: usize = 0;
    loop {
        // feed as many bytes as possible to the parser
        let mut e = parser.next_event();
        while e == JsonEvent::NeedMoreInput {
            i += parser.feeder.push_bytes(&json[i..]);
            if i == json.len() {
                parser.feeder.done();
            }
            e = parser.next_event();
        }

        match e {
            JsonEvent::Error(k) => return k,
            JsonEvent::Eof => panic!("End of file before error happened"),
            _ => {}
        };
    }
}

/// Parse the given JSON string and check if the parser returns the correct number
/// of consumed bytes for each event produced
fn parse_checking_consumed_bytes(json: &str, events_bytes: &[(JsonEvent, usize)]) {
    let buf = json.as_bytes();
    let mut parser = JsonParser::new(PushJsonFeeder::new());
    for &(event, bytes) in events_bytes {
        let parsed_bytes = parser.parsed_bytes();
        let next_event = parse_until_next_event(&buf[parsed_bytes..], &mut parser);
        let parsed_bytes = parser.parsed_bytes();
        assert_eq!(next_event, event);
        assert_eq!(parsed_bytes, bytes);
    }
}

/// Parse the given JSON string and return the next event produced by the parser
fn parse_until_next_event(json: &[u8], parser: &mut JsonParser<PushJsonFeeder>) -> JsonEvent {
    let mut i: usize = 0;
    let mut event = parser.next_event();
    while event == JsonEvent::NeedMoreInput {
        i += parser.feeder.push_bytes(&json[i..]);
        if i == json.len() {
            parser.feeder.done();
        }
        event = parser.next_event();
    }
    event
}

fn assert_json_eq(expected: &str, actual: &str) {
    let em: Value = serde_json::from_str(expected).unwrap();
    let am: Value = serde_json::from_str(actual).unwrap();
    assert_eq!(em, am);
}

/// Test if valid files can be parsed correctly
#[test]
fn test_pass() {
    for i in 1..=3 {
        let json = fs::read_to_string(format!("tests/fixtures/pass{}.txt", i)).unwrap();
        assert_json_eq(&json, &parse(&json));
    }
}

#[test]
fn test_fail() {
    let feeder = PushJsonFeeder::new();
    let mut parser = JsonParser::new_with_max_depth(feeder, 16);
    for i in 2..=34 {
        let json = fs::read_to_string(format!("tests/fixtures/fail{}.txt", i)).unwrap();

        // ignore return value - we accept any error
        parse_fail_with_parser(json.as_bytes(), &mut parser);
    }
}

/// Test that an empty object is parsed correctly
#[test]
fn empty_object() {
    let json = r#"{}"#;
    assert_json_eq(json, &parse(json));
}

/// Test that a simple object is parsed correctly
#[test]
fn simple_object() {
    let json = r#"{"name": "Elvis"}"#;
    assert_json_eq(json, &parse(json));
}

/// Test that an empty array is parsed correctly
#[test]
fn empty_array() {
    let json = r#"[]"#;
    assert_json_eq(json, &parse(json));
}

/// Test that a simple array is parsed correctly
#[test]
fn simple_array() {
    let json = r#"["Elvis", "Max"]"#;
    assert_json_eq(json, &parse(json));
}

/// Test that an array with mixed values is parsed correctly
#[test]
fn mixed_array() {
    let json = r#"["Elvis", 132, "Max", 80.67]"#;
    assert_json_eq(json, &parse(json));
}

/// Test that a JSON text containing a UTF-8 character is parsed correctly
#[test]
fn utf8() {
    let json = "{\"name\": \"Bj\u{0153}rn\"}";
    assert_json_eq(json, &parse(json));
}

#[test]
fn too_many_next_event() {
    let json = "{}";
    let feeder = PushJsonFeeder::new();
    let mut parser = JsonParser::new(feeder);
    assert_json_eq(json, &parse_with_parser(json, &mut parser));
    assert!(matches!(
        parser.next_event(),
        JsonEvent::Error(ParseErrorKind::NoMoreInput)
    ));
}

#[test]
fn illegal_character() {
    let json = "{\"key\":\x02}";
    assert_eq!(
        parse_fail(json.as_bytes()),
        ParseErrorKind::IllegalCharacter
    );
}

#[test]
fn syntax_error() {
    let json = "{key}";
    assert_eq!(parse_fail(json.as_bytes()), ParseErrorKind::SyntaxError);
}

/// Make sure a number right before the end of the object can be parsed
#[test]
fn number_and_end_of_object() {
    let json = r#"{"n":2}"#;
    assert_json_eq(json, &parse(json));
}

/// Make sure a fraction can be parsed
#[test]
fn fraction() {
    let json = r#"{"n":2.1}"#;
    assert_json_eq(json, &parse(json));
}

/// Test that the parser does not accept illegal numbers ending with a dot
#[test]
fn illegal_number() {
    let json = r#"{"n":-2.}"#;
    assert_eq!(parse_fail(json.as_bytes()), ParseErrorKind::SyntaxError);
}

/// Make sure '0e1' can be parsed
#[test]
fn zero_with_exp() {
    let json = r#"{"n":0e1}"#;
    assert_json_eq(json, &parse(json));
}

/// Test if a top-level empty string can be parsed
#[test]
fn top_level_empty_string() {
    let json = r#""""#;
    assert_json_eq(json, &parse(json));
}

/// Test if a top-level 'false' can be parsed
#[test]
fn top_level_false() {
    let json = r#"false"#;
    assert_json_eq(json, &parse(json));
}

/// Test if a top-level integer can be parsed
#[test]
fn top_level_int() {
    let json = r#"42"#;
    assert_json_eq(json, &parse(json));
}

/// Test if a top-level long can be parsed
#[test]
fn top_level_long() {
    let json = r#"42123123123"#;
    assert_json_eq(json, &parse(json));
}

/// Make sure pre-mature end of file is detected correctly
#[test]
fn number_and_eof() {
    let json = r#"{"i":42"#;
    assert_eq!(parse_fail(json.as_bytes()), ParseErrorKind::NoMoreInput);
}

/// Test if a top-level zero can be parsed
#[test]
fn top_level_zero() {
    let json = r#"0"#;
    assert_json_eq(json, &parse(json));
}

/// Test if the parser returns an accurate amount when calling the `parsed_bytes()` method
#[test]
fn number_of_processed_bytes() {
    //                 16
    //  1     7        |17
    //  ↓     ↓        ↓↓
    //  {"name": "Elvis"}
    let json = r#"{"name": "Elvis"}"#;
    // the events and the corresponding bytes that are processed to produces them
    let events_bytes = [
        (JsonEvent::StartObject, 1),
        (JsonEvent::FieldName, 7),
        (JsonEvent::ValueString, 16),
        (JsonEvent::EndObject, 17),
        (JsonEvent::Eof, 17),
    ];
    parse_checking_consumed_bytes(json, &events_bytes);

    // 1      8     14    20      28
    // ↓      ↓     ↓     ↓       ↓
    // ["Elvis", 132, "Max", 80.67]
    let json = r#"["Elvis", 132, "Max", 80.67]"#;
    let events_bytes = [
        (JsonEvent::StartArray, 1),
        (JsonEvent::ValueString, 8),
        (JsonEvent::ValueInt, 14),
        (JsonEvent::ValueString, 20),
        (JsonEvent::ValueFloat, 28),
        (JsonEvent::EndArray, 28),
        (JsonEvent::Eof, 28),
    ];
    parse_checking_consumed_bytes(json, &events_bytes);

    // œ is encoded as: 0xC5 0x93, so it is 2 bytes long
    //                17
    // 1     7        |18
    // ↓     ↓        ↓↓
    // {"name": "Bjœrn"}
    let json = "{\"name\": \"Bj\u{0153}rn\"}";
    let events_bytes = [
        (JsonEvent::StartObject, 1),
        (JsonEvent::FieldName, 7),
        (JsonEvent::ValueString, 17),
        (JsonEvent::EndObject, 18),
        (JsonEvent::Eof, 18),
    ];
    parse_checking_consumed_bytes(json, &events_bytes);
}

/// Test if the parser is able to process all valid files from the test suite
#[test]
fn test_suite_pass() {
    let files = fs::read_dir("tests/json_test_suite/test_parsing").unwrap();
    for f in files {
        let f = f.unwrap();
        let name = f.file_name();
        if name.to_str().unwrap().starts_with('y') {
            let json = fs::read_to_string(f.path()).unwrap();
            if name == "y_number_minus_zero.json" || name == "y_number_negative_zero.json" {
                // -0 equals 0
                assert_eq!("[\n  0\n]", &parse(&json));
            } else {
                assert_json_eq(&json, &parse(&json));
            }
        }
    }
}

/// Test if the parser actually fails to process each invalid file from the test suite
#[test]
fn test_suite_fail() {
    let files = fs::read_dir("tests/json_test_suite/test_parsing").unwrap();
    for f in files {
        let f = f.unwrap();
        let name = f.file_name();
        if name.to_str().unwrap().starts_with('n') {
            let json = fs::read(f.path()).unwrap();
            parse_fail(&json); // ignore return value - we accept any error
        }
    }
}
