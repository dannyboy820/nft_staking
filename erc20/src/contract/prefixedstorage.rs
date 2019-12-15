use cosmwasm::storage::Storage;

pub struct PrefixedStorage<'a, T: Storage> {
    store: &'a mut T,
    prefix_impl: Vec<u8>,
}

impl<'a, T> PrefixedStorage<'a, T>
where
    T: Storage,
{
    pub fn new(store: &'a mut T, prefix: &[u8]) -> Self {
        PrefixedStorage {
            store,
            prefix_impl: [&[prefix.len() as u8], prefix].concat(),
        }
    }
}

impl<'a, T> Storage for PrefixedStorage<'a, T>
where
    T: Storage,
{
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let full_key = [&self.prefix_impl, key].concat();
        self.store.get(&full_key)
    }

    fn set(&mut self, key: &[u8], value: &[u8]) {
        let full_key = [&self.prefix_impl, key].concat();
        self.store.set(&full_key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm::mock::MockStorage;

    #[test]
    fn works() {
        let mut store = MockStorage::new();

        // use a block to release prefix at end, and release it's "write lock" on store
        {
            let mut prefixed = PrefixedStorage::new(&mut store, b"foo");
            prefixed.set(b"bar", b"some data");
            let val = prefixed.get(b"bar");
            assert_eq!(val, Some(b"some data".to_vec()));
        }

        // now check the underlying storage
        let val = store.get(b"bar");
        assert_eq!(val, None);
        let val = store.get(b"\x03foobar");
        assert_eq!(val, Some(b"some data".to_vec()));
    }
}
