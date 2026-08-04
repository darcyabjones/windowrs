//! An iterator over overlapping subslices of a fixed size.
//!
//! The standard library already has [`std::slice::Windows`], but it only
//! ever yields full-size windows and always steps by 1. This crate covers
//! two cases that come up when chunking real data:
//!
//! - **A custom step.** [`Windows`] advances by `step` elements each time
//!   (`1 <= step <= size`), rather than always by 1, so windows can
//!   overlap or abut. `step` may not exceed `size`. Gapped windows that
//!   skip elements between them aren't supported.
//! - **A trailing partial window.** If the input doesn't divide evenly,
//!   [`std::slice::Windows`] silently drops the leftover elements at the
//!   end. [`Windows`] instead yields one final, shorter window so no data
//!   is lost.
//!
//! Each yielded [`Window`] carries the `start`/`end` indices (into the
//! original slice) of its `value`, so callers can always tell where a
//! window came from even after mapping over it.
//!
//! [`Windower`] adds a `.as_windows(size, step)` method to `[T]` (and, via
//! deref coercion, `Vec<T>` too) so you don't have to reach for
//! [`Windows::new`] and a slice directly; implement it for your own
//! container types the same way.
//!
//! # Examples
//!
//! ```rust
//! use windowrs::{Window, Windows};
//!
//! // size = 3, step = 2: windows overlap by one element, and the final
//! // window is shorter because 6 elements don't divide evenly by a step
//! // of 2 into windows of 3.
//! let elems = [1, 2, 3, 4, 5, 6];
//! let windows: Vec<_> = Windows::new(&elems, 3, 2).collect();
//!
//! assert_eq!(windows, vec![
//!     Window::new(0, 3, &[1, 2, 3][..]),
//!     Window::new(2, 5, &[3, 4, 5][..]),
//!     Window::new(4, 6, &[5, 6][..]),
//! ]);
//! ```
//!
//! The same thing, via [`Windower`]:
//!
//! ```rust
//! use windowrs::Windower;
//!
//! let elems = vec![1, 2, 3, 4, 5, 6];
//! let windows: Vec<_> = elems.as_windows(3, 2).collect();
//! assert_eq!(windows.len(), 3);
//! ```

#[derive(Debug, Clone)]
pub struct Windows<'a, T: 'a> {
    elements: &'a [T],
    size: usize,
    step: usize,
    start: usize,
}

impl<'a, T> Windows<'a, T> {
    /// Create a new windows object.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use windowrs::{Windows,Window};
    ///
    /// let mut win = Windows::new(&[1, 2, 3, 4], 2, 1);
    /// assert_eq!(win.next().unwrap(), Window::new(0, 2, &[1, 2][..]));
    /// assert_eq!(win.next().unwrap(), Window::new(1, 3, &[2, 3][..]));
    /// assert_eq!(win.next().unwrap(), Window::new(2, 4, &[3, 4][..]));
    /// assert!(win.next().is_none());
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `size == 0`, `step == 0`, or `step > size`.
    pub fn new(elements: &'a [T], size: usize, step: usize) -> Self {
        assert!(size > 0, "window size must be greater than 0, got {size}");
        assert!(step > 0, "window step must be greater than 0, got {step}");
        assert!(
            step <= size,
            "window step ({step}) must not be greater than window size ({size})"
        );
        Windows {
            elements,
            start: 0,
            size,
            step,
        }
    }

    /// The length of the iterator.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use windowrs::Windows;
    ///
    /// let win = Windows::new(&[1, 2, 3, 4], 2, 1);
    /// assert_eq!(win.len(), 3);
    ///
    /// let win = Windows::new(&[1, 2, 3, 4, 5], 3, 2);
    /// assert_eq!(win.len(), 2);
    /// ```
    pub fn len(&self) -> usize {
        if self.elements.is_empty() {
            return 0;
        }

        // Catches underflow because unsigned ints.
        let len = if self.elements.len() < self.size {
            0
        } else {
            self.elements.len() - self.size
        };

        let ndivs = len / self.step;
        let rem = if len % self.step == 0 { 0 } else { 1 };

        // +1 because cannot be empty at this point.
        ndivs + rem + 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The size of the window starting at `offset` elements into the
    /// current remaining slice, clamped to however much is actually left
    /// from there. This is the one formula `next`, `nth`, and `last` all need.
    #[inline]
    fn clamped_size(&self, offset: usize) -> usize {
        self.size.min(self.elements.len() - offset)
    }
}

impl<'a, T> Iterator for Windows<'a, T> {
    type Item = Window<&'a [T]>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.elements.is_empty() {
            return None;
        }

