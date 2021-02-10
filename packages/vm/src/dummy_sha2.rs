//! Dummy Sha-256 Digest impl.
//! This digest stores/accepts an already hashed value of the proper length.
//! Just to comply with the Digest contract.
//!
//! Adapted from `sha2` [sha256.rs](https://github.com/RustCrypto/hashes/blob/master/sha2/src/sha256.rs)
use sha2::digest::consts::U32;
use sha2::digest::generic_array::GenericArray;
use sha2::digest::{FixedOutputDirty, Reset, Update};

use sha2::digest;

/// The SHA-256 identity container
#[derive(Clone)]
pub struct IdentitySha256 {
    array: GenericArray<u8, U32>,
}

impl Default for IdentitySha256 {
    fn default() -> Self {
        IdentitySha256 {
            array: GenericArray::default(),
        }
    }
}

impl Update for IdentitySha256 {
    fn update(&mut self, hash: impl AsRef<[u8]>) {
        assert_eq!(hash.as_ref().len(), 32);
        self.array = *GenericArray::from_slice(hash.as_ref());
    }
}

impl FixedOutputDirty for IdentitySha256 {
    type OutputSize = U32;

    fn finalize_into_dirty(&mut self, out: &mut digest::Output<Self>) {
        *out = self.array;
    }
}

impl Reset for IdentitySha256 {
    fn reset(&mut self) {
        *self = Self::default();
    }
}
