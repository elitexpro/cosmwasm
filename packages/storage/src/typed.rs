use serde::{de::DeserializeOwned, ser::Serialize};
use std::marker::PhantomData;

use cosmwasm_std::{to_vec, ReadonlyStorage, StdResult, Storage};
#[cfg(feature = "iterator")]
use cosmwasm_std::{Order, KV};

#[cfg(feature = "iterator")]
use crate::type_helpers::deserialize_kv;
use crate::type_helpers::{may_deserialize, must_deserialize};

/// An alias of TypedStorage::new for less verbose usage
pub fn typed<S, T>(storage: &mut S) -> TypedStorage<S, T>
where
    S: Storage,
    T: Serialize + DeserializeOwned,
{
    TypedStorage::new(storage)
}

/// An alias of ReadonlyTypedStorage::new for less verbose usage
pub fn typed_read<S, T>(storage: &S) -> ReadonlyTypedStorage<S, T>
where
    S: ReadonlyStorage,
    T: Serialize + DeserializeOwned,
{
    ReadonlyTypedStorage::new(storage)
}

pub struct TypedStorage<'a, S, T>
where
    S: Storage,
    T: Serialize + DeserializeOwned,
{
    storage: &'a mut S,
    // see https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-type-parameters for why this is needed
    data: PhantomData<T>,
}

impl<'a, S, T> TypedStorage<'a, S, T>
where
    S: Storage,
    T: Serialize + DeserializeOwned,
{
    pub fn new(storage: &'a mut S) -> Self {
        TypedStorage {
            storage,
            data: PhantomData,
        }
    }

    /// save will serialize the model and store, returns an error on serialization issues
    pub fn save(&mut self, key: &[u8], data: &T) -> StdResult<()> {
        self.storage.set(key, &to_vec(data)?);
        Ok(())
    }

    /// load will return an error if no data is set at the given key, or on parse error
    pub fn load(&self, key: &[u8]) -> StdResult<T> {
        let value = self.storage.get(key);
        must_deserialize(&value)
    }

    /// may_load will parse the data stored at the key if present, returns Ok(None) if no data there.
    /// returns an error on issues parsing
    pub fn may_load(&self, key: &[u8]) -> StdResult<Option<T>> {
        let value = self.storage.get(key);
        may_deserialize(&value)
    }

    #[cfg(feature = "iterator")]
    pub fn range<'b>(
        &'b self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<KV<T>>> + 'b> {
        let mapped = self
            .storage
            .range(start, end, order)
            .map(deserialize_kv::<T>);
        Box::new(mapped)
    }

    /// update will load the data, perform the specified action, and store the result
    /// in the database. This is shorthand for some common sequences, which may be useful
    ///
    /// This is the least stable of the APIs, and definitely needs some usage
    pub fn update<A>(&mut self, key: &[u8], action: A) -> StdResult<T>
    where
        A: FnOnce(Option<T>) -> StdResult<T>,
    {
        let input = self.may_load(key)?;
        let output = action(input)?;
        self.save(key, &output)?;
        Ok(output)
    }
}

pub struct ReadonlyTypedStorage<'a, S, T>
where
    S: ReadonlyStorage,
    T: Serialize + DeserializeOwned,
{
    storage: &'a S,
    // see https://doc.rust-lang.org/std/marker/struct.PhantomData.html#unused-type-parameters for why this is needed
    data: PhantomData<T>,
}

