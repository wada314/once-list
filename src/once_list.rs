use ::allocator_api2::alloc::{Allocator, Global};
use ::allocator_api2::boxed::Box;
use ::std::fmt::Debug;
use ::std::hash::Hash;
#[cfg(feature = "nightly")]
use ::std::marker::Unsize;
use ::std::ops::DerefMut;

use crate::cache_mode::{CacheMode, NoCache, TailSlot, WithLen, WithTail, WithTailLen};
use crate::cons::Cons;
use crate::iter::{IntoIter, Iter, IterMut};

/// A single linked list which behaves like [`std::cell::OnceCell`], but for multiple values.
///
/// # Usage
///
/// A simple example:
///
/// ```rust
/// use once_list2::OnceList;
///
/// // Create a new empty list. Note that the variable is immutable.
/// let list = OnceList::<i32>::new();
///
/// // You can push values to the list without the need for mutability.
/// list.push(1);
/// list.push(2);
///
/// // Or you can push multiple values at once.
/// list.extend([3, 4, 5]);
///
/// // You can iterate over the list.
/// assert_eq!(list.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
///
/// // Some methods are mutable only.
/// let mut list_mut = list;
///
/// // You can remove (take) a value from the list.
/// let removed = list_mut.remove(|&x| x % 2 == 0);
/// assert_eq!(removed, Some(2));
/// assert_eq!(list_mut.iter().copied().collect::<Vec<_>>(), vec![1, 3, 4, 5]);
/// ```
///
/// # Caching modes (optional)
///
/// You can choose a mode depending on what you want to optimize:
///
/// - **No cache (default)**:
///   - `OnceList::<T>::new()`
///   - `OnceList::<T, A>::new_in(alloc)`
///   - `push()`: O(n), `len()`: O(n)
///
/// - **Len cache** (O(1) `len()`):
///   - `OnceList::new_with_len()`
///   - `OnceList::new_in_with_len(alloc)`
///   - `once_list2::OnceListWithLen<T, A>`
///
/// - **Tail cache** (fast repeated tail appends):
///   - `OnceList::new_with_tail()`
///   - `OnceList::new_in_with_tail(alloc)`
///   - `once_list2::OnceListWithTail<T, A>`
///
/// - **Tail + len cache**:
///   - `OnceList::new_with_tail_len()`
///   - `OnceList::new_in_with_tail_len(alloc)`
///   - `once_list2::OnceListWithTailLen<T, A>`
///
/// These modes keep the same behavior guarantees (including the iterator observing newly pushed values).
///
/// # Unsized types support
///
/// You can use the [unsized types] like `str`, `[u8]` or `dyn Display` as the value type of the `OnceList`.
///
/// If you are using the stable rust compiler, you can only use the `dyn Any` type as the unsized type.
/// (Strictly speaking, you can use ANY type as the unsized type, but you can't do any actual operations
/// like pushing, removing, etc.)
///
/// In the nightly compiler and with the `nightly` feature enabled, the additional methods like `push_unsized`
/// and `remove_unsized_as` become available:
///
/// ```rust
/// # #[cfg(not(feature = "nightly"))]
/// # fn main() {}
/// # #[cfg(feature = "nightly")]
/// # fn main() {
/// // This code only works with the nightly compiler and the `nightly` feature enabled.
///
/// use once_list2::OnceList;
///
/// // Creating a `OnceList` for `[i32]`, the unsized type.
/// let list = OnceList::<[i32]>::new();
///
/// list.push_unsized([1] /* A sized array type, `[i32; 1]`, can be coerced into [i32].*/);
/// list.push_unsized([2, 3] /* Same for `[i32; 2] type. */);
///
/// // The normal methods like `iter` are available because it returns a reference to the value.
/// assert_eq!(list.iter().nth(0).unwrap(), &[1]);
/// assert_eq!(list.iter().nth(1).unwrap(), &[2, 3]);
///
/// let mut list_mut = list;
///
/// // `remove_unsized_as` method allows you to check the unsized value type and remove it.
/// let removed: Option<[i32; 2]> = unsafe {
///     list_mut.remove_unsized_as(|x| if x.len() == 2 {
///         Some(x.try_into().unwrap())
///     } else {
///         None
///     })
/// };
/// // The removed value is an array, not a slice!
/// assert_eq!(removed, Some([2, 3]));
/// # }
/// ```
/// [unsized types]: https://doc.rust-lang.org/book/ch19-04-advanced-types.html#dynamically-sized-types-and-the-sized-trait
#[derive(Clone)]
pub struct OnceList<T: ?Sized, A: Allocator = Global, M = NoCache> {
    pub(crate) head: TailSlot<T, A>,
    pub(crate) alloc: A,
    pub(crate) mode: M,
}

