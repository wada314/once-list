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

use crate::OnceCell;

/// A workaround for the missing `OnceCell::try_insert` method.
pub(crate) trait OnceCellExt<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)>;
}

#[cfg(not(feature = "nightly"))]
impl<T> OnceCellExt<T> for OnceCell<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)> {
        match self.set(value) {
            Ok(()) => match self.get() {
                Some(r) => Ok(r),
                None => unreachable!("OnceCell was just set but returned None"),
            },
            Err(value) => match self.get() {
                Some(r) => Err((r, value)),
                None => unreachable!("OnceCell::set returned Err but cell has no value"),
            },
        }
    }
}

#[cfg(feature = "nightly")]
impl<T> OnceCellExt<T> for OnceCell<T> {
    fn try_insert2(&self, value: T) -> Result<&T, (&T, T)> {
        self.try_insert(value)
    }
}
