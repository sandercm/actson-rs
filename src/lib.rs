//! # Actson
//!
//! A non-blocking, event-based JSON parser.
//!
//! ## Examples
//!
//! ### Push-based parsing
//!
//! Push-based parsing is the most flexible way of using Actson. Push new bytes
//! into a [`PushJsonFeeder`](feeder::PushJsonFeeder) and then let the parser
//! consume them until it returns [`Some(JsonEvent::NeedMoreInput)`](JsonEvent::NeedMoreInput).
//! Repeat this process until you receive `None`, which means the end of the
//! JSON text has been reached. The parser returns `Err` if the JSON text is
//! invalid or some other error has occurred.
//!
//! This approach is very low-level but gives you the freedom to provide new
//! bytes to the parser whenever they are available and to generate new JSON
//! events whenever you need them.
//!
//! ```
//! use actson::{JsonParser, JsonEvent};
//! use actson::feeder::{PushJsonFeeder, JsonFeeder};
//!
//! let json = r#"{"name": "Elvis"}"#.as_bytes();
//!
//! let feeder = PushJsonFeeder::new();
//! let mut parser = JsonParser::new(feeder);
//! let mut i = 0;
//! while let Some(event) = parser.next_event().unwrap() {
//!     match event {
//!         JsonEvent::NeedMoreInput => {
//!             // feed as many bytes as possible to the parser
//!             i += parser.feeder.push_bytes(&json[i..]);
//!             if i == json.len() {
//!                 parser.feeder.done();
//!             }
//!         }
//!
//!         JsonEvent::FieldName => assert!(matches!(parser.current_str(), Ok("name"))),
//!         JsonEvent::ValueString => assert!(matches!(parser.current_str(), Ok("Elvis"))),
//!
//!         _ => {} // there are many other event types you may process here
//!     }
//! }
//! ```
//!
//! ### Asynchronous parsing with Tokio
//!
//! Actson can be used with Tokio to parse JSON asynchronously.
//!
//! The main idea here is to call [`JsonParser::next_event()`] in a loop to
//! parse the JSON document and to produce events. Whenever you get
//! [`JsonEvent::NeedMoreInput`], call
//! [`AsyncBufReaderJsonFeeder::fill_buf()`](tokio::AsyncBufReaderJsonFeeder::fill_buf)
//! to asynchronously read more bytes from the input and to provide them to
//! the parser.
//!
//! *Heads up:* The `tokio` feature has to be enabled for this. It is disabled
//! by default.
//!
//! ```
//! use tokio::fs::File;
//! use tokio::io::{self, AsyncReadExt, BufReader};
//!
//! use actson::{JsonParser, JsonEvent};
//! use actson::tokio::AsyncBufReaderJsonFeeder;
//!
//! #[tokio::main]
//! async fn main() {
//!     let file = File::open("tests/fixtures/pass1.txt").await.unwrap();
//!     let reader = BufReader::new(file);
//!
//!     let feeder = AsyncBufReaderJsonFeeder::new(reader);
//!     let mut parser = JsonParser::new(feeder);
//!     while let Some(event) = parser.next_event().unwrap() {
//!         match event {
//!             JsonEvent::NeedMoreInput => parser.feeder.fill_buf().await.unwrap(),
//!             _ => {} // do something useful with the event
//!         }
//!     }
//! }
//! ```
//!
//! ### Parsing from a `BufReader`
//!
//! [`BufReaderJsonFeeder`](feeder::BufReaderJsonFeeder) allows you to
//! feed the parser from a [`BufReader`](std::io::BufReader).
//!
//! Note: By following this synchronous and blocking approach, you are missing
//! out on Actson's reactive properties. We recommend using Actson together
//! with Tokio instead to parse JSON asynchronously (see above).
//!
//! ```
//! use actson::{JsonParser, JsonEvent};
//! use actson::feeder::BufReaderJsonFeeder;
//!
//! use std::fs::File;
//! use std::io::BufReader;
//!
//! let file = File::open("tests/fixtures/pass1.txt").unwrap();
//! let reader = BufReader::new(file);
//!
//! let feeder = BufReaderJsonFeeder::new(reader);
//! let mut parser = JsonParser::new(feeder);
//! while let Some(event) = parser.next_event().unwrap() {
//!     match event {
//!         JsonEvent::NeedMoreInput => parser.feeder.fill_buf().unwrap(),
//!         _ => {} // do something useful with the event
//!     }
//! }
//! ```
//!
//! ### Parsing a slice of bytes
//!
//! For convenience, [`SliceJsonFeeder`](feeder::SliceJsonFeeder) allows
//! you to feed the parser from a slice of bytes.
//!
//! ```
//! use actson::{JsonParser, JsonEvent};
//! use actson::feeder::SliceJsonFeeder;
//!
//! let json = r#"{"name": "Elvis"}"#.as_bytes();
//!
//! let feeder = SliceJsonFeeder::new(json);
//! let mut parser = JsonParser::new(feeder);
//! while let Some(event) = parser.next_event().unwrap() {
//!     match event {
//!         JsonEvent::FieldName => assert!(matches!(parser.current_str(), Ok("name"))),
//!         JsonEvent::ValueString => assert!(matches!(parser.current_str(), Ok("Elvis"))),
//!         _ => {}
//!     }
//! }
//! ```
//!
//! ### Parsing into a Serde JSON Value
//!
//! For testing and compatibility reasons, Actson is able to parse a byte slice
//! into a [Serde JSON](https://github.com/serde-rs/json) Value.
//!
//! Heads up: You need to enable the `serde_json` feature for this.
//!
//! ```
//! use actson::serde_json::from_slice;
//!
//! let json = r#"{"name": "Elvis"}"#.as_bytes();
//! let value = from_slice(json).unwrap();
//!
//! assert!(value.is_object());
//! assert_eq!(value["name"], "Elvis");
//! ```
//!
//! However, if you find yourself doing this, you probably don't need the
//! reactive features of Actson and your data seems to completely fit into
//! memory. In this case, you're most likely better off using Serde JSON
//! directly.
//!
//! ### Parsing in streaming mode (multiple top-level JSON values)
//!
//! If you want to parse a stream of multiple top-level JSON values, you can enable
//! streaming mode. Values must be clearly separable. They must either be
//! self-delineating values (i.e. arrays, objects, strings) or keywords (i.e.
//! `true`, `false`, `null`), or they must be separated either by white space, at
//! least one self-delineating value, or at least one keyword.
//!
//! #### Example streams
//!
//! `1 2 3 true 4 5`
//!
//! `[1,2,3][4,5,6]{"key": "value"} 7 8 9`
//!
//! `"a""b"[1, 2, 3] {"key": "value"}`
//!
//! #### Example
//!
//! ```
//! use actson::feeder::SliceJsonFeeder;
//! use actson::options::JsonParserOptionsBuilder;
//! use actson::{JsonEvent, JsonParser};
//!
//! let json = r#"1 2""{"key":"value"}
//! ["a","b"]4true"#.as_bytes();
//!
//! let feeder = SliceJsonFeeder::new(json);
//! let mut parser = JsonParser::new_with_options(
//!     feeder,
//!     JsonParserOptionsBuilder::default()
//!         .with_streaming(true)
//!         .build(),
//! );
//!
//! let mut events = Vec::new();
//! while let Some(e) = parser.next_event().unwrap() {
//!     events.push(e);
//! }
//!
//! assert_eq!(events, vec![
//!     JsonEvent::ValueInt,
//!     JsonEvent::ValueInt,
//!     JsonEvent::ValueString,
//!     JsonEvent::StartObject,
//!     JsonEvent::FieldName,
//!     JsonEvent::ValueString,
//!     JsonEvent::EndObject,
//!     JsonEvent::StartArray,
//!     JsonEvent::ValueString,
//!     JsonEvent::ValueString,
//!     JsonEvent::EndArray,
//!     JsonEvent::ValueInt,
//!     JsonEvent::ValueTrue,
//! ]);
//! ```
pub mod event;
pub mod feeder;
pub mod options;
pub mod parser;

#[cfg(feature = "tokio")]
pub mod tokio;

#[cfg(feature = "serde_json")]
pub mod serde_json;

pub use event::JsonEvent;
pub use parser::JsonParser;