pub type OnceListWithTail<T, A = Global> = OnceList<T, A, WithTail<T, A>>;
pub type OnceListWithLen<T, A = Global> = OnceList<T, A, WithLen<T, A>>;
pub type OnceListWithTailLen<T, A = Global> = OnceList<T, A, WithTailLen<T, A>>;

impl<T: ?Sized> OnceList<T, Global, NoCache> {
    /// Creates a new empty `OnceList`. This method does not allocate.
    pub fn new() -> Self {
        Self {
            head: TailSlot::new(),
            alloc: Global,
            mode: NoCache,
        }
    }
}

impl<T: ?Sized> OnceList<T, Global, NoCache> {
    /// Creates a new empty `OnceList` with tail caching enabled.
    pub fn new_with_tail() -> OnceListWithTail<T, Global> {
        OnceList {
            head: TailSlot::new(),
            alloc: Global,
            mode: WithTail::new(),
        }
    }

    /// Creates a new empty `OnceList` with length caching enabled.
    pub fn new_with_len() -> OnceListWithLen<T, Global> {
        OnceList {
            head: TailSlot::new(),
            alloc: Global,
            mode: WithLen::new(),
        }
    }

    /// Creates a new empty `OnceList` with tail and length caching enabled.
    pub fn new_with_tail_len() -> OnceListWithTailLen<T, Global> {
        OnceList {
            head: TailSlot::new(),
            alloc: Global,
            mode: WithTailLen::new(),
        }
    }
}

impl<T: ?Sized, A: Allocator> OnceList<T, A, NoCache> {
    /// Creates a new empty `OnceList` with the given allocator. This method does not allocate.
    pub fn new_in(alloc: A) -> Self {
        Self {
            head: TailSlot::new(),
            alloc,
            mode: NoCache,
        }
    }

    /// Creates a new empty `OnceList` with the given allocator and tail caching enabled.
    pub fn new_in_with_tail(alloc: A) -> OnceListWithTail<T, A> {
        OnceList {
            head: TailSlot::new(),
            alloc,
            mode: WithTail::new(),
        }
    }

    /// Creates a new empty `OnceList` with the given allocator and length caching enabled.
    pub fn new_in_with_len(alloc: A) -> OnceListWithLen<T, A> {
        OnceList {
            head: TailSlot::new(),
            alloc,
            mode: WithLen::new(),
        }
    }

    /// Creates a new empty `OnceList` with the given allocator and tail/len caching enabled.
    pub fn new_in_with_tail_len(alloc: A) -> OnceListWithTailLen<T, A> {
        OnceList {
            head: TailSlot::new(),
            alloc,
            mode: WithTailLen::new(),
        }
    }
}

impl<T: ?Sized, A: Allocator, M> OnceList<T, A, M> {
    /// Returns the number of values in the list.
    ///
    /// - O(1) if the current mode caches length
    /// - O(n) otherwise
    pub fn len(&self) -> usize
    where
        M: CacheMode<T, A>,
    {
        if let Some(n) = self.mode.cached_len() {
            return n;
        }
        self.iter().count()
    }

    /// Returns `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.head.get().is_none()
    }

    /// Returns `true` if the list contains the value.
    pub fn contains(&self, val: &T) -> bool
    where
        T: PartialEq,
    {
        self.iter().any(|v| v == val)
    }

    /// Returns a first value, if it exists.
    pub fn first(&self) -> Option<&T> {
        self.head.get().map(|c| &c.val)
    }

    /// Returns a mutable reference to the first value, if it exists.
    pub fn first_mut(&mut self) -> Option<&mut T> {
        self.head.get_mut().map(|c| &mut c.val)
    }

    /// Returns a last value, if it exists.
    /// This method is O(n).
    pub fn last(&self) -> Option<&T> {
        let mut last_opt = None;
        let mut next_cell = &self.head;
        while let Some(next_box) = next_cell.get() {
            last_opt = Some(&next_box.val);
            next_cell = &next_box.next;
        }
        last_opt
    }

    /// Returns a mutable reference to the last value, if it exists.
    /// This method is O(n).
    pub fn last_mut(&mut self) -> Option<&mut T> {
        let mut last_opt = None;
        let mut next_cell = &mut self.head;
        while let Some(next_box) = next_cell.get_mut() {
            let next_cons = Box::deref_mut(next_box);
            last_opt = Some(&mut next_cons.val);
            next_cell = &mut next_cons.next;
        }
        last_opt
    }

    pub(crate) fn last_cell(&self) -> &TailSlot<T, A> {
        let mut next_cell = &self.head;
        while let Some(next_box) = next_cell.get() {
            next_cell = &next_box.next;
        }
        next_cell
    }

    /// Returns an iterator over the `&T` references in the list.
    pub fn iter(&self) -> Iter<'_, T, A> {
        Iter::new(&self.head)
    }

    /// Returns an iterator over the `&mut T` references in the list.
    pub fn iter_mut(&mut self) -> IterMut<'_, T, A> {
        IterMut::new(&mut self.head)
    }

    /// Returns an allocator of this struct.
    pub fn allocator(&self) -> &A {
        &self.alloc
    }
}

