
This library is a natural extension of the [`std::cell::OnceCell`](https://doc.rust-lang.org/std/cell/struct.OnceCell.html) (or its original crate [`once_cell`](https://crates.io/crates/once_cell)) library. This library provides a single-linked list `OnceList` type that allows you to store multiple values in a single `OnceList` instance even without the need for the mutability.

# Alternatives (Consider using these crates first!)

- [`once_cell`](https://crates.io/crates/once_cell) - The original crate that provides a `OnceCell` type. If you only need to store a single value, this crate is quite enough.
- [`elsa`](https://crates.io/crates/elsa) - A crate that provides `Frozen` collection types that allows you to store multiple values without the need for the mutability. They provides something similar to `Vec` or `HashMap`, so if your use case requires more than 3-ish values or you need more complex data structure than a single-linked list, then you should use this crate instead.

# Usage

A simple example:

```rust
use once_list2::OnceList;

// Create a new empty list. Note that the variable is immutable.
let list = OnceList::<i32>::new();

// You can push values to the list without the need for mutability.
list.push(1);
list.push(2);

// Or you can push multiple values at once.
list.extend([3, 4, 5]);

// You can iterate over the list.
assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);

// Some methods are mutable only.
let mut list_mut = list;

// You can remove (take) a value from the list.
let removed = list_mut.remove(|&x| x % 2 == 0);
assert_eq!(removed, Some(2));
assert_eq!(list_mut.iter().copied().collect::<Vec<_>>(), vec![1, 3, 4, 5]);

```
