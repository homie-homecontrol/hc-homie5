use std::collections::HashSet;
use std::hash::Hash;

/// An iterator adapter that filters out duplicates based on a key extraction function.
pub struct UniqueBy<I, F, K> {
    iter: I,
    key_fn: F,
    seen: HashSet<K>,
}

impl<I, F, K> Iterator for UniqueBy<I, F, K>
where
    I: Iterator,
    F: Fn(&I::Item) -> K,
    K: Eq + Hash,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        for item in &mut self.iter {
            let key = (self.key_fn)(&item);
            if self.seen.insert(key) {
                return Some(item);
            }
        }
        None
    }
}

/// Extension trait to add `unique_by` to all iterators.
pub trait UniqueByExt: Iterator {
    fn unique_by<F, K>(self, key_fn: F) -> UniqueBy<Self, F, K>
    where
        Self: Sized,
        F: Fn(&Self::Item) -> K,
        K: Eq + Hash,
    {
        UniqueBy {
            iter: self,
            key_fn,
            seen: HashSet::new(),
        }
    }
}

// Implement the trait for all iterators
impl<I: Iterator> UniqueByExt for I {}
