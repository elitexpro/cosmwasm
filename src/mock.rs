use std::collections::HashMap;

pub use crate::storage::Storage;

#[derive(Clone)]
pub struct MockStorage {
    data: HashMap<Vec<u8>, Vec<u8>>,
}

impl MockStorage {
    pub fn new() -> Self {
        MockStorage {
            data: HashMap::new(),
        }
    }
}

impl Default for MockStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for MockStorage {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        self.data.insert(key.to_vec(), value.to_vec());
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn get_and_set() {
        let mut store = MockStorage::new();
        assert_eq!(None, store.get(b"foo"));
        store.set(b"foo", b"bar");
        assert_eq!(Some(b"bar".to_vec()), store.get(b"foo"));
        assert_eq!(None, store.get(b"food"));
    }
}
