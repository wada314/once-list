// Copyright 2021 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(feature = "nightly", feature(allocator_api))]
#![cfg_attr(feature = "nightly", feature(box_into_inner))]

use ::allocator_api2::alloc::{Allocator, Global};
use ::allocator_api2::boxed::Box;
use ::std::fmt::Debug;

#[cfg(not(feature = "sync"))]
use ::std::cell::OnceCell;
#[cfg(feature = "sync")]
use ::std::sync::OnceLock as OnceCell;

/// A single linked list which behaves like [`std::cell::OnceCell`], but for multiple values.
/// See the crate document for the examples.
#[derive(Clone)]
pub struct OnceList<T, A: Allocator = Global> {
    head: OnceCell<Box<Cons<T, A>, A>>,
    alloc: A,
}

#[derive(Clone)]
struct Cons<T, A: Allocator> {
    next: OnceCell<Box<Cons<T, A>, A>>,
    val: T,
}

impl<T> OnceList<T, Global> {
    /// Creates a new empty `OnceList`. This method does not allocate.
    pub fn new() -> Self {
        Self {
            head: OnceCell::new(),
            alloc: Global,
        }
    }
}
impl<T, A: Allocator> OnceList<T, A> {
    /// Creates a new empty `OnceList` with the given allocator. This method does not allocate.
    pub fn new_in(alloc: A) -> Self {
        Self {
            head: OnceCell::new(),
            alloc,
        }
    }

    /// Returns a first value, if it exists.
    pub fn first(&self) -> Option<&T> {
        self.head.get().map(|c| &c.val)
    }

    /// Returns a last value, if it exists.
    pub fn last(&self) -> Option<&T> {
        let mut last_opt = None;
        let mut next_cell = &self.head;
        while let Some(next_box) = next_cell.get() {
            last_opt = Some(&next_box.val);
            next_cell = &next_box.next;
        }
        last_opt
    }

    /// Returns an iterator over the `&T` references in the list.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let mut next_cell = &self.head;
        ::std::iter::from_fn(move || match next_cell.get() {
            Some(c) => {
                next_cell = &c.next;
                Some(&c.val)
            }
            None => None,
        })
    }

    /// Clears the list, dropping all values.
    pub fn clear(&mut self) {
        self.head = OnceCell::new();
    }

    /// Returns an allocator of this struct.
    pub fn allocator(&self) -> &A {
        &self.alloc
    }
}

impl<T: Sized, A: Allocator + Clone> OnceList<T, A> {
    /// Appends a value to the list, and returns the reference to that value.
    ///
    /// Note that this method takes `&self`, not `&mut self`.
    pub fn push(&self, val: T) -> &T {
        let mut next_cell = &self.head;
        let mut boxed_cons = Box::new_in(Cons::new(val), self.alloc.clone());
        loop {
            match next_cell.set(boxed_cons) {
                Ok(()) => {
                    // Safe because we are sure the `next` value is set.
                    return &unsafe { next_cell.get().unwrap_unchecked() }.val;
                }
                Err(new_val_cons) => {
                    // Safe because we are sure the `next` value is set.
                    next_cell = &unsafe { next_cell.get().unwrap_unchecked() }.next;
                    boxed_cons = new_val_cons;
                }
            }
        }
    }

    /// Find a first value in the list matches the predicate, remove that item from the list,
    /// and then returns that value.
    pub fn remove<P>(&mut self, mut pred: P) -> Option<T>
    where
        P: FnMut(&T) -> bool,
    {
        let mut next_cell = &mut self.head;
        while let Some(next_box) = next_cell.take() {
            if pred(&next_box.val) {
                let mut next_val = Box::into_inner(next_box);

                // reconnect the list
                if let Some(next_next) = next_val.next.take() {
                    let _ = next_cell.set(next_next);
                }
                return Some(next_val.val);
            }

            let _ = next_cell.set(next_box);
            // Safe because we are sure the `next_cell` value is set.
            next_cell = &mut unsafe { next_cell.get_mut().unwrap_unchecked() }.next;
        }
        None
    }
}

impl<T> Default for OnceList<T, Global> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Debug, A: Allocator> Debug for OnceList<T, A> {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

pub struct Iter<T, A: Allocator>(OnceCell<Box<Cons<T, A>, A>>);
impl<T, A: Allocator> IntoIterator for OnceList<T, A> {
    type Item = T;
    type IntoIter = Iter<T, A>;
    fn into_iter(self) -> Self::IntoIter {
        Iter(self.head)
    }
}

impl<T, A: Allocator> Iterator for Iter<T, A> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        let cons = Box::into_inner(self.0.take()?);
        let val = cons.val;
        self.0 = cons.next;
        Some(val)
    }
}

impl<T> FromIterator<T> for OnceList<T, Global> {
    fn from_iter<U: IntoIterator<Item = T>>(iter: U) -> Self {
        // TODO: O(n^2). Can optimize.
        let list = Self::new();
        for val in iter {
            list.push(val);
        }
        list
    }
}

impl<T, A: Allocator + Clone> Extend<T> for OnceList<T, A> {
    fn extend<U: IntoIterator<Item = T>>(&mut self, iter: U) {
        // TODO: O(n^2). Can optimize.
        for val in iter {
            self.push(val);
        }
    }
}

impl<T, A: Allocator> Cons<T, A> {
    fn new(val: T) -> Self {
        Self {
            next: OnceCell::new(),
            val,
        }
    }
}
