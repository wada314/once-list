[![Crates.io Version](https://img.shields.io/crates/v/once_list2)](https://crates.io/crates/once-list2)
[![docs.rs](https://img.shields.io/docsrs/once-list2)](https://docs.rs/once-list2/latest/once_list2/)

This library is a natural extension of the [`std::cell::OnceCell`](https://doc.rust-lang.org/std/cell/struct.OnceCell.html) (or its original crate [`once_cell`](https://crates.io/crates/once_cell)) library. This library provides a single-linked list `OnceList` type that allows you to store multiple values in a single `OnceList` instance even without the need for the mutability.

# Alternatives (Consider using these crates first!)

- [`once_cell`](https://crates.io/crates/once_cell) - The original crate that provides a `OnceCell` type. If you only need to store a single value, this crate is quite enough.
- [`elsa`](https://crates.io/crates/elsa) - A crate that provides `Frozen` collection types that allows you to store multiple values without the need for the mutability. They provides something similar to `Vec` or `HashMap`, so if your use case requires more than 3-ish values or you need more complex data structure than a single-linked list, then you should use this crate instead.

# Features

By default, none of the features are enabled.

- `nightly`: Enables the nightly-only features.

  - Uses the `allocator_api` std unstable feature. Note that even without this feature, this crate still supports the allocators thanks to the [`allocator_api2`](https://crates.io/crates/allocator-api2) crate.
  - Supports the special methods for the unsized value types. See the doc of `OnceCell` for more details.

- `sync`: This library internally uses `std::cell::OnceCell` which is not thread-safe. When you enable this feature, this library uses the thread-safe `std::sync::OnceLock` instead.
