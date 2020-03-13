use crate::Error;
use http::StatusCode;
use std::any::{Any, TypeId};
use std::borrow::{Borrow, Cow};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

pub trait Key: Any {
    fn key(self) -> &'static str;
}

impl Key for &'static str {
    #[inline]
    fn key(self) -> &'static str {
        self
    }
}

pub trait Value: Any + Send + Sync {}

impl<V> Value for V where V: Any + Send + Sync {}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
struct WrapKey(TypeId, &'static str);

/// A context scoped storage.
#[derive(Clone)]
pub struct Storage(HashMap<WrapKey, Arc<dyn Any + Send + Sync>>);

/// A variable.
#[derive(Debug, Clone)]
pub struct Variable<V> {
    key: &'static str,
    value: Arc<V>,
}

impl<V> Deref for Variable<V> {
    type Target = V;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<V> Variable<V> {
    /// Construct a variable from name and value.
    #[inline]
    fn new(key: &'static str, value: Arc<V>) -> Variable<V> {
        Self { key, value }
    }
}

impl<V> Variable<V>
where
    V: AsRef<str>,
{
    /// A wrapper of `str::parse`. Converts `T::FromStr::Err` to `roa_core::Error` automatically.
    #[inline]
    pub fn parse<T>(&self) -> Result<T, Error>
    where
        T: FromStr,
        T::Err: Display,
    {
        self.as_ref().parse().map_err(|err| {
            Error::new(
                StatusCode::BAD_REQUEST,
                format!(
                    "{}\ntype of variable `{}` should be {}",
                    err,
                    self.key,
                    std::any::type_name::<T>()
                ),
                true,
            )
        })
    }
}

impl Storage {
    /// Construct an empty Bucket.
    #[inline]
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Inserts a key-value pair into the storage.
    ///
    /// If the storage did not have this key present, [`None`] is returned.
    ///
    /// If the storage did have this key present, the value is updated, and the old
    /// value is returned.
    #[inline]
    pub fn insert<K: Key, V: Value>(&mut self, key: K, value: V) -> Option<Variable<V>> {
        let key = key.key();
        self.0
            .insert(WrapKey(TypeId::of::<K>(), key), Arc::new(value))
            .and_then(|value| Some(Variable::new(key, value.downcast().ok()?)))
    }

    /// If the storage did not have this key present, [`None`] is returned.
    ///
    /// If the storage did have this key present, the key-value pair will be returned as a `Variable`
    #[inline]
    pub fn get<K: Key, V: Value>(&self, key: K) -> Option<Variable<V>> {
        let key = key.key();
        self.0
            .get(&WrapKey(TypeId::of::<K>(), key))
            .and_then(|value| Some(Variable::new(key, value.clone().downcast().ok()?)))
    }
}

impl Default for Storage {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Storage, Variable};
    use http::StatusCode;
    use std::borrow::Cow;
    use std::sync::Arc;

    #[test]
    fn storage() {
        let mut storage = Storage::default();
        assert!(storage.get::<_, &'static str>("id").is_none());
        assert!(storage.insert("id", "1").is_none());
        let id: i32 = storage
            .get::<_, &'static str>("id")
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(1, id);
        assert_eq!(
            1,
            storage.insert("id", "2").unwrap().parse::<i32>().unwrap()
        );
    }

    #[test]
    fn variable() {
        assert_eq!(
            1,
            Variable::new("id", Arc::new("1")).parse::<i32>().unwrap()
        );
        let result = Variable::new("id", Arc::new("x")).parse::<usize>();
        assert!(result.is_err());
        let status = result.unwrap_err();
        assert_eq!(StatusCode::BAD_REQUEST, status.status_code);
        assert!(status
            .message
            .ends_with("type of variable `id` should be usize"));
    }
}
