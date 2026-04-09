//! Bounded deque for memory-efficient event/data buffering
//!
//! Provides a fixed-capacity deque that automatically evicts oldest items
//! when capacity is reached (FIFO eviction).

use std::collections::VecDeque;

/// A bounded deque with FIFO eviction policy
///
/// When the deque reaches its capacity, the oldest item is automatically
/// removed when a new item is pushed.
#[derive(Clone, Debug)]
pub struct BoundedDeque<T> {
    cap: usize,
    buf: VecDeque<T>,
}

impl<T> BoundedDeque<T> {
    /// Create a new bounded deque with the specified capacity
    ///
    /// # Arguments
    /// * `cap` - Maximum number of items to store. If 0, push operations are no-ops.
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            buf: VecDeque::with_capacity(cap.min(1024)),
        }
    }

    /// Push a new value, evicting the oldest if at capacity
    pub fn push(&mut self, value: T) {
        if self.cap == 0 {
            return;
        }
        if self.buf.len() == self.cap {
            self.buf.pop_front(); // FIFO eviction
        }
        self.buf.push_back(value);
    }

    /// Push multiple values
    pub fn extend(&mut self, values: impl IntoIterator<Item = T>) {
        for value in values {
            self.push(value);
        }
    }

    /// Get an iterator over the items (oldest to newest)
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf.iter()
    }

    /// Get a reverse iterator (newest to oldest)
    pub fn iter_rev(&self) -> impl Iterator<Item = &T> {
        self.buf.iter().rev()
    }

    /// Get the number of items currently stored
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Check if the deque is empty
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Get the capacity
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    /// Get the most recent item
    pub fn last(&self) -> Option<&T> {
        self.buf.back()
    }

    /// Get the oldest item
    pub fn first(&self) -> Option<&T> {
        self.buf.front()
    }

    /// Get item by index (0 = oldest)
    pub fn get(&self, index: usize) -> Option<&T> {
        self.buf.get(index)
    }

    /// Convert to a Vec (clones all items)
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.buf.iter().cloned().collect()
    }
}

impl<T> Default for BoundedDeque<T> {
    fn default() -> Self {
        Self::new(100)
    }
}

impl<T> FromIterator<T> for BoundedDeque<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let items: Vec<T> = iter.into_iter().collect();
        let cap = items.len();
        let mut deque = Self::new(cap);
        for item in items {
            deque.push(item);
        }
        deque
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounded_deque_basic() {
        let mut deque = BoundedDeque::new(3);
        deque.push(1);
        deque.push(2);
        deque.push(3);
        assert_eq!(deque.len(), 3);
        assert_eq!(deque.to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn test_bounded_deque_eviction() {
        let mut deque = BoundedDeque::new(3);
        deque.push(1);
        deque.push(2);
        deque.push(3);
        deque.push(4); // Should evict 1
        assert_eq!(deque.len(), 3);
        assert_eq!(deque.to_vec(), vec![2, 3, 4]);
    }

    #[test]
    fn test_bounded_deque_zero_capacity() {
        let mut deque = BoundedDeque::new(0);
        deque.push(1);
        assert!(deque.is_empty());
    }
}
