mod array_store;
mod bitmap_store;

use std::mem;
use std::ops::{
    BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, RangeInclusive, Sub, SubAssign,
};
use std::{slice, vec};

use self::bitmap_store::BITMAP_LENGTH;
use self::Store::{Array, Bitmap};

pub use self::array_store::ArrayStore;
pub use self::bitmap_store::{BitmapIter, BitmapStore};

#[derive(Clone)]
pub enum Store {
    Array(ArrayStore),
    Bitmap(BitmapStore),
}

pub enum Iter<'a> {
    Array(slice::Iter<'a, u16>),
    Vec(vec::IntoIter<u16>),
    BitmapBorrowed(BitmapIter<&'a [u64; BITMAP_LENGTH]>),
    BitmapOwned(BitmapIter<Box<[u64; BITMAP_LENGTH]>>),
}

impl Store {
    pub fn new() -> Store {
        Store::Array(ArrayStore::new())
    }

    pub fn insert(&mut self, index: u16) -> bool {
        match *self {
            Array(ref mut vec) => vec.insert(index),
            Bitmap(ref mut bits) => bits.insert(index),
        }
    }

    pub fn insert_range(&mut self, range: RangeInclusive<u16>) -> u64 {
        // A Range is defined as being of size 0 if start >= end.
        if range.is_empty() {
            return 0;
        }

        match *self {
            Array(ref mut vec) => vec.insert_range(range),
            Bitmap(ref mut bits) => bits.insert_range(range),
        }
    }

    /// Push `index` at the end of the store only if `index` is the new max.
    ///
    /// Returns whether `index` was effectively pushed.
    pub fn push(&mut self, index: u16) -> bool {
        match self {
            Array(vec) => vec.push(index),
            Bitmap(bits) => bits.push(index),
        }
    }

    ///
    /// Pushes `index` at the end of the store.
    /// It is up to the caller to have validated index > self.max()
    ///
    /// # Panics
    ///
    /// If debug_assertions enabled and index is > self.max()
    pub(crate) fn push_unchecked(&mut self, index: u16) {
        match self {
            Array(vec) => vec.push_unchecked(index),
            Bitmap(bits) => bits.push_unchecked(index),
        }
    }

    pub fn remove(&mut self, index: u16) -> bool {
        match *self {
            Array(ref mut vec) => vec.remove(index),
            Bitmap(ref mut bits) => bits.remove(index),
        }
    }

    pub fn remove_range(&mut self, range: RangeInclusive<u16>) -> u64 {
        if range.is_empty() {
            return 0;
        }

        match *self {
            Array(ref mut vec) => vec.remove_range(range),
            Bitmap(ref mut bits) => bits.remove_range(range),
        }
    }

    pub fn contains(&self, index: u16) -> bool {
        match *self {
            Array(ref vec) => vec.contains(index),
            Bitmap(ref bits) => bits.contains(index),
        }
    }

    pub fn is_disjoint(&self, other: &Self) -> bool {
        match (self, other) {
            (&Array(ref vec1), &Array(ref vec2)) => vec1.is_disjoint(vec2),
            (&Bitmap(ref bits1), &Bitmap(ref bits2)) => bits1.is_disjoint(bits2),
            (&Array(ref vec), &Bitmap(ref bits)) | (&Bitmap(ref bits), &Array(ref vec)) => {
                vec.iter().all(|&i| !bits.contains(i))
            }
        }
    }

    pub fn is_subset(&self, other: &Self) -> bool {
        match (self, other) {
            (&Array(ref vec1), &Array(ref vec2)) => vec1.is_subset(vec2),
            (&Bitmap(ref bits1), &Bitmap(ref bits2)) => bits1.is_subset(bits2),
            (&Array(ref vec), &Bitmap(ref bits)) => vec.iter().all(|&i| bits.contains(i)),
            (&Bitmap(..), &Array(..)) => false,
        }
    }

    pub fn len(&self) -> u64 {
        match *self {
            Array(ref vec) => vec.len(),
            Bitmap(ref bits) => bits.len(),
        }
    }

    pub fn min(&self) -> Option<u16> {
        match *self {
            Array(ref vec) => vec.min(),
            Bitmap(ref bits) => bits.min(),
        }
    }

    pub fn max(&self) -> Option<u16> {
        match *self {
            Array(ref vec) => vec.max(),
            Bitmap(ref bits) => bits.max(),
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Store::new()
    }
}

impl BitOr<&Store> for &Store {
    type Output = Store;

    fn bitor(self, rhs: &Store) -> Store {
        match (self, rhs) {
            (&Array(ref vec1), &Array(ref vec2)) => Array(BitOr::bitor(vec1, vec2)),
            (&Bitmap(..), &Array(..)) => {
                let mut lhs = self.clone();
                BitOrAssign::bitor_assign(&mut lhs, rhs);
                lhs
            }
            (&Bitmap(..), &Bitmap(..)) => {
                let mut lhs = self.clone();
                BitOrAssign::bitor_assign(&mut lhs, rhs);
                lhs
            }
            (&Array(..), &Bitmap(..)) => {
                let mut rhs = rhs.clone();
                BitOrAssign::bitor_assign(&mut rhs, self);
                rhs
            }
        }
    }
}

impl BitOrAssign<Store> for Store {
    fn bitor_assign(&mut self, mut rhs: Store) {
        match (self, &mut rhs) {
            (&mut Array(ref mut vec1), &mut Array(ref vec2)) => {
                *vec1 = BitOr::bitor(&*vec1, vec2);
            }
            (&mut Bitmap(ref mut bits1), &mut Array(ref vec2)) => {
                BitOrAssign::bitor_assign(bits1, vec2);
            }
            (&mut Bitmap(ref mut bits1), &mut Bitmap(ref bits2)) => {
                BitOrAssign::bitor_assign(bits1, bits2);
            }
            (this @ &mut Array(..), &mut Bitmap(..)) => {
                mem::swap(this, &mut rhs);
                BitOrAssign::bitor_assign(this, rhs);
            }
        }
    }
}

impl BitOrAssign<&Store> for Store {
    fn bitor_assign(&mut self, rhs: &Store) {
        match (self, rhs) {
            (&mut Array(ref mut vec1), &Array(ref vec2)) => {
                let this = mem::take(vec1);
                *vec1 = BitOr::bitor(&this, vec2);
            }
            (&mut Bitmap(ref mut bits1), &Array(ref vec2)) => {
                BitOrAssign::bitor_assign(bits1, vec2);
            }
            (&mut Bitmap(ref mut bits1), &Bitmap(ref bits2)) => {
                BitOrAssign::bitor_assign(bits1, bits2);
            }
            (this @ &mut Array(..), &Bitmap(ref bits2)) => {
                let mut lhs: Store = Bitmap(bits2.clone());
                BitOrAssign::bitor_assign(&mut lhs, &*this);
                *this = lhs;
            }
        }
    }
}

impl BitAnd<&Store> for &Store {
    type Output = Store;

    fn bitand(self, rhs: &Store) -> Store {
        match (self, rhs) {
            (&Array(ref vec1), &Array(ref vec2)) => Array(BitAnd::bitand(vec1, vec2)),
            (&Bitmap(..), &Array(..)) => {
                let mut rhs = rhs.clone();
                BitAndAssign::bitand_assign(&mut rhs, self);
                rhs
            }
            _ => {
                let mut lhs = self.clone();
                BitAndAssign::bitand_assign(&mut lhs, rhs);
                lhs
            }
        }
    }
}

impl BitAndAssign<Store> for Store {
    #[allow(clippy::suspicious_op_assign_impl)]
    fn bitand_assign(&mut self, mut rhs: Store) {
        match (self, &mut rhs) {
            (&mut Array(ref mut vec1), &mut Array(ref mut vec2)) => {
                if vec2.len() < vec1.len() {
                    mem::swap(vec1, vec2);
                }
                BitAndAssign::bitand_assign(vec1, &*vec2);
            }
            (&mut Bitmap(ref mut bits1), &mut Bitmap(ref bits2)) => {
                BitAndAssign::bitand_assign(bits1, bits2);
            }
            (&mut Array(ref mut vec1), &mut Bitmap(ref bits2)) => {
                BitAndAssign::bitand_assign(vec1, bits2);
            }
            (this @ &mut Bitmap(..), &mut Array(..)) => {
                mem::swap(this, &mut rhs);
                BitAndAssign::bitand_assign(this, rhs);
            }
        }
    }
}

impl BitAndAssign<&Store> for Store {
    #[allow(clippy::suspicious_op_assign_impl)]
    fn bitand_assign(&mut self, rhs: &Store) {
        match (self, rhs) {
            (&mut Array(ref mut vec1), &Array(ref vec2)) => {
                let (mut lhs, rhs) = if vec2.len() < vec1.len() {
                    (vec2.clone(), &*vec1)
                } else {
                    (mem::take(vec1), vec2)
                };

                BitAndAssign::bitand_assign(&mut lhs, rhs);
                *vec1 = lhs;
            }
            (&mut Bitmap(ref mut bits1), &Bitmap(ref bits2)) => {
                BitAndAssign::bitand_assign(bits1, bits2);
            }
            (&mut Array(ref mut vec1), &Bitmap(ref bits2)) => {
                BitAndAssign::bitand_assign(vec1, bits2);
            }
            (this @ &mut Bitmap(..), &Array(..)) => {
                let mut new = rhs.clone();
                BitAndAssign::bitand_assign(&mut new, &*this);
                *this = new;
            }
        }
    }
}

impl Sub<&Store> for &Store {
    type Output = Store;

    fn sub(self, rhs: &Store) -> Store {
        match (self, rhs) {
            (&Array(ref vec1), &Array(ref vec2)) => Array(Sub::sub(vec1, vec2)),
            _ => {
                let mut lhs = self.clone();
                SubAssign::sub_assign(&mut lhs, rhs);
                lhs
            }
        }
    }
}

impl SubAssign<&Store> for Store {
    fn sub_assign(&mut self, rhs: &Store) {
        match (self, rhs) {
            (&mut Array(ref mut vec1), &Array(ref vec2)) => {
                SubAssign::sub_assign(vec1, vec2);
            }
            (&mut Bitmap(ref mut bits1), &Array(ref vec2)) => {
                SubAssign::sub_assign(bits1, vec2);
            }
            (&mut Bitmap(ref mut bits1), &Bitmap(ref bits2)) => {
                SubAssign::sub_assign(bits1, bits2);
            }
            (&mut Array(ref mut vec1), &Bitmap(ref bits2)) => {
                SubAssign::sub_assign(vec1, bits2);
            }
        }
    }
}

impl BitXor<&Store> for &Store {
    type Output = Store;

    fn bitxor(self, rhs: &Store) -> Store {
        match (self, rhs) {
            (&Array(ref vec1), &Array(ref vec2)) => Array(BitXor::bitxor(vec1, vec2)),
            (&Array(..), &Bitmap(..)) => {
                let mut lhs = rhs.clone();
                BitXorAssign::bitxor_assign(&mut lhs, self);
                lhs
            }
            _ => {
                let mut lhs = self.clone();
                BitXorAssign::bitxor_assign(&mut lhs, rhs);
                lhs
            }
        }
    }
}

impl BitXorAssign<Store> for Store {
    fn bitxor_assign(&mut self, mut rhs: Store) {
        // TODO improve this function
        match (self, &mut rhs) {
            (&mut Array(ref mut vec1), &mut Array(ref mut vec2)) => {
                BitXorAssign::bitxor_assign(vec1, vec2);
            }
            (&mut Bitmap(ref mut bits1), &mut Array(ref mut vec2)) => {
                BitXorAssign::bitxor_assign(bits1, &*vec2);
            }
            (&mut Bitmap(ref mut bits1), &mut Bitmap(ref bits2)) => {
                BitXorAssign::bitxor_assign(bits1, bits2);
            }
            (this @ &mut Array(..), &mut Bitmap(..)) => {
                mem::swap(this, &mut rhs);
                BitXorAssign::bitxor_assign(this, rhs);
            }
        }
    }
}

impl BitXorAssign<&Store> for Store {
    fn bitxor_assign(&mut self, rhs: &Store) {
        match (self, rhs) {
            (&mut Array(ref mut vec1), &Array(ref vec2)) => {
                BitXorAssign::bitxor_assign(vec1, vec2);
            }
            (&mut Bitmap(ref mut bits1), &Array(ref vec2)) => {
                BitXorAssign::bitxor_assign(bits1, vec2);
            }
            (&mut Bitmap(ref mut bits1), &Bitmap(ref bits2)) => {
                BitXorAssign::bitxor_assign(bits1, bits2);
            }
            (this @ &mut Array(..), &Bitmap(..)) => {
                let mut new = rhs.clone();
                BitXorAssign::bitxor_assign(&mut new, &*this);
                *this = new;
            }
        }
    }
}

impl<'a> IntoIterator for &'a Store {
    type Item = u16;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        match *self {
            Array(ref vec) => Iter::Array(vec.iter()),
            Bitmap(ref bits) => Iter::BitmapBorrowed(bits.iter()),
        }
    }
}

impl IntoIterator for Store {
    type Item = u16;
    type IntoIter = Iter<'static>;
    fn into_iter(self) -> Iter<'static> {
        match self {
            Array(vec) => Iter::Vec(vec.into_iter()),
            Bitmap(bits) => Iter::BitmapOwned(bits.into_iter()),
        }
    }
}

impl PartialEq for Store {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (&Array(ref vec1), &Array(ref vec2)) => vec1 == vec2,
            (&Bitmap(ref bits1), &Bitmap(ref bits2)) => {
                bits1.len() == bits2.len()
                    && bits1.iter().zip(bits2.iter()).all(|(i1, i2)| i1 == i2)
            }
            _ => false,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        match *self {
            Iter::Array(ref mut inner) => inner.next().cloned(),
            Iter::Vec(ref mut inner) => inner.next(),
            Iter::BitmapBorrowed(ref mut inner) => inner.next(),
            Iter::BitmapOwned(ref mut inner) => inner.next(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Iter::Array(inner) => inner.size_hint(),
            Iter::Vec(inner) => inner.size_hint(),
            Iter::BitmapBorrowed(inner) => inner.size_hint(),
            Iter::BitmapOwned(inner) => inner.size_hint(),
        }
    }
}

impl ExactSizeIterator for Iter<'_> {
    fn len(&self) -> usize {
        match self {
            Iter::Array(inner) => inner.len(),
            Iter::Vec(inner) => inner.len(),
            Iter::BitmapBorrowed(inner) => inner.len(),
            Iter::BitmapOwned(inner) => inner.len(),
        }
    }
}