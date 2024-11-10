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

    pub fn allocator(&self) -> &A {
        &self.alloc
    }
}

impl<T: Sized, A: Allocator + Clone> OnceList<T, A> {
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

    pub fn find<P: FnMut(&T) -> bool>(&self, mut pred: P) -> Option<&T> {
        let mut next_cell = &self.head;
        while let Some(c) = next_cell.get() {
            if pred(&c.val) {
                return Some(&c.val);
            }
            next_cell = &c.next;
        }
        None
    }

    pub fn find_map<F: FnMut(&T) -> Option<&U>, U>(&self, mut f: F) -> Option<&U> {
        let mut next_cell = &self.head;
        while let Some(c) = next_cell.get() {
            if let Some(u) = f(&c.val) {
                return Some(u);
            }
            next_cell = &c.next;
        }
        None
    }

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

    pub fn remove_map<F, U>(&mut self, mut f: F) -> Option<U>
    where
        F: FnMut(T) -> Result<U, T>,
    {
        let mut next_cell = &mut self.head;
        while let Some(next_box) = next_cell.take() {
            let next_val = Box::into_inner(next_box);
            match f(next_val.val) {
                Ok(u) => {
                    // reconnect the list
                    if let Some(next_next) = next_val.next.take() {
                        let _ = next_cell.set(next_next);
                    }
                    return Some(u);
                }
                Err(t) => {
                    next_val.val = t;
                    let _ = next_cell.set(next_box);
                    next_cell = &mut unsafe { next_cell.get_mut().unwrap_unchecked() }.next;
                }
            }
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
        let cons = self.0.take()?;
        let val = cons.val;
        self.0 = cons.next;
        Some(val)
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
