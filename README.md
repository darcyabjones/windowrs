# windowrs

An iterator over overlapping subslices of a fixed size, with a configurable
step and a shorter final window instead of dropped elements.

`std::slice::Windows` covers the common case (step of 1, every window full
size), but drops any leftover elements at the end when the input doesn't
divide evenly. `windowrs` steps by any `step` from `1` up to `size` (the window length) and
always yields a final, possibly shorter, window so no data is lost. `step`
may not exceed `size`. Gapped windows that skip elements between them
aren't supported.

## Example

```rust
use windowrs::{Window, Windows};

let elems = [1, 2, 3, 4, 5, 6];
let windows: Vec<_> = Windows::new(&elems, 3, 2).collect();

assert_eq!(windows, vec![
    Window::new(0, 3, &[1, 2, 3][..]),
    Window::new(2, 5, &[3, 4, 5][..]),
    Window::new(4, 6, &[5, 6][..]),
]);
```

Each `Window` carries the `start`/`end` indices of its `value` within the
original slice, so you can still tell where a window came from after
mapping over it with `Window::map`.

## `Windower`

`[T]` gets a `.as_windows(size, step)` method, so you don't need to reach
for `Windows::new` and a slice directly. `Vec<T>` gets it too, for free,
via deref coercion to `[T]`. There's no separate `Vec<T>` impl:

```rust
use windowrs::Windower;

let elems = vec![1, 2, 3, 4, 5, 6];
let windows: Vec<_> = elems.as_windows(3, 2).collect();
assert_eq!(windows.len(), 3);
```

Implement the `Windower` trait the same way for your own container types.

Note that deref coercion only kicks in for method-call syntax like the one
above — it doesn't apply to generic trait bounds. A function written as
`fn f<W: Windower>(w: W)` is not satisfied by a bare `Vec<T>`; pass
`&v[..]` or `v.as_slice()` at the call site instead.

## MSRV

Requires Rust 1.85 or later (edition 2024).

## License

MIT
