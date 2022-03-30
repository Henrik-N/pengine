use std::ops::{Deref, DerefMut, Index, IndexMut};

// Typed handle to an index in an array of T.
#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialOrd, PartialEq, Eq, Ord)]
pub struct Handle<T> {
    pub id: u32,
    _marker: std::marker::PhantomData<T>
}

impl<T> From<usize> for Handle<T>
{
    fn from(handle: usize) -> Self {
        Self {
            id: handle as _,
            _marker: std::marker::PhantomData,
        }
    }
}
impl<T> From<Handle<T>> for usize
{
    fn from(handle: Handle<T>) -> Self {
        handle.id as _
    }
}

/// Calculates the number of bytes that need to be added to 'size' to reach 'alignment'.
pub fn calculate_padding(size: usize, alignment: usize) -> usize {
    (alignment - size % alignment) % alignment
}

/// Wrapper of Vec<T> that is indexed by Handle<T>s.
#[repr(C)]
#[derive(Debug, Clone, Default, PartialOrd, PartialEq, Eq, Ord)]
pub struct HandleMap<T> {
    pub inner: Vec<T>,
}
impl<T> HandleMap<T> {
    pub fn new() -> Self {
        Self {
            inner: Vec::new()
        }
    }

    pub fn push(&mut self, value: T) -> Handle<T> {
        self.inner.push(value);
        Handle::from(self.inner.len() - 1)
    }
}

impl<T> Deref for HandleMap<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<T> DerefMut for HandleMap<T>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T> Index<Handle<T>> for HandleMap<T>
{
    type Output = T;

    fn index(&self, handle: Handle<T>) -> &Self::Output {
        &self.inner[handle.id as usize]
    }
}
impl<T> IndexMut<Handle<T>> for HandleMap<T> {
    fn index_mut(&mut self, handle: Handle<T>) -> &mut Self::Output {
        &mut self.inner[handle.id as usize]
    }
}
