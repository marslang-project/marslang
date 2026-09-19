//! Memory reclamation, measured with a counting allocator. This is its own test
//! binary so the allocator only observes these runs; a single test function
//! keeps the measurements from overlapping.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

struct Meter;
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Meter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = System.alloc(layout);
        if !p.is_null() {
            let live = LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed) + layout.size() as isize;
            PEAK.fetch_max(live, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        System.dealloc(p, layout);
    }
}

#[global_allocator]
static ALLOC: Meter = Meter;

/// (bytes still allocated after the run, peak bytes above the starting point)
fn measure(source: &str) -> (isize, isize) {
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let compiled = marslang::compile(source).unwrap();
    let (out, result) = marslang::run_captured(compiled, "");
    result.unwrap();
    drop(out);
    (LIVE.load(Ordering::Relaxed) - before, PEAK.load(Ordering::Relaxed) - before)
}

fn cycles(count: usize) -> String {
    format!("family Node{{ func init(){{ me.me2 = me; me.cb = me.get; }} func get => 1; }}
        func m{{ repeat {count} {{ a = arr(); a.add(a); p = pair(1, 2); p.first = p; n = Node(); }} }}")
}

#[test]
fn cycles_are_reclaimed_during_and_after_a_run() {
    measure("func m{}");
    for _ in 0..3 {
        let (plain, _) = measure("func m{ repeat 1000 { a=arr(); a.add(1); } }");
        let (cyclic, _) = measure("func m{ repeat 1000 { a=arr(); a.add(a); } }");
        assert_eq!(plain, 0, "ordinary arrays retained memory after the run");
        assert_eq!(cyclic, 0, "self-referencing arrays retained memory after the run");
    }
    // Garbage cycles are collected while the program runs, so peak memory does
    // not grow with the number of cycles created.
    let (retained_small, peak_small) = measure(&cycles(20_000));
    let (retained_large, peak_large) = measure(&cycles(200_000));
    assert_eq!((retained_small, retained_large), (0, 0));
    assert!(peak_large < peak_small * 2, "peak grew from {peak_small} to {peak_large} bytes");
}