        let size = self.clamped_size(0);
        let ret = Window::new(self.start, self.start + size, &self.elements[..size]);

        self.start += self.step;
        if size == self.elements.len() {
            self.elements = &[];
        } else {
            self.elements = &self.elements[self.step..];
        }

        Some(ret)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let l = self.len();
        (l, Some(l))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if n >= self.len() {
            self.elements = &[];
            return None;
        }

        // `pos` is an offset into `self.elements` (the remaining slice),
        // not an absolute index into the original slice: `self.start`
        // tracks that separately, since `self.elements` shrinks as we go.
        let pos = n * self.step;
        let size = self.clamped_size(pos);
        let start = self.start + pos;
        let item = Window::new(start, start + size, &self.elements[pos..(pos + size)]);

        if pos + size == self.elements.len() {
            // This window consumed the entire remaining tail. Advancing by
            // just `step` (as the non-final case does) would strand
            // leftover elements whenever the final window is longer than
            // `step`, and a later `next()` would wrongly yield them as a
            // spurious extra window.
            self.elements = &[];
        } else {
            self.start = start + self.step;
            self.elements = &self.elements[(pos + self.step)..];
        }

        Some(item)
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        if self.elements.is_empty() {
            return None;
        }

        // `pos` is relative to `self.elements` (the remaining slice)
        // `self.start` is added back to report indices relative to the
        // original slice, same as `next()`/`nth()` do.
        let pos = (self.len() - 1) * self.step;
        let size = self.clamped_size(pos);
        let start = self.start + pos;
        let rec = Window::new(start, start + size, &self.elements[pos..(pos + size)]);
        Some(rec)
    }
}

impl<'a, T> ExactSizeIterator for Windows<'a, T> {
    #[inline]
    fn len(&self) -> usize {
        Windows::len(self)
    }
}

/// A `value` tagged with the `[start, end)` span (into some original slice)
/// that it was derived from.
///
/// `start` and `end` are plain informational indices, not an enforced
/// invariant: nothing in this crate relies on `start <= end` holding, the
/// fields are public and freely mutable, and [`Window::new`] does not
/// validate them. [`Windows`] itself only ever constructs windows with
/// `start <= end`, so this only matters if you build or mutate a `Window`
/// by hand. In that case, any relationship between `start` and `end` (or
/// none at all) is up to you.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Window<V> {
    pub start: usize,
    pub end: usize,
    pub value: V,
}

impl<V> Window<V> {
    /// Create a new window object.
    ///
    /// This does not validate `start` and `end` in any way, see the
    /// [`Window`] docs.
    pub fn new(start: usize, end: usize, value: V) -> Self {
        Window { start, end, value }
    }

    /// Converts from [`Window<V>`] to [`Window<&V>`].
    ///
    /// [`Window<V>`]: struct.Window.html
    /// [`Window<&V>`]: struct.Window.html
    pub fn as_ref(&self) -> Window<&V> {
        Window::new(self.start, self.end, &self.value)
    }

    /// Converts from [`Window<V>`] to [`Window<&mut V>`].
    ///
    /// [`Window<V>`]: struct.Window.html
    /// [`Window<&mut V>`]: struct.Window.html
    ///
    /// # Examples:
    ///
    /// ```
    /// use windowrs::Window;
    /// let mut win = Window::new(1, 10, 4.0);
    /// {
    ///     let win2 = win.as_mut();
    ///     *win2.value = 3.0;
    /// } // Mutable ref goes out of scope here
    /// assert_eq!(Window::new(1, 10, 3.0), win);
    /// ```
    pub fn as_mut(&mut self) -> Window<&mut V> {
        Window::new(self.start, self.end, &mut self.value)
    }

