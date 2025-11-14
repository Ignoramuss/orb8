use crate::Result;
use std::collections::HashMap;
use std::marker::PhantomData;

pub struct EbpfMap<K, V> {
    name: String,
    _key: PhantomData<K>,
    _value: PhantomData<V>,
}

impl<K, V> EbpfMap<K, V> {
    pub fn new(name: String) -> Self {
        Self {
            name,
            _key: PhantomData,
            _value: PhantomData,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

pub struct PodMetadataMap {
    inner: HashMap<u32, PodMetadata>,
}

impl PodMetadataMap {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    pub fn insert(&mut self, pid: u32, metadata: PodMetadata) -> Result<()> {
        self.inner.insert(pid, metadata);
        Ok(())
    }

    pub fn get(&self, pid: &u32) -> Option<&PodMetadata> {
        self.inner.get(pid)
    }

    pub fn remove(&mut self, pid: &u32) -> Option<PodMetadata> {
        self.inner.remove(pid)
    }
}

impl Default for PodMetadataMap {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct PodMetadata {
    pub name: String,
    pub namespace: String,
    pub uid: String,
}
