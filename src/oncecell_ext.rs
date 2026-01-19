use crate::OnceCell;

/// A workaround for the missing `OnceCell::try_insert` method.
pub(crate) trait OnceCellExt<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)>;
}

#[cfg(not(feature = "nightly"))]
impl<T> OnceCellExt<T> for OnceCell<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)> {
        // Both unsafe blocks are safe because it's sure the cell value is set.
        match self.set(value) {
            Ok(()) => Ok(unsafe { self.get().unwrap_unchecked() }),
            Err(value) => Err((unsafe { self.get().unwrap_unchecked() }, value)),
        }
    }
}

#[cfg(feature = "nightly")]
impl<T> OnceCellExt<T> for OnceCell<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)> {
        self.try_insert(value)
    }
}