impl<'a, S, T> ReadonlyTypedStorage<'a, S, T>
where
    S: ReadonlyStorage,
    T: Serialize + DeserializeOwned,
{
    pub fn new(storage: &'a S) -> Self {
        ReadonlyTypedStorage {
            storage,
            data: PhantomData,
        }
    }

    /// load will return an error if no data is set at the given key, or on parse error
    pub fn load(&self, key: &[u8]) -> StdResult<T> {
        let value = self.storage.get(key);
        must_deserialize(&value)
    }

    /// may_load will parse the data stored at the key if present, returns Ok(None) if no data there.
    /// returns an error on issues parsing
    pub fn may_load(&self, key: &[u8]) -> StdResult<Option<T>> {
        let value = self.storage.get(key);
        may_deserialize(&value)
    }

    #[cfg(feature = "iterator")]
    pub fn range<'b>(
        &'b self,
        start: Option<&[u8]>,
        end: Option<&[u8]>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<KV<T>>> + 'b> {
        let mapped = self
            .storage
            .range(start, end, order)
            .map(deserialize_kv::<T>);
        Box::new(mapped)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cosmwasm_std::testing::MockStorage;
    use cosmwasm_std::StdError;
    use serde::{Deserialize, Serialize};

    use crate::prefixed;

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    struct Data {
        pub name: String,
        pub age: i32,
    }

    #[test]
    fn store_and_load() {
        let mut store = MockStorage::new();
        let mut bucket = TypedStorage::<_, Data>::new(&mut store);

        // check empty data handling
        assert!(bucket.load(b"maria").is_err());
        assert_eq!(bucket.may_load(b"maria").unwrap(), None);

        // save data
        let data = Data {
            name: "Maria".to_string(),
            age: 42,
        };
        bucket.save(b"maria", &data).unwrap();

        // load it properly
        let loaded = bucket.load(b"maria").unwrap();
        assert_eq!(data, loaded);
    }

    #[test]
    fn store_with_prefix() {
        let mut store = MockStorage::new();
        let mut space = prefixed(&mut store, b"data");
        let mut bucket = typed::<_, Data>(&mut space);

        // save data
        let data = Data {
            name: "Maria".to_string(),
            age: 42,
        };
        bucket.save(b"maria", &data).unwrap();

        // load it properly
        let loaded = bucket.load(b"maria").unwrap();
        assert_eq!(data, loaded);
    }

    #[test]
    fn readonly_works() {
        let mut store = MockStorage::new();
        let mut bucket = typed::<_, Data>(&mut store);

        // save data
        let data = Data {
            name: "Maria".to_string(),
            age: 42,
        };
        bucket.save(b"maria", &data).unwrap();

        let reader = typed_read::<_, Data>(&mut store);

        // check empty data handling
        assert!(reader.load(b"john").is_err());
        assert_eq!(reader.may_load(b"john").unwrap(), None);

        // load it properly
        let loaded = reader.load(b"maria").unwrap();
        assert_eq!(data, loaded);
    }

    #[test]
    fn update_success() {
        let mut store = MockStorage::new();
        let mut bucket = typed::<_, Data>(&mut store);

        // initial data
        let init = Data {
            name: "Maria".to_string(),
            age: 42,
        };
        bucket.save(b"maria", &init).unwrap();

        // it's my birthday (fail if no data)
        let birthday = |mayd: Option<Data>| -> StdResult<Data> {
            let mut d = mayd.ok_or(StdError::not_found("Data"))?;
            d.age += 1;
            Ok(d)
        };
        let output = bucket.update(b"maria", &birthday).unwrap();
        let expected = Data {
            name: "Maria".to_string(),
            age: 43,
        };
        assert_eq!(output, expected);

        // load it properly
        let loaded = bucket.load(b"maria").unwrap();
        assert_eq!(loaded, expected);
    }

    #[test]
    fn update_fails_on_error() {
        let mut store = MockStorage::new();
        let mut bucket = typed::<_, Data>(&mut store);

        // initial data
        let init = Data {
            name: "Maria".to_string(),
            age: 42,
        };
        bucket.save(b"maria", &init).unwrap();

        // it's my birthday
        let output = bucket.update(b"maria", |_d| {
            Err(StdError::generic_err("cuz i feel like it"))
        });
        assert!(output.is_err());

        // load it properly
        let loaded = bucket.load(b"maria").unwrap();
        assert_eq!(loaded, init);
    }

    #[test]
    fn update_handles_on_no_data() {
        let mut store = MockStorage::new();
        let mut bucket = typed::<_, Data>(&mut store);

        let init_value = Data {
            name: "Maria".to_string(),
            age: 42,
        };

        // it's my birthday
        let output = bucket
            .update(b"maria", |d| match d {
                Some(_) => Err(StdError::generic_err("Ensure this was empty")),
                None => Ok(init_value.clone()),
            })
            .unwrap();
        assert_eq!(output, init_value);

        // nothing stored
        let loaded = bucket.load(b"maria").unwrap();
        assert_eq!(loaded, init_value);
    }

    #[test]
    #[cfg(feature = "iterator")]
    fn range_over_data() {
        let mut store = MockStorage::new();
        let mut bucket = typed::<_, Data>(&mut store);

        let jose = Data {
            name: "Jose".to_string(),
            age: 42,
        };
        let maria = Data {
            name: "Maria".to_string(),
            age: 27,
        };

        bucket.save(b"maria", &maria).unwrap();
        bucket.save(b"jose", &jose).unwrap();

        let res_data: StdResult<Vec<KV<Data>>> =
            bucket.range(None, None, Order::Ascending).collect();
        let data = res_data.unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0], (b"jose".to_vec(), jose.clone()));
        assert_eq!(data[1], (b"maria".to_vec(), maria.clone()));

        // also works for readonly
        let read_bucket = typed_read::<_, Data>(&store);
        let res_data: StdResult<Vec<KV<Data>>> =
            read_bucket.range(None, None, Order::Ascending).collect();
        let data = res_data.unwrap();
        assert_eq!(data.len(), 2);
        assert_eq!(data[0], (b"jose".to_vec(), jose));
        assert_eq!(data[1], (b"maria".to_vec(), maria));
    }
}
