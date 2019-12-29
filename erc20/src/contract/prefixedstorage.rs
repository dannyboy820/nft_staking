use cosmwasm::traits::{ReadonlyStorage, Storage};

// prepend length of the prefix
fn calculate_prefix_impl(prefix: &[u8]) -> Vec<u8> {
    if prefix.len() > 0xFFFF {
        panic!("only supports prefixes up to length 0xFFFF")
    }
    let mut out = Vec::with_capacity(prefix.len() + 2);
    let length_bytes = (prefix.len() as u64).to_be_bytes();
    out.extend_from_slice(&length_bytes[6..8]);
    out.extend_from_slice(prefix);
    out
}

pub struct ReadonlyPrefixedStorage<'a, T: ReadonlyStorage> {
    store: &'a T,
    prefix_impl: Vec<u8>,
}

pub struct PrefixedStorage<'a, T: Storage> {
    store: &'a mut T,
    prefix_impl: Vec<u8>,
}

impl<'a, T> ReadonlyPrefixedStorage<'a, T>
where
    T: ReadonlyStorage,
{
    pub fn new(store: &'a T, prefix: &[u8]) -> Self {
        ReadonlyPrefixedStorage {
            store,
            prefix_impl: calculate_prefix_impl(prefix),
        }
    }
}

impl<'a, T> PrefixedStorage<'a, T>
where
    T: Storage,
{
    pub fn new(store: &'a mut T, prefix: &[u8]) -> Self {
        PrefixedStorage {
            store,
            prefix_impl: calculate_prefix_impl(prefix),
        }
    }
}

impl<'a, T> ReadonlyStorage for ReadonlyPrefixedStorage<'a, T>
where
    T: ReadonlyStorage,
{
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let full_key = [&self.prefix_impl, key].concat();
        self.store.get(&full_key)
    }
}

impl<'a, T> ReadonlyStorage for PrefixedStorage<'a, T>
where
    T: Storage,
{
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let full_key = [&self.prefix_impl, key].concat();
        self.store.get(&full_key)
    }
}

impl<'a, T> Storage for PrefixedStorage<'a, T>
where
    T: Storage,
{
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
    fn calculate_prefix_impl_works() {
        assert_eq!(calculate_prefix_impl(b""), b"\x00\x00");
        assert_eq!(calculate_prefix_impl(b"a"), b"\x00\x01a");
        assert_eq!(calculate_prefix_impl(b"ab"), b"\x00\x02ab");
        assert_eq!(calculate_prefix_impl(b"abc"), b"\x00\x03abc");
    }

    #[test]
    fn calculate_prefix_impl_works_for_long_prefix() {
        let limit = 0xFFFF;
        let long_prefix = vec![0; limit];
        calculate_prefix_impl(&long_prefix);
    }

    #[test]
    #[should_panic(expected = "only supports prefixes up to length 0xFFFF")]
    fn calculate_prefix_impl_panics_for_too_long_prefix() {
        let limit = 0xFFFF;
        let long_prefix = vec![0; limit + 1];
        calculate_prefix_impl(&long_prefix);
    }

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
        let val = store.get(b"\x00\x03foobar");
        assert_eq!(val, Some(b"some data".to_vec()));
    }
}