impl<T: ?Sized, A: Allocator, M> OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    /// Clears the list, dropping all values.
    pub fn clear(&mut self) {
        self.head = TailSlot::new();
        self.mode.on_clear();
        self.mode.invalidate();
    }
}

impl<T: ?Sized, A: Allocator, M> OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    /// Removes the first value in the list that matches the predicate, and returns the value as a boxed value.
    ///
    /// This method supports the unsized value type `T` as well.
    ///
    /// Note that even though this method returns a boxed value, the box is something re-allcoated.
    /// So this method might not be efficient as you expect.
    #[cfg(feature = "nightly")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "nightly")))]
    pub fn remove_into_box<P>(&mut self, pred: P) -> Option<Box<T, A>>
    where
        P: FnMut(&T) -> bool,
    {
        self.remove_inner(pred, |boxed_cons| boxed_cons.box_into_inner_box())
    }

    /// Removes the first value in the list that matches the predicate, and returns the value.
    ///
    /// The predicate function `pred` should return `Some(&U)` if the value is found,
    /// and the returned reference `&U` must be the same address as the value given in the `pred`.
    ///
    /// # Safety
    /// This method is unsafe because it requires the predicate to return a reference to the same address as the value.
    #[cfg(feature = "nightly")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "nightly")))]
    pub unsafe fn remove_unsized_as<P, U>(&mut self, mut pred: P) -> Option<U>
    where
        P: FnMut(&T) -> Option<&U>,
    {
        use ::allocator_api2::alloc;

        let found_sized_ptr: OnceCell<*const U> = OnceCell::new();
        self.remove_inner(
            |val| {
                if let Some(val) = pred(val) {
                    // We only set the value once, so this is safe.
                    found_sized_ptr.set(val as *const U).unwrap();
                    true
                } else {
                    false
                }
            },
            |boxed_cons| -> U {
                // Given the boxed cons with the unsized value type `T`,
                // and returns the sized type value `U` by value (i.e. out of the box).

                // We are sure the `found_sized_ptr` is set.
                let found_sized_ptr: *const U = *found_sized_ptr.get().unwrap();

                let cons_layout = alloc::Layout::for_value::<Cons<T, T, A>>(&boxed_cons);
                let (cons_ptr, alloc) = Box::into_non_null_with_allocator(boxed_cons);
                let val_ptr = &unsafe { cons_ptr.as_ref() }.val as *const T;

                // Double check the ptr returned by the `pred` is the same as the pointer we extracted from the cons.
                debug_assert_eq!(val_ptr as *const U, found_sized_ptr);

                // Load (memcpy) the value into the output variable.
                let result = unsafe { ::std::ptr::read(val_ptr as *const U) };

                // Free the cons memory.
                unsafe { alloc.deallocate(cons_ptr.cast(), cons_layout) };

                result
            },
        )
    }

    /// An inner implementeation for `remove_xxx` methods.
    pub(crate) fn remove_inner<P, F, U>(&mut self, mut pred: P, mut f: F) -> Option<U>
    where
        P: FnMut(&T) -> bool,
        F: FnMut(Box<Cons<T, T, A>, A>) -> U,
    {
        // Any structural change through `&mut self` invalidates the cached tail slot.
        self.mode.invalidate();

        let mut next_cell = &mut self.head;
        while let Some(next_ref) = next_cell.get() {
            if pred(&next_ref.val) {
                // Safe because we are sure the `next_cell` value is set.
                let mut next_box = next_cell.take().unwrap();

                // reconnect the list
                if let Some(next_next) = next_box.next.take() {
                    let _ = next_cell.set(next_next);
                }

                self.mode.on_remove_success();
                return Some(f(next_box));
            }
            // Safe because we are sure the `next_cell` value is set.
            next_cell = &mut next_cell.get_mut().unwrap().next;
        }
        None
    }
}

