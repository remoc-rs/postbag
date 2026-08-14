use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

use serde::{Deserialize, Serialize};

use postbag::{from_full_slice, from_slim_slice};

thread_local! {
    /// Bytes currently held by this thread, and the high-water mark and
    /// number of allocations since it was last reset.
    ///
    /// Per thread rather than per process because each test runs on its own
    /// thread: shared counters would have every test measuring every other
    /// test that happened to run beside it.
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static PEAK: Cell<usize> = const { Cell::new(0) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

/// Tracks what the calling thread allocates, so a test can state what a piece
/// of input was allowed to cost.
struct Tracking;

unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // A thread on its way out has no counters left to update; the
        // allocation still has to go through.
        let _ = LIVE.try_with(|live| {
            let now = live.get() + layout.size();
            live.set(now);
            let _ = PEAK.try_with(|peak| peak.set(peak.get().max(now)));
            let _ = COUNT.try_with(|count| count.set(count.get() + 1));
        });

        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _ = LIVE.try_with(|live| live.set(live.get().saturating_sub(layout.size())));

        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Tracking = Tracking;

/// Runs `f` and returns how far this thread's live allocations rose.
///
/// `f` runs once before the measurement, so that any one-time initialisation
/// it triggers is not charged to it.
fn peak_allocation(mut f: impl FnMut()) -> usize {
    f();

    let before = LIVE.get();
    PEAK.set(before);
    f();

    PEAK.get().saturating_sub(before)
}

/// Runs `f` and returns how many allocations this thread made.
fn allocation_count(mut f: impl FnMut()) -> usize {
    f();

    let before = COUNT.get();
    f();

    COUNT.get() - before
}

/// A varint for `value`, to build input claiming more than it carries.
fn varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
    out
}

/// Generous enough to cover the reader's own working memory, far below what
/// the hostile inputs claim.
const ALLOWED: usize = 1 << 20;

#[test]
fn a_claimed_string_length_is_not_reserved() {
    let hostile = varint(1 << 34);

    let peak = peak_allocation(|| {
        let result = from_full_slice::<String>(&hostile);
        assert!(result.is_err(), "16 GB of string arrived in {} bytes", hostile.len());
    });

    assert!(peak < ALLOWED, "reserved {peak} bytes for {} bytes of input", hostile.len());
}

#[test]
fn a_claimed_byte_array_length_is_not_reserved() {
    let hostile = varint(1 << 34);

    let peak = peak_allocation(|| {
        assert!(from_full_slice::<serde_bytes::ByteBuf>(&hostile).is_err());
    });

    assert!(peak < ALLOWED, "reserved {peak} bytes for {} bytes of input", hostile.len());
}

#[test]
fn a_claimed_length_inside_a_block_is_not_reserved() {
    // The block says three bytes; the string inside it claims 16 GB.
    let mut hostile = vec![0x01, 0x41];
    let inner = varint(1 << 34);
    hostile.push(inner.len() as u8);
    hostile.extend(&inner);

    #[derive(Deserialize)]
    struct One {
        #[serde(rename = "_0")]
        _v: String,
    }

    let peak = peak_allocation(|| {
        assert!(from_full_slice::<One>(&hostile).is_err());
    });

    assert!(peak < ALLOWED, "reserved {peak} bytes for {} bytes of input", hostile.len());
}

#[test]
fn a_claimed_sequence_length_is_not_reserved() {
    // Element preallocation is Serde's to bound, and `size_hint::cautious`
    // stops at a megabyte's worth however many elements are claimed. The
    // point here is that the claim itself — sixteen billion `u64`, or 128 GB
    // — reaches no allocator.
    const SERDE_PREALLOC_CAP: usize = (1 << 20) + 1024;

    let hostile = varint(1 << 34);

    let peak = peak_allocation(|| {
        assert!(from_slim_slice::<Vec<u64>>(&hostile).is_err());
    });

    assert!(peak < SERDE_PREALLOC_CAP, "reserved {peak} bytes for {} bytes of input", hostile.len());
}

#[test]
fn a_claimed_identifier_length_is_not_reserved() {
    // Field count one, then an identifier claiming to be 16 GB of name.
    let mut hostile = vec![0x01, 0x40];
    hostile.extend(varint(1 << 34));

    #[derive(Deserialize)]
    struct One {
        #[serde(rename = "_0")]
        _v: u8,
    }

    let peak = peak_allocation(|| {
        assert!(from_full_slice::<One>(&hostile).is_err());
    });

    assert!(peak < ALLOWED, "reserved {peak} bytes for {} bytes of input", hostile.len());
}

#[test]
fn reading_a_struct_of_numbered_fields() {
    // A numbered identifier is borrowed from a table rather than formatted,
    // so reading one costs nothing. What is left is the reader asking its
    // input for a few bytes at a time.
    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Reading {
        #[serde(rename = "_0")]
        a: u32,
        #[serde(rename = "_1")]
        b: u32,
        #[serde(rename = "_2")]
        c: bool,
    }

    let value = Reading { a: 300, b: 7, c: true };
    let bytes = postbag::to_full_vec(&value).unwrap();

    let count = allocation_count(|| {
        assert_eq!(from_full_slice::<Reading>(&bytes).unwrap(), value);
    });
    println!("3 fields, {} bytes: {count} allocations", bytes.len());

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Wider {
        #[serde(rename = "_0")]
        a: u32,
        #[serde(rename = "_1")]
        b: u32,
        #[serde(rename = "_2")]
        c: bool,
        #[serde(rename = "_3")]
        d: u32,
        #[serde(rename = "_4")]
        e: u32,
        #[serde(rename = "_5")]
        f: bool,
    }

    let wide = Wider { a: 300, b: 7, c: true, d: 1, e: 2, f: false };
    let wide_bytes = postbag::to_full_vec(&wide).unwrap();
    let wide_count = allocation_count(|| {
        assert_eq!(from_full_slice::<Wider>(&wide_bytes).unwrap(), wide);
    });
    println!("6 fields, {} bytes: {wide_count} allocations", wide_bytes.len());

    // What matters either way is that this grows with the number of fields
    // and not with the number of bytes.
    if cfg!(postbag_fast_compile) {
        // The buffered path copies each field's bytes out and indexes the
        // field names before handing anything to the visitor.
        assert!(count <= 5 * 3, "reading three fields took {count} allocations");
        assert!(wide_count <= 5 * 6, "reading six fields took {wide_count} allocations");
    } else {
        // One per field: the box that `start_skippable` puts the reader in
        // for that field's block. Removing it means holding the open blocks
        // in one stack rather than nesting them.
        assert_eq!(count, 3, "reading three fields");
        assert_eq!(wide_count, 6, "reading six fields");
    }
}
