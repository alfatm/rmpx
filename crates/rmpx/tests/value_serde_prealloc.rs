//! Regression test for <https://github.com/3Hren/msgpack-rust/issues/151>: lengths come from the untrusted input, so a
//! tiny input announcing huge arrays and maps must not make the decoder reserve memory out of proportion to it.
//!
//! A counting allocator measures the peak, so this file holds a single test: other tests running in parallel in the
//! same binary would skew it.
// The counting allocator forwards to `System`: it needs `unsafe` to implement `GlobalAlloc`, and plain arithmetic on
// the byte counts.
#![allow(clippy::unwrap_used, clippy::arithmetic_side_effects, unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards every call to `System` unchanged, only counting the bytes.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: same contract as the caller's.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let current = CURRENT.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(current, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(ptr, layout) };
        CURRENT.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Extra memory at the peak of `decode`, above what was allocated before it.
fn peak_during(decode: impl FnOnce()) -> usize {
    let before = CURRENT.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    decode();
    PEAK.load(Ordering::Relaxed) - before
}

/// `header` repeated: every element starts the next nested container, none carries any data.
fn nested(header: &[u8]) -> Vec<u8> {
    header.repeat(1000)
}

#[test]
fn nested_huge_lengths_do_not_reserve_disproportionate_memory() {
    // Nesting up to the depth limit takes more stack than a test thread has in debug builds. Thread stacks are mapped
    // outside the global allocator, so the big stack does not show up in the counts.
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(check_nested_huge_lengths)
        .unwrap()
        .join()
        .unwrap();
}

fn check_nested_huge_lengths() {
    // 3000 bytes of input. Every nesting level may reserve a few KiB, the depth limit stops the nesting: a few MiB in
    // total. Reserving 64 KiB per level used 33-65 MB here, upstream before the fix reserved the announced length.
    const LIMIT: usize = 8 * 1024 * 1024;

    let arrays = nested(&[0xdc, 0xff, 0xff]); // Array16 of 65535 elements
    let maps = nested(&[0xde, 0xff, 0xff]); // Map16 of 65535 entries

    for (name, input) in [("arrays", &arrays), ("maps", &maps)] {
        let read_value = peak_during(|| assert!(rmpx::value::decode::read_value(&mut &input[..]).is_err()));
        let read_value_ref = peak_during(|| assert!(rmpx::value::decode::read_value_ref(&mut &input[..]).is_err()));
        let from_slice = peak_during(|| assert!(rmpx::from_slice::<rmpx::value::Value>(input).is_err()));

        for (decoder, peak) in [
            ("read_value", read_value),
            ("read_value_ref", read_value_ref),
            ("from_slice::<Value>", from_slice),
        ] {
            assert!(
                peak < LIMIT,
                "{decoder} on {} bytes of nested {name} reserved {peak} bytes",
                input.len()
            );
        }
    }
}
