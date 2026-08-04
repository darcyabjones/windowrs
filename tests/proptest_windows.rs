//! Property tests for the public API: `Windows`, `Window`, and `Windower`.
//!
//! These only exercise the public surface (as a consumer of the crate
//! would), and are mainly aimed at proving that no valid input can make
//! `Windows`/`Window` panic, plus a handful of correctness invariants.

use proptest::prelude::*;
use windowrs::{Window, Windower, Windows};

/// `(size, step)` pairs satisfying `Windows::new`'s preconditions
/// (`size > 0`, `step > 0`, `step <= size`), kept small so shrinking stays
/// useful and cases run fast.
fn size_step() -> impl Strategy<Value = (usize, usize)> {
    (1usize..=8).prop_flat_map(|size| (Just(size), 1usize..=size))
}

fn elems() -> impl Strategy<Value = Vec<i32>> {
    prop::collection::vec(any::<i32>(), 0..64)
}

proptest! {
    /// Every public method should run to completion without panicking,
    /// including `nth` past the end of the iterator (the underflow /
    /// wrong-index / out-of-bounds bugs all showed up here).
    #[test]
    fn never_panics(
        v in elems(),
        (size, step) in size_step(),
        nth_arg in 0usize..70,
    ) {
        let win = Windows::new(&v, size, step);
        prop_assert_eq!(win.len(), win.size_hint().0);

        let mut win = Windows::new(&v, size, step);
        let _ = win.nth(nth_arg);
        // iterator must still be safely usable/exhaustible afterwards
        let _: Vec<_> = win.collect();

        let _ = Windows::new(&v, size, step).last();
        let _: Vec<_> = Windows::new(&v, size, step).collect();
    }

    /// The windows returned by full iteration exactly tile the input:
    /// contiguous starts advancing by `step`, correct slice values, and
    /// only the final window may be shorter than `size`.
    #[test]
    fn tiles_the_input_exactly(v in elems(), (size, step) in size_step()) {
        let windows: Vec<_> = Windows::new(&v, size, step).collect();

        if v.is_empty() {
            prop_assert!(windows.is_empty());
            return Ok(());
        }

        prop_assert_eq!(windows[0].start, 0);
        prop_assert_eq!(windows.last().unwrap().end, v.len());

        for pair in windows.windows(2) {
            prop_assert_eq!(pair[1].start, pair[0].start + step);
        }

        let last_index = windows.len() - 1;
        for (i, w) in windows.iter().enumerate() {
            prop_assert_eq!(w.value, &v[w.start..w.end]);
            prop_assert!(w.end - w.start <= size);
            if i != last_index {
                prop_assert_eq!(w.end - w.start, size);
            }
        }
    }

    /// `len()`/`size_hint()` must agree with the number of items actually
    /// produced by iterating to completion.
    #[test]
    fn len_matches_actual_count(v in elems(), (size, step) in size_step()) {
        let win = Windows::new(&v, size, step);
        let reported_len = win.len();
        let (lo, hi) = win.size_hint();

        let actual = Windows::new(&v, size, step).count();

        prop_assert_eq!(reported_len, actual);
        prop_assert_eq!(lo, actual);
        prop_assert_eq!(hi, Some(actual));
    }

    /// Standard iterator law: `nth(n)` must agree with discarding `n` items
    /// via `next()` and returning the next one — including when called
    /// after the iterator has already been partially advanced. Critically,
    /// this also checks the iterator's state *after* `nth`, not just its
    /// return value: `nth` landing on the final window used to strand
    /// leftover elements, which only shows up on the next call after it.
    #[test]
    fn nth_matches_repeated_next(
        v in elems(),
        (size, step) in size_step(),
        skip in 0usize..10,
        n in 0usize..70,
    ) {
        let mut a = Windows::new(&v, size, step);
        let mut b = Windows::new(&v, size, step);

        for _ in 0..skip {
            prop_assert_eq!(a.next(), b.next());
        }

        let mut expected = None;
        for _ in 0..=n {
            expected = b.next();
            if expected.is_none() {
                break;
            }
        }

        prop_assert_eq!(a.nth(n), expected);

        // The rest of `a` (post-`nth`) must match the rest of `b`
        // (post-repeated-`next`) exactly — same items, same count.
        let a_rest: Vec<_> = a.collect();
        let b_rest: Vec<_> = b.collect();
        prop_assert_eq!(a_rest, b_rest);
    }

    /// `last()` must agree with whatever `next()` produces at the very end
    /// of a full iteration — including when `last()` is called after the
    /// iterator has already been partially advanced, where it used to
    /// report `start`/`end` relative to the remaining slice instead of the
    /// original one.
    #[test]
    fn last_matches_final_next(v in elems(), (size, step) in size_step(), skip in 0usize..10) {
        let mut a = Windows::new(&v, size, step);
        let mut b = Windows::new(&v, size, step);

        for _ in 0..skip {
            prop_assert_eq!(a.next(), b.next());
        }

        let mut expected = None;
        for w in b {
            expected = Some(w);
        }

        prop_assert_eq!(a.last(), expected);
    }

    /// `Window::new` never panics and never adjusts `start`/`end` — it's a
    /// plain constructor, not a validating one (see the `Window` docs).
    #[test]
    fn window_new_never_panics_and_stores_as_given(start in 0usize..200, end in 0usize..200) {
        let w = Window::new(start, end, ());
        prop_assert_eq!(w.start, start);
        prop_assert_eq!(w.end, end);
    }

    /// `Windows::new` panics iff the (size, step) combination is invalid
    /// (`size == 0`, `step == 0`, or `step > size`) — never for any other
    /// reason, regardless of the input slice.
    #[test]
    fn windows_new_panics_iff_invalid_size_step(
        v in prop::collection::vec(any::<i32>(), 0..8),
        size in 0usize..6,
        step in 0usize..6,
    ) {
        let valid = size > 0 && step > 0 && step <= size;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Windows::new(&v, size, step)
        }));
        prop_assert_eq!(result.is_ok(), valid);
    }

    /// `map` transforms only `value`, leaving `start`/`end` untouched.
    #[test]
    fn map_preserves_span(start in 0usize..100, len in 0usize..100, value in any::<i32>()) {
        let w = Window::new(start, start + len, value);
        let mapped = w.clone().map(|x| x.wrapping_add(1));

        prop_assert_eq!(mapped.start, w.start);
        prop_assert_eq!(mapped.end, w.end);
        prop_assert_eq!(mapped.value, value.wrapping_add(1));
    }

    /// `as_ref`/`as_mut` expose the same span and value as the original,
    /// and mutating through `as_mut` is visible on the original.
    #[test]
    fn as_ref_as_mut_roundtrip(start in 0usize..100, len in 0usize..100, value in any::<i32>()) {
        let mut w = Window::new(start, start + len, value);

        let r = w.as_ref();
        prop_assert_eq!(r.start, start);
        prop_assert_eq!(r.end, start + len);
        prop_assert_eq!(*r.value, value);

        let m = w.as_mut();
        *m.value = value.wrapping_add(1);
        prop_assert_eq!(w.value, value.wrapping_add(1));
    }
}

proptest! {
    /// `Windower` is only implemented for `[T]`, but calling it on a
    /// `Vec<T>` must still work (via deref coercion to `[T]`) and agree
    /// exactly with calling `Windows::new` directly on the same data.
    #[test]
    fn windower_matches_direct_windows(v in elems(), (size, step) in size_step()) {
        let direct: Vec<_> = Windows::new(&v, size, step).collect();

        let via_vec: Vec<_> = v.as_windows(size, step).collect();
        prop_assert_eq!(&via_vec, &direct);

        let via_slice: Vec<_> = v.as_slice().as_windows(size, step).collect();
        prop_assert_eq!(&via_slice, &direct);
    }
}
