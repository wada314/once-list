use ::allocator_api2::alloc;
use ::allocator_api2::alloc::Allocator;
use ::allocator_api2::boxed::Box;
use ::std::any::Any;
use ::std::ptr::NonNull;

use crate::cons::Cons;
use crate::once_list::OnceList;
use crate::tail_mode::TailMode;

impl<A: Allocator + Clone, M> OnceList<dyn Any, A, M>
where
    M: TailMode<dyn Any, A>,
{
    /// Pushes an aribitrary value to the list, and returns the reference to that value.
    ///
    /// ```rust
    /// use once_list2::OnceList;
    /// use std::any::Any;
    ///
    /// let list = OnceList::<dyn Any>::new();
    /// list.push_any(1);
    /// list.push_any("hello");
    ///
    /// assert_eq!(list.iter().nth(0).unwrap().downcast_ref::<i32>(), Some(&1));
    /// assert_eq!(list.iter().nth(1).unwrap().downcast_ref::<&str>(), Some(&"hello"));
    /// ```
    pub fn push_any<T: Any>(&self, val: T) -> &T {
        let sized_box = Box::new_in(Cons::<T, dyn Any, A>::new(val), A::clone(&self.alloc));
        // Because we are using the non-standard `Box`, we need to manually do the unsized coercions...
        // Watching the PR:
        // https://github.com/zakarumych/allocator-api2/pull/23
        let unsized_box = unsafe {
            let (sized_ptr, alloc) = Box::into_raw_with_allocator(sized_box);
            // Pointer unsized coercion!
            let unsized_ptr: *mut Cons<dyn Any, dyn Any, A> = sized_ptr;
            Box::from_raw_in(unsized_ptr, alloc)
        };
        self.push_inner(
            unsized_box,
            // Safe because we know the given value is type `T`.
            |c| c.downcast_ref::<T>().unwrap(),
        )
    }
}

impl<A: Allocator, M> OnceList<dyn Any, A, M>
where
    M: TailMode<dyn Any, A>,
{
    /// Finds the first value in the list that is the same type as `T`, and returns the reference to that value.
    ///
    /// ```rust
    /// use once_list2::OnceList;
    /// use std::any::Any;
    ///
    /// let list = OnceList::<dyn Any>::new();
    /// list.push_any(1);
    /// list.push_any("hello");
    ///
    /// assert_eq!(list.find_by_type::<i32>(), Some(&1));
    /// assert_eq!(list.find_by_type::<&str>(), Some(&"hello"));
    /// assert_eq!(list.find_by_type::<Vec<u8>>(), None);
    /// ```
    pub fn find_by_type<T: Any>(&self) -> Option<&T> {
        self.iter().find_map(|val| val.downcast_ref())
    }

    /// Removes the first value in the list that is the same type as `T`, and returns the value.
    ///
    /// ```rust
    /// use once_list2::OnceList;
    /// use std::any::Any;
    ///
    /// let mut list = OnceList::<dyn Any>::new();
    /// list.push_any(1);
    /// list.push_any("hello");
    ///
    /// assert_eq!(list.remove_by_type::<i32>(), Some(1));
    ///
    /// assert_eq!(list.len(), 1);
    /// assert_eq!(list.iter().nth(0).unwrap().downcast_ref::<&str>(), Some(&"hello"));
    /// ```
    pub fn remove_by_type<T: Any>(&mut self) -> Option<T> {
        self.remove_inner(
            |v| v.is::<T>(),
            |boxed_cons| {
                let cons_layout = alloc::Layout::for_value::<Cons<_, _, _>>(&boxed_cons);
                let (cons_ptr, alloc) = Box::into_raw_with_allocator(boxed_cons);

                let Cons {
                    next: next_ref,
                    val: val_any_ref,
                } = unsafe { &*cons_ptr };
                // Drop the `next` field.
                unsafe { ::std::ptr::read(next_ref) };

                let val_ref = <dyn Any>::downcast_ref::<T>(val_any_ref).unwrap();
                let val = unsafe { ::std::ptr::read(val_ref) };

                unsafe {
                    alloc.deallocate(NonNull::new_unchecked(cons_ptr as *mut u8), cons_layout);
                }

                val
            },
        )
    }
}

