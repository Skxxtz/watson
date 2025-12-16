pub mod calendar;
pub mod notification;
pub mod protocol;
pub mod tokio;

use std::collections::VecDeque;

pub struct RingBuffer<T> {
    inner: VecDeque<T>,
    capacity: usize,
}
impl<T> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: VecDeque::with_capacity(capacity),
            capacity,
        }
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    // Inserts
    pub fn push(&mut self, value: T) {
        if self.inner.len() == self.capacity {
            self.inner.pop_front();
        }
        self.inner.push_back(value);
    }

    // Getters
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index)
    }
    pub fn last(&self) -> Option<&T> {
        self.inner.back()
    }

    // Iterators
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.inner.iter()
    }
    pub fn into_iter(self) -> impl IntoIterator<Item = T> {
        self.inner.into_iter()
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//     }
// }
//
//
