// Benches are test code: `clippy.toml` allows panics and indexing in tests, not in benches, and they count freely.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

//! A busy websocket server: a stream of tiny messages, each encoded or decoded on its own.
//!
//! Message shapes follow a game server's protocol:
//! - a response is `{ord, ..response}`, the response flattened into it with most of its optional fields skipped and an
//!   optional `rmpx::value::Value` result flattened in as well, so every response is a map of unknown length;
//! - a request is `{cmd: "name", seq, ..args}`, an internally tagged enum flattened into the message, so decoding goes
//!   through serde's buffered content.
//!
//! Plain structs and empty maps are the floor: the fixed cost of one `to_vec_named` / `from_slice` call.

// The allocator std uses on wasm32-unknown-unknown, so the wasm benches allocate as a browser build does.
#[cfg(target_family = "wasm")]
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::hint::black_box;
use std::num::NonZeroU32;

/// How many messages a stream bench handles per iteration.
const STREAM: usize = 1000;

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
struct Empty {}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Ack {
    seq: u32,
    status: u16,
    ct: f64,
}

/// A response: few fields set, the rest skipped.
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    seq: Option<NonZeroU32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cmd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keepalive_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_ord: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    responses: Vec<Ordered>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connect_count: Option<i32>,
    #[serde(flatten)]
    result: Option<rmpx::value::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ct: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
struct Ordered {
    ord: i32,
    #[serde(flatten)]
    response: Response,
}

#[allow(non_camel_case_types)]
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(tag = "cmd")]
enum Command {
    ping,
    collect { item_id: u32 },
    moveTo { x: i32, y: i32, target: String },
}

/// A request: the command flattened next to its sequence number.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Request {
    #[serde(flatten)]
    command: Command,
    seq: NonZeroU32,
}

fn seq(i: usize) -> NonZeroU32 {
    NonZeroU32::new(i as u32 + 1).expect("seq is never zero")
}

fn ack_response(i: usize) -> Ordered {
    Ordered {
        ord: i as i32,
        response: Response {
            seq: Some(seq(i)),
            status: Some(1),
            ct: Some(1_759_000_000.25 + i as f64),
            ..Response::default()
        },
    }
}

fn result_response(i: usize) -> Ordered {
    let result = rmpx::value::Value::Map(vec![
        ("coins".into(), (i as u64 * 7).into()),
        ("level".into(), 12.into()),
        ("item".into(), "gold_coin".into()),
    ]);
    Ordered {
        response: Response {
            result: Some(result),
            ..ack_response(i).response
        },
        ..ack_response(i)
    }
}

fn requests() -> Vec<Request> {
    (0..STREAM)
        .map(|i| Request {
            command: match i % 3 {
                0 => Command::ping,
                1 => Command::collect { item_id: i as u32 },
                _ => Command::moveTo {
                    x: i as i32,
                    y: -(i as i32),
                    target: "farm_plot".into(),
                },
            },
            seq: seq(i),
        })
        .collect()
}

fn named<T: Serialize>(value: &T) -> Vec<u8> {
    rmpx::to_vec_named(value).expect("encode")
}

fn encode_one<T: Serialize>(c: &mut Criterion, name: &str, value: &T) {
    c.bench_function(name, |b| {
        b.iter(|| rmpx::to_vec_named(black_box(value)).expect("encode"))
    });
}

fn decode_one<T: for<'de> Deserialize<'de>>(c: &mut Criterion, name: &str, buf: &[u8]) {
    c.bench_function(name, |b| {
        b.iter(|| rmpx::from_slice::<T>(black_box(buf)).expect("decode"))
    });
}

/// One message per call: the per-message cost of the smallest messages.
fn single(c: &mut Criterion) {
    let empty_map: BTreeMap<String, i64> = BTreeMap::new();
    let ack = Ack {
        seq: 42,
        status: 1,
        ct: 1_759_000_000.25,
    };
    let ack_response = ack_response(42);
    let result_response = result_response(42);
    let ping = Request {
        command: Command::ping,
        seq: seq(42),
    };
    let move_to = Request {
        command: Command::moveTo {
            x: 3,
            y: -4,
            target: "farm_plot".into(),
        },
        seq: seq(42),
    };

    encode_one(c, "ws/encode/empty_struct", &Empty {});
    encode_one(c, "ws/encode/empty_map", &empty_map);
    encode_one(c, "ws/encode/ack_struct", &ack);
    encode_one(c, "ws/encode/ack_response", &ack_response);
    encode_one(c, "ws/encode/result_response", &result_response);

    decode_one::<Empty>(c, "ws/decode/empty_struct", &named(&Empty {}));
    decode_one::<BTreeMap<String, i64>>(c, "ws/decode/empty_map", &named(&empty_map));
    decode_one::<Ack>(c, "ws/decode/ack_struct", &named(&ack));
    decode_one::<Request>(c, "ws/decode/ping_request", &named(&ping));
    decode_one::<Request>(c, "ws/decode/move_request", &named(&move_to));
    decode_one::<Ordered>(c, "ws/decode/result_response", &named(&result_response));
}

/// A stream of distinct messages, each in its own call and buffer, as a connection handles them.
fn stream(c: &mut Criterion) {
    let responses: Vec<Ordered> = (0..STREAM)
        .map(|i| {
            if i % 4 == 0 {
                result_response(i)
            } else {
                ack_response(i)
            }
        })
        .collect();
    let requests: Vec<Vec<u8>> = requests().iter().map(named).collect();

    let mut group = c.benchmark_group("ws/stream");
    group.throughput(Throughput::Elements(STREAM as u64));
    group.bench_function("encode_responses", |b| {
        b.iter(|| {
            for response in &responses {
                black_box(rmpx::to_vec_named(black_box(response)).expect("encode"));
            }
        })
    });
    group.bench_function("decode_requests", |b| {
        b.iter(|| {
            for request in &requests {
                black_box(rmpx::from_slice::<Request>(black_box(request)).expect("decode"));
            }
        })
    });
    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            for (request, response) in requests.iter().zip(&responses) {
                black_box(rmpx::from_slice::<Request>(black_box(request)).expect("decode"));
                black_box(rmpx::to_vec_named(black_box(response)).expect("encode"));
            }
        })
    });
    group.finish();
}

criterion_group!(benches, single, stream);
criterion_main!(benches);
