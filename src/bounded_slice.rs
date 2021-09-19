use std::ops::{Deref, DerefMut};

pub struct BoundedSlice<T: Sized, const MAX_SIZE: usize>([T]);

impl<T: Sized, const MAX_SIZE: usize> Deref for BoundedSlice<T, MAX_SIZE> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Sized, const MAX_SIZE: usize> DerefMut for BoundedSlice<T, MAX_SIZE> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub trait BoundedSliceSource<T: Sized, const SIZE: usize> {
    fn get_prefix(&self, len: usize) -> &BoundedSlice<T, SIZE>;
    fn get_prefix_mut(&mut self, len: usize) -> &mut BoundedSlice<T, SIZE>;
}

impl<T: Sized, const SIZE: usize> BoundedSliceSource<T, SIZE> for [T; SIZE] {
    fn get_prefix(&self, len: usize) -> &BoundedSlice<T, SIZE> {
        std::debug_assert!(len <= SIZE);
        unsafe { std::mem::transmute(&self[..len]) }
    }

    fn get_prefix_mut(&mut self, len: usize) -> &mut BoundedSlice<T, SIZE> {
        std::debug_assert!(len <= SIZE);
        unsafe { std::mem::transmute(&mut self[..len]) }
    }
}
