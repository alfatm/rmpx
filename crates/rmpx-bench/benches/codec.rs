// Benches are test code: `clippy.toml` allows panics and indexing in tests, not in benches, and they count freely.
#![allow(clippy::expect_used, clippy::indexing_slicing, clippy::arithmetic_side_effects)]

//! Encode/decode benchmarks shaped after how a game server uses the crates: small named messages,
//! large profile snapshots, `#[serde(flatten)]` structs and `rmpx::value::Value` trees.
//!
//! Real MessagePack files can be added with `RMPX_BENCH_FILES=/abs/a.msgpack:/abs/b.msgpack`. Benches run from the
//! package dir, so relative paths resolve against `crates/rmpx-bench`.

// The allocator std uses on wasm32-unknown-unknown, so the wasm benches allocate as a browser build does.
#[cfg(target_family = "wasm")]
#[global_allocator]
static ALLOCATOR: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::Path;

const WORDS: &[&str] = &[
    "id",
    "name",
    "level",
    "sword",
    "gold_coin",
    "quest_main_01",
    "энергия",
    "farm_plot",
    "decoration_big_tree",
    "x",
    "hp",
    "reward_chest_legendary",
];

const BIO: &str = "Игрок собирает урожай на ферме и выполняет задания. The player harvests crops and completes \
                   quests, торгует с соседями и украшает остров. ";

/// Deterministic xorshift, so every run benches the same data.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    /// Integers spread over every MessagePack integer width.
    fn int(&mut self) -> i64 {
        let v = self.next();
        match self.below(6) {
            0 => (v % 128) as i64,
            1 => -((v % 32) as i64),
            2 => (v % 256) as i64,
            3 => (v % 65_536) as i64,
            4 => (v % (1 << 32)) as i64,
            _ => v as i64,
        }
    }

    fn word(&mut self) -> String {
        WORDS[self.below(WORDS.len() as u64) as usize].to_owned()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum Kind {
    Chat,
    Move { x: i32, y: i32 },
    Leave(u8),
}

/// A client message: a few scalars, a short string, an enum and a small array.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Message {
    id: u64,
    seq: u32,
    kind: Kind,
    room: String,
    sender: Option<String>,
    payload: Vec<i64>,
    ok: bool,
    ratio: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Item {
    id: u32,
    template: String,
    count: i64,
    durability: Option<f32>,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Quest {
    stage: u16,
    progress: Vec<u32>,
    done: bool,
}

/// A player profile snapshot: many nested structs, maps, long UTF-8 text and binary data.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Profile {
    player_id: u64,
    nickname: String,
    level: u32,
    balance: i64,
    items: Vec<Item>,
    quests: BTreeMap<String, Quest>,
    bio: String,
    avatar: ByteBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Common {
    created_at: u64,
    owner: String,
}

/// `#[serde(flatten)]` makes serde serialize a map of unknown length.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Flattened {
    id: u64,
    #[serde(flatten)]
    common: Common,
    #[serde(flatten)]
    extra: BTreeMap<String, i64>,
}

fn message(rng: &mut Rng) -> Message {
    Message {
        id: rng.next(),
        seq: rng.below(10_000) as u32,
        kind: Kind::Move {
            x: rng.int() as i32,
            y: rng.int() as i32,
        },
        room: rng.word(),
        sender: Some(rng.word()),
        payload: (0..8).map(|_| rng.int()).collect(),
        ok: true,
        ratio: 0.75,
    }
}

fn profile(rng: &mut Rng, items: usize) -> Profile {
    Profile {
        player_id: rng.next(),
        nickname: rng.word(),
        level: rng.below(100) as u32,
        balance: rng.int(),
        items: (0..items)
            .map(|i| Item {
                id: i as u32,
                template: rng.word(),
                count: rng.int(),
                durability: (i % 3 == 0).then_some(0.5),
                tags: (0..rng.below(4)).map(|_| rng.word()).collect(),
            })
            .collect(),
        quests: (0..items / 10)
            .map(|i| {
                let quest = Quest {
                    stage: rng.below(10) as u16,
                    progress: (0..4).map(|_| rng.below(1000) as u32).collect(),
                    done: i % 2 == 0,
                };
                (format!("quest_{i}"), quest)
            })
            .collect(),
        bio: BIO.repeat(16),
        avatar: ByteBuf::from((0..4096).map(|_| rng.next() as u8).collect::<Vec<_>>()),
    }
}

fn flattened(rng: &mut Rng) -> Vec<Flattened> {
    (0..100)
        .map(|_| Flattened {
            id: rng.next(),
            common: Common {
                created_at: rng.next(),
                owner: rng.word(),
            },
            extra: (0..8).map(|i| (format!("{}_{i}", rng.word()), rng.int())).collect(),
        })
        .collect()
}

fn primitives(c: &mut Criterion) {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let ints: Vec<i64> = (0..1000).map(|_| rng.int()).collect();
    let strs: Vec<String> = (0..1000).map(|_| rng.word()).collect();

    let mut ints_buf = Vec::new();
    for &v in &ints {
        rmpx::raw::encode::write_sint(&mut ints_buf, v).expect("encode int");
    }
    let mut strs_buf = Vec::new();
    for s in &strs {
        rmpx::raw::encode::write_str(&mut strs_buf, s).expect("encode str");
    }

    let mut group = c.benchmark_group("primitives");
    group.throughput(Throughput::Elements(1000));

    let mut out = Vec::with_capacity(16 * 1024);
    group.bench_function("encode_ints", |b| {
        b.iter(|| {
            out.clear();
            for &v in &ints {
                rmpx::raw::encode::write_sint(&mut out, black_box(v)).expect("encode int");
            }
            black_box(&out);
        })
    });
    group.bench_function("encode_strs", |b| {
        b.iter(|| {
            out.clear();
            for s in &strs {
                rmpx::raw::encode::write_str(&mut out, black_box(s)).expect("encode str");
            }
            black_box(&out);
        })
    });
    group.bench_function("decode_ints", |b| {
        b.iter(|| {
            let mut rd = &ints_buf[..];
            for _ in 0..ints.len() {
                black_box(rmpx::raw::decode::read_int::<i64, _>(&mut rd).expect("decode int"));
            }
        })
    });
    group.bench_function("decode_strs", |b| {
        b.iter(|| {
            let mut rd = &strs_buf[..];
            for _ in 0..strs.len() {
                let (s, tail) = rmpx::raw::decode::read_str_from_slice(rd).expect("decode str");
                black_box(s);
                rd = tail;
            }
        })
    });
    group.finish();
}

fn message_bench(c: &mut Criterion) {
    let msg = message(&mut Rng(0x9e37_79b9_7f4a_7c15));
    let buf = rmpx::to_vec_named(&msg).expect("encode message");

    let mut group = c.benchmark_group("message");
    group.throughput(Throughput::Bytes(buf.len() as u64));
    group.bench_function("to_vec_named", |b| {
        b.iter(|| rmpx::to_vec_named(black_box(&msg)).expect("encode"))
    });
    group.bench_function("from_slice", |b| {
        b.iter(|| rmpx::from_slice::<Message>(black_box(&buf)).expect("decode"))
    });
    group.finish();
}

fn profile_bench(c: &mut Criterion) {
    let profile = profile(&mut Rng(0xdead_beef_cafe_babe), 500);
    let compact = rmpx::to_vec(&profile).expect("encode profile");
    let named = rmpx::to_vec_named(&profile).expect("encode profile");

    let mut group = c.benchmark_group("profile");
    group.throughput(Throughput::Bytes(named.len() as u64));
    group.bench_function("to_vec", |b| {
        b.iter(|| rmpx::to_vec(black_box(&profile)).expect("encode"))
    });
    group.bench_function("to_vec_named", |b| {
        b.iter(|| rmpx::to_vec_named(black_box(&profile)).expect("encode"))
    });
    group.bench_function("from_slice", |b| {
        b.iter(|| rmpx::from_slice::<Profile>(black_box(&compact)).expect("decode"))
    });
    group.bench_function("from_slice_named", |b| {
        b.iter(|| rmpx::from_slice::<Profile>(black_box(&named)).expect("decode"))
    });
    group.bench_function("from_read_named", |b| {
        b.iter(|| rmpx::from_read::<_, Profile>(black_box(&named[..])).expect("decode"))
    });
    group.bench_function("from_slice_ignored", |b| {
        b.iter(|| rmpx::from_slice::<IgnoredAny>(black_box(&named)).expect("decode"))
    });
    group.finish();
}

fn flatten_bench(c: &mut Criterion) {
    let values = flattened(&mut Rng(0x0123_4567_89ab_cdef));
    let buf = rmpx::to_vec_named(&values).expect("encode flattened");

    let mut group = c.benchmark_group("flatten");
    group.throughput(Throughput::Bytes(buf.len() as u64));
    group.bench_function("to_vec_named", |b| {
        b.iter(|| rmpx::to_vec_named(black_box(&values)).expect("encode"))
    });
    group.bench_function("from_slice", |b| {
        b.iter(|| rmpx::from_slice::<Vec<Flattened>>(black_box(&buf)).expect("decode"))
    });
    group.finish();
}

fn rmpv_bench(c: &mut Criterion) {
    let profile = profile(&mut Rng(0xfeed_face_d00d_f00d), 500);
    let buf = rmpx::to_vec_named(&profile).expect("encode profile");
    let value = rmpx::value::decode::read_value(&mut &buf[..]).expect("decode value");

    let mut group = c.benchmark_group("rmpv");
    group.throughput(Throughput::Bytes(buf.len() as u64));
    group.bench_function("read_value", |b| {
        b.iter(|| rmpx::value::decode::read_value(&mut black_box(&buf[..])).expect("decode"))
    });
    group.bench_function("read_value_ref", |b| {
        b.iter(|| rmpx::value::decode::read_value_ref(&mut black_box(&buf[..])).expect("decode"))
    });
    let mut out = Vec::with_capacity(buf.len());
    group.bench_function("write_value", |b| {
        b.iter(|| {
            out.clear();
            rmpx::value::encode::write_value(&mut out, black_box(&value)).expect("encode");
            black_box(&out);
        })
    });
    group.bench_function("to_value", |b| {
        b.iter(|| rmpx::value::ext::to_value(black_box(&profile)).expect("to_value"))
    });
    group.bench_function("from_value", |b| {
        b.iter_batched(
            || value.clone(),
            |value| rmpx::value::ext::from_value::<Profile>(value).expect("from_value"),
            BatchSize::LargeInput,
        )
    });
    group.finish();
}

/// Real files, decoded and encoded whole. `from_slice_value` and `to_vec_value` go through serde's `deserialize_any`
/// and a `Serialize` impl, as code that converts MessagePack to and from another dynamic representation does.
fn files_bench(c: &mut Criterion) {
    let Ok(paths) = std::env::var("RMPX_BENCH_FILES") else {
        return;
    };

    let mut group = c.benchmark_group("files");
    group.sample_size(10);
    for path in paths.split(':').filter(|p| !p.is_empty()) {
        let buf = std::fs::read(path).expect("read RMPX_BENCH_FILES file");
        let name = Path::new(path)
            .file_name()
            .map_or_else(|| path.to_owned(), |n| n.to_string_lossy().into_owned());
        let value = rmpx::value::decode::read_value(&mut &buf[..]).expect("decode value");

        group.throughput(Throughput::Bytes(buf.len() as u64));
        group.bench_with_input(BenchmarkId::new("from_slice_ignored", &name), &buf, |b, buf| {
            b.iter(|| rmpx::from_slice::<IgnoredAny>(buf).expect("decode"))
        });
        group.bench_with_input(BenchmarkId::new("read_value", &name), &buf, |b, buf| {
            b.iter(|| rmpx::value::decode::read_value(&mut &buf[..]).expect("decode"))
        });
        group.bench_with_input(BenchmarkId::new("read_value_ref", &name), &buf, |b, buf| {
            b.iter(|| rmpx::value::decode::read_value_ref(&mut &buf[..]).expect("decode"))
        });
        group.bench_with_input(BenchmarkId::new("from_slice_value", &name), &buf, |b, buf| {
            b.iter(|| rmpx::from_slice::<rmpx::value::Value>(buf).expect("decode"))
        });
        group.bench_with_input(BenchmarkId::new("to_vec_value", &name), &value, |b, value| {
            b.iter(|| rmpx::to_vec(value).expect("encode"))
        });
        let mut out = Vec::with_capacity(buf.len());
        group.bench_with_input(BenchmarkId::new("write_value", &name), &value, |b, value| {
            b.iter(|| {
                out.clear();
                rmpx::value::encode::write_value(&mut out, value).expect("encode");
                black_box(&out);
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    primitives,
    message_bench,
    profile_bench,
    flatten_bench,
    rmpv_bench,
    files_bench
);
criterion_main!(benches);