    /// Applies a function to the value in a window, leaving start and end
    /// unchanged.
    ///
    /// Examples:
    ///
    /// ```
    /// use windowrs::Window;
    /// let win = Window::new(1, 10, 3.0);
    /// let result = win.map(|x| x > 2.0);
    /// assert_eq!(result, Window::new(1, 10, true));
    /// ```
    pub fn map<U, F: FnOnce(V) -> U>(self, f: F) -> Window<U> {
        Window::new(self.start, self.end, f(self.value))
    }

    /// Applies a fuction taking the value and returning a new Window object.
    ///
    /// Examples:
    ///
    /// ```
    /// use windowrs::Window;
    /// let win = Window::new(1, 10, 3.0);
    /// let result = win.flat_map(|x| Window::new(0, 0, x));
    /// assert_eq!(Window::new(0, 0, 3.0), result);
    /// ```
    pub fn flat_map<U, F: FnOnce(V) -> Window<U>>(self, f: F) -> Window<U> {
        f(self.value)
    }
}

/// Types that can be turned into a [`Windows`] iterator over their elements.
///
/// This is a convenience trait for types (e.g. containers wrapping a slice)
/// that want to expose `.as_windows(size, step)` without the caller having
/// to reach for `Windows::new` and a slice directly. It's implemented here
/// only for `[T]`; implement it for your own container types the same way.
///
/// There's deliberately no separate `impl Windower for Vec<T>`: `Vec<T>`
/// derefs to `[T]`, and since `as_windows` takes `&self`, method-call syntax
/// (`my_vec.as_windows(size, step)`) already reaches the `[T]` impl through
/// that deref coercion — a second impl would just be a duplicate to keep in
/// sync. The one place this matters is a generic bound: `fn f<W:
/// Windower>(w: W)` is **not** satisfied by a bare `Vec<T>` (deref coercion
/// doesn't apply to trait bounds), so pass `&v[..]` or `v.as_slice()` at
/// the call site instead.
///
/// Named `as_windows` rather than `windows` to avoid colliding with the
/// inherent `<[T]>::windows` in `std`. Since inherent methods always win
/// over trait methods, a same-named trait method would be unreachable on
/// slices and would silently shadow `std`'s method on `Vec<T>` for anyone
/// who has `Windower` in scope.
pub trait Windower {
    /// The element type that the resulting windows will slide over.
    type Item;

    /// Build a [`Windows`] iterator over `self` with the given window
    /// `size` and `step`.
    ///
    /// See [`Windows::new`] for the meaning of `size` and `step`, including
    /// panics.
    fn as_windows(&self, size: usize, step: usize) -> Windows<'_, Self::Item>;
}

impl<T> Windower for [T] {
    type Item = T;

    fn as_windows(&self, size: usize, step: usize) -> Windows<'_, T> {
        Windows::new(self, size, step)
    }
}

#[cfg(test)]
mod test {
    use super::Window;
    use super::Windows;

    #[test]
    fn can_use_next() {
        let elems = &[1, 2, 3, 4];
        let mut win = Windows::new(elems, 2, 1);
        assert_eq!(win.next().unwrap(), Window::new(0, 2, &[1, 2][..]));
        assert_eq!(win.next().unwrap(), Window::new(1, 3, &[2, 3][..]));
        assert_eq!(win.next().unwrap(), Window::new(2, 4, &[3, 4][..]));
        assert!(win.next().is_none());

        let elems = &[1, 2, 3, 4, 5, 6, 7];
        let mut win = Windows::new(elems, 3, 1);
        assert_eq!(win.next().unwrap(), Window::new(0, 3, &[1, 2, 3][..]));
        assert_eq!(win.next().unwrap(), Window::new(1, 4, &[2, 3, 4][..]));
        assert_eq!(win.next().unwrap(), Window::new(2, 5, &[3, 4, 5][..]));
        assert_eq!(win.next().unwrap(), Window::new(3, 6, &[4, 5, 6][..]));
        assert_eq!(win.next().unwrap(), Window::new(4, 7, &[5, 6, 7][..]));
        assert!(win.next().is_none());
    }