impl<T: ?Sized, A: Allocator + Clone, M> OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    /// An unsized version of the [`OnceList::push`] method.
    ///
    /// You can push a sized value to the list. For exaple, you can push `[i32; 3]` to the list of `[i32]`.
    #[cfg(feature = "nightly")]
    #[cfg_attr(feature = "nightly", doc(cfg(feature = "nightly")))]
    pub fn push_unsized<U: Unsize<T>>(&self, val: U) -> &U {
        let boxed_cons = Cons::new_boxed(val, self.alloc.clone());
        self.push_inner(boxed_cons, |c| unsafe { &*(c as *const T as *const U) })
    }

    /// An inner implementation for the `push_xxx` methods.
    pub(crate) fn push_inner<F, U>(&self, mut new_cons: Box<Cons<T, T, A>, A>, f: F) -> &U
    where
        F: FnOnce(&T) -> &U,
    {
        let mut next_cell = self.mode.start_slot(&self.head);
        loop {
            match next_cell.try_insert2(new_cons) {
                Ok(new_cons) => {
                    self.mode.on_push_success(&new_cons.next);
                    return f(&new_cons.val);
                }
                Err((cur_cons, new_cons2)) => {
                    next_cell = &cur_cons.next;
                    new_cons = new_cons2;
                }
            }
        }
    }
}

impl<T, A: Allocator, M> OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    /// Find a first value in the list matches the predicate, remove that item from the list,
    /// and then returns that value.
    pub fn remove<P>(&mut self, mut pred: P) -> Option<T>
    where
        P: FnMut(&T) -> bool,
    {
        self.remove_inner(&mut pred, |boxed_cons| Box::into_inner(boxed_cons).val)
    }
}

impl<T, A: Allocator + Clone, M> OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    /// Appends a value to the list, and returns the reference to that value.
    ///
    /// Note that this method takes `&self`, not `&mut self`.
    pub fn push(&self, val: T) -> &T {
        let boxed_cons = Box::new_in(Cons::new(val), A::clone(&self.alloc));
        self.push_inner(boxed_cons, |c| c)
    }

    /// An almost same method with the [`std::iter::Extend::extend`],
    /// though this method takes `&self` instead of `&mut self`.
    ///
    /// [`std::iter::Extend::extend`]: https://doc.rust-lang.org/std/iter/trait.Extend.html#tymethod.extend
    pub fn extend<U: IntoIterator<Item = T>>(&self, iter: U) {
        let mut last_cell = self.last_cell();
        let alloc = self.allocator();
        for val in iter {
            let _ = last_cell.set(Box::new_in(Cons::new(val), A::clone(alloc)));
            let inserted = unsafe { &last_cell.get().unwrap_unchecked() };
            self.mode.on_push_success(&inserted.next);
            last_cell = &inserted.next;
        }
    }
}

impl<T: ?Sized> Default for OnceList<T, Global, NoCache> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: ?Sized + Debug, A: Allocator, M> Debug for OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<T: ?Sized + PartialEq, A: Allocator, M> PartialEq for OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<T: ?Sized + Eq, A: Allocator, M> Eq for OnceList<T, A, M> where M: CacheMode<T, A> {}

impl<T: ?Sized + Hash, A: Allocator, M> Hash for OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.len());
        for val in self.iter() {
            val.hash(state);
        }
    }
}

impl<T> FromIterator<T> for OnceList<T, Global, NoCache> {
    fn from_iter<U: IntoIterator<Item = T>>(iter: U) -> Self {
        let list = Self::new();
        let mut last_cell = &list.head;
        for val in iter {
            let _ = last_cell.set(Box::new(Cons::new(val)));
            last_cell = &unsafe { &last_cell.get().unwrap_unchecked() }.next;
        }
        list
    }
}

impl<T, A: Allocator, M> IntoIterator for OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    type Item = T;
    type IntoIter = IntoIter<T, A>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.head)
    }
}

impl<T, A: Allocator + Clone, M> Extend<T> for OnceList<T, A, M>
where
    M: CacheMode<T, A>,
{
    /// Due to the definition of the `Extend` trait, this method takes `&mut self`.
    /// Use the [`OnceList::extend`] method instead if you want to use `&self`.
    fn extend<U: IntoIterator<Item = T>>(&mut self, iter: U) {
        // Call the inherent `extend(&self, ..)` method.
        OnceList::<T, A, M>::extend(&*self, iter);
    }
}
