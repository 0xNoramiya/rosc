//! Fixed-size history buffers for the monitor.

/// Keeps the newest `cap` items. Iterates oldest first.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "borsh", derive(borsh::BorshSerialize, borsh::BorshDeserialize))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ring<T> {
    cap: u16,
    head: u16,
    items: Vec<T>,
}

impl<T> Ring<T> {
    pub fn new(cap: u16) -> Self {
        assert!(cap > 0);
        Ring { cap, head: 0, items: Vec::new() }
    }

    pub fn push(&mut self, item: T) {
        if self.items.len() < self.cap as usize {
            self.items.push(item);
        } else {
            self.items[self.head as usize] = item;
            self.head = (self.head + 1) % self.cap;
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.cap as usize
    }

    /// Oldest to newest.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + '_ {
        let (newer, older) = self.items.split_at(self.head as usize);
        older.iter().chain(newer.iter())
    }

    pub fn last(&self) -> Option<&T> {
        self.iter().next_back()
    }

    pub fn last_mut(&mut self) -> Option<&mut T> {
        if self.items.is_empty() {
            return None;
        }
        let n = self.items.len();
        let idx = if n < self.cap as usize { n - 1 } else { (self.head as usize + n - 1) % n };
        self.items.get_mut(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::Ring;

    #[test]
    fn keeps_the_newest_items_in_order() {
        let mut r = Ring::new(4);
        for i in 0..10u32 {
            r.push(i);
            assert_eq!(r.last(), Some(&i));
            *r.last_mut().unwrap() = i;
        }
        assert_eq!(r.iter().copied().collect::<Vec<_>>(), vec![6, 7, 8, 9]);
        assert_eq!(r.len(), 4);
    }
}