    #[test]
    fn handles_incomplete_windows() {
        let elems = &[1, 2, 3, 4];
        let mut win = Windows::new(elems, 3, 2);
        assert_eq!(win.next().unwrap(), Window::new(0, 3, &[1, 2, 3][..]));
        assert_eq!(win.next().unwrap(), Window::new(2, 4, &[3, 4][..]));
        assert!(win.next().is_none());

        let elems = &[1, 2, 3, 4, 5, 6];
        let mut win = Windows::new(elems, 3, 2);
        assert_eq!(win.next().unwrap(), Window::new(0, 3, &[1, 2, 3][..]));
        assert_eq!(win.next().unwrap(), Window::new(2, 5, &[3, 4, 5][..]));
        assert_eq!(win.next().unwrap(), Window::new(4, 6, &[5, 6][..]));
        assert!(win.next().is_none());
    }

    #[test]
    fn handles_single_step() {
        let elems = &[1, 2, 3, 4];
        let mut win = Windows::new(elems, 4, 2);
        assert_eq!(win.next().unwrap(), Window::new(0, 4, &[1, 2, 3, 4][..]));
        assert!(win.next().is_none());

        let elems: &[u8] = b"This is";
        let mut win = Windows::new(elems, 20, 1);
        assert_eq!(win.next().unwrap(), Window::new(0, 7, b"This is" as &[u8]));
        assert!(win.next().is_none());
    }

    #[test]
    fn correct_len() {
        let elems = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let win = Windows::new(elems, 6, 2);
        assert_eq!(win.len(), 4);
        let win = Windows::new(elems, 7, 2);
        assert_eq!(win.len(), 4);
        let win = Windows::new(elems, 7, 3);
        assert_eq!(win.len(), 3);

        let elems = &[1, 2, 3, 4, 5, 6, 7];
        let win = Windows::new(elems, 3, 1);
        assert_eq!(win.len(), 5);
        let win = Windows::new(elems, 2, 1);
        assert_eq!(win.len(), 6);

        let elems = &[1, 2, 3, 4, 5, 6];
        let win = Windows::new(elems, 1, 1);
        assert_eq!(win.len(), 6);
        let win = Windows::new(elems, 3, 2);
        assert_eq!(win.len(), 3);
        let win = Windows::new(elems, 3, 3);
        assert_eq!(win.len(), 2);
        let win = Windows::new(elems, 4, 2);
        assert_eq!(win.len(), 2);
        let win = Windows::new(elems, 4, 3);
        assert_eq!(win.len(), 2);
        let win = Windows::new(elems, 6, 3);
        assert_eq!(win.len(), 1);
    }

    #[test]
    fn gets_last() {
        let elems = &[1, 2, 3, 4];
        let win = Windows::new(elems, 3, 2);
        assert_eq!(win.last().unwrap(), Window::new(2, 4, &[3, 4][..]));

        let elems = &[1, 2, 3, 4, 5, 6];
        let win = Windows::new(elems, 3, 2);
        assert_eq!(win.last().unwrap(), Window::new(4, 6, &[5, 6][..]));
    }

    #[test]
    fn gets_nth() {
        let elems = &[1, 2, 3, 4, 5, 6, 7];
        let mut win = Windows::new(elems, 3, 1);
        assert_eq!(win.nth(1).unwrap(), Window::new(1, 4, &[2, 3, 4][..]));
        assert_eq!(win.next().unwrap(), Window::new(2, 5, &[3, 4, 5][..]));

        let mut win = Windows::new(elems, 3, 1);
        assert_eq!(win.nth(3).unwrap(), Window::new(3, 6, &[4, 5, 6][..]));
        assert_eq!(win.next().unwrap(), Window::new(4, 7, &[5, 6, 7][..]));

        println!("last");
        let mut win = Windows::new(elems, 3, 2);
        assert_eq!(win.nth(1).unwrap(), Window::new(2, 5, &[3, 4, 5][..]));
        assert_eq!(win.next().unwrap(), Window::new(4, 7, &[5, 6, 7][..]));
        assert!(win.next().is_none());
    }

    #[test]
    fn maps_ok() {
        let elems: &[u32] = &[1, 1, 1, 2, 2, 2, 3, 3];
        let v: Vec<Window<u32>> = Windows::new(elems, 3, 1)
            .map(|w| w.map(|v| v.iter().cloned().fold(0u32, u32::max)))
            .collect();

        assert_eq!(v[0], Window::new(0, 3, 1));
        assert_eq!(v[5], Window::new(5, 8, 3));
    }
}
