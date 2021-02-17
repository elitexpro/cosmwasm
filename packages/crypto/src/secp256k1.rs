use digest::Digest; // trait
use k256::{
    ecdsa::signature::{DigestVerifier, Signature as _}, // traits
    ecdsa::{Signature, VerifyingKey},                   // type aliases
};

use crate::errors::{CryptoError, CryptoResult};
use crate::identity_digest::Identity256;

/// Max length of a message hash for secp256k1 verification in bytes.
/// This is typically a 32 byte output of e.g. SHA-256 or Keccak256. In theory shorter values
/// are possible but currently not supported by the implementation. Let us know when you need them.
pub const MESSAGE_HASH_MAX_LEN: usize = 32;

/// ECDSA (secp256k1) parameters
/// Length of a serialized signature
pub const ECDSA_SIGNATURE_LEN: usize = 64;

/// Compressed public key prefix (variant 1)
const ECDSA_COMPRESSED_PUBKEY_PREFIX_1: u8 = 0x02;
/// Compressed public key prefix (variant 2)
const ECDSA_COMPRESSED_PUBKEY_PREFIX_2: u8 = 0x03;
/// Length of a serialized compressed public key
const ECDSA_COMPRESSED_PUBKEY_LEN: usize = 33;
/// Uncompressed public key prefix
const ECDSA_UNCOMPRESSED_PUBKEY_PREFIX: u8 = 0x04;
/// Length of a serialized uncompressed public key
const ECDSA_UNCOMPRESSED_PUBKEY_LEN: usize = 65;
/// Max length of a serialized public key
pub const ECDSA_PUBKEY_MAX_LEN: usize = ECDSA_UNCOMPRESSED_PUBKEY_LEN;

/// ECDSA secp256k1 implementation.
///
/// This function verifies message hashes (typically, hashed unsing SHA-256) against a signature,
/// with the public key of the signer, using the secp256k1 elliptic curve digital signature
/// parametrization / algorithm.
///
/// The signature and public key are in "Cosmos" format:
/// - signature:  Serialized "compact" signature (64 bytes).
/// - public key: [Serialized according to SEC 2](https://www.oreilly.com/library/view/programming-bitcoin/9781492031482/ch04.html)
/// (33 or 65 bytes).
pub fn secp256k1_verify(
    message_hash: &[u8],
    signature: &[u8],
    public_key: &[u8],
) -> CryptoResult<bool> {
    if message_hash.len() != MESSAGE_HASH_MAX_LEN {
        return Err(CryptoError::hash_err(format!(
            "wrong length: {}",
            message_hash.len()
        )));
    }
    if signature.len() != ECDSA_SIGNATURE_LEN {
        return Err(CryptoError::sig_err(format!(
            "wrong / unsupported length: {}",
            signature.len()
        )));
    }
    let pubkey_len = public_key.len();
    if pubkey_len == 0 {
        return Err(CryptoError::pubkey_err("empty"));
    }
    let pubkey_fmt = public_key[0];
    if !(pubkey_len == ECDSA_UNCOMPRESSED_PUBKEY_LEN
        && pubkey_fmt == ECDSA_UNCOMPRESSED_PUBKEY_PREFIX
        || pubkey_len == ECDSA_COMPRESSED_PUBKEY_LEN
            && (pubkey_fmt == ECDSA_COMPRESSED_PUBKEY_PREFIX_1
                || pubkey_fmt == ECDSA_COMPRESSED_PUBKEY_PREFIX_2))
    {
        return Err(CryptoError::pubkey_err(format!(
            "wrong / unsupported length/format: {}/{}",
            pubkey_len, pubkey_fmt,
        )));
    }

    // Already hashed, just build Digest container
    let message_digest = Identity256::new().chain(message_hash);

    let mut signature =
        Signature::from_bytes(signature).map_err(|e| CryptoError::generic_err(e.to_string()))?;
    // Non low-S signatures require normalization
    signature
        .normalize_s()
        .map_err(|e| CryptoError::generic_err(e.to_string()))?;

    let public_key = VerifyingKey::from_sec1_bytes(public_key)
        .map_err(|e| CryptoError::generic_err(e.to_string()))?;

    match public_key.verify_digest(message_digest, &signature) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use elliptic_curve::sec1::ToEncodedPoint;
    use rand_core::OsRng;

    use k256::{
        ecdsa::signature::DigestSigner, // trait
        ecdsa::SigningKey,              // type alias
    };
    use sha2::Sha256;

    // For generic signature verification
    const MSG: &str = "Hello World!";

    // Cosmos secp256k1 signature verification
    // tendermint/PubKeySecp256k1 pubkey
    const COSMOS_SECP256K1_PUBKEY_BASE64: &str = "A08EGB7ro1ORuFhjOnZcSgwYlpe0DSFjVNUIkNNQxwKQ";

    const COSMOS_SECP256K1_MSG_HEX1: &str = "0a93010a90010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e6412700a2d636f736d6f7331706b707472653766646b6c366766727a6c65736a6a766878686c63337234676d6d6b38727336122d636f736d6f7331717970717870713971637273737a673270767871367273307a716733797963356c7a763778751a100a0575636f736d12073132333435363712650a4e0a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a21034f04181eeba35391b858633a765c4a0c189697b40d216354d50890d350c7029012040a02080112130a0d0a0575636f736d12043230303010c09a0c1a0c73696d642d74657374696e672001";
    const COSMOS_SECP256K1_MSG_HEX2: &str = "0a93010a90010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e6412700a2d636f736d6f7331706b707472653766646b6c366766727a6c65736a6a766878686c63337234676d6d6b38727336122d636f736d6f7331717970717870713971637273737a673270767871367273307a716733797963356c7a763778751a100a0575636f736d12073132333435363712670a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a21034f04181eeba35391b858633a765c4a0c189697b40d216354d50890d350c7029012040a020801180112130a0d0a0575636f736d12043230303010c09a0c1a0c73696d642d74657374696e672001";
    const COSMOS_SECP256K1_MSG_HEX3: &str = "0a93010a90010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e6412700a2d636f736d6f7331706b707472653766646b6c366766727a6c65736a6a766878686c63337234676d6d6b38727336122d636f736d6f7331717970717870713971637273737a673270767871367273307a716733797963356c7a763778751a100a0575636f736d12073132333435363712670a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a21034f04181eeba35391b858633a765c4a0c189697b40d216354d50890d350c7029012040a020801180212130a0d0a0575636f736d12043230303010c09a0c1a0c73696d642d74657374696e672001";

    const COSMOS_SECP256K1_SIGNATURE_HEX1: &str = "c9dd20e07464d3a688ff4b710b1fbc027e495e797cfa0b4804da2ed117959227772de059808f765aa29b8f92edf30f4c2c5a438e30d3fe6897daa7141e3ce6f9";
    const COSMOS_SECP256K1_SIGNATURE_HEX2: &str = "525adc7e61565a509c60497b798c549fbf217bb5cd31b24cc9b419d098cc95330c99ecc4bc72448f85c365a4e3f91299a3d40412fb3751bab82f1940a83a0a4c";
    const COSMOS_SECP256K1_SIGNATURE_HEX3: &str = "f3f2ca73806f2abbf6e0fe85f9b8af66f0e9f7f79051fdb8abe5bb8633b17da132e82d577b9d5f7a6dae57a144efc9ccc6eef15167b44b3b22a57240109762af";

    // Test data originally from https://github.com/cosmos/cosmjs/blob/v0.24.0-alpha.22/packages/crypto/src/secp256k1.spec.ts#L195-L394
    const COSMOS_SECP256K1_TESTS_JSON: &str = "./testdata/secp256k1_tests.json";

    #[test]
    fn test_secp256k1_verify() {
        // Explicit / external hashing
        let message_digest = Sha256::new().chain(MSG);
        let message_hash = message_digest.clone().finalize();

        // Signing
        let secret_key = SigningKey::random(&mut OsRng); // Serialize with `::to_bytes()`

        // Note: the signature type must be annotated or otherwise inferrable as
        // `Signer` has many impls of the `Signer` trait (for both regular and
        // recoverable signature types).
        let signature: Signature = secret_key.sign_digest(message_digest);

        let public_key = VerifyingKey::from(&secret_key); // Serialize with `::to_encoded_point()`

        // Verification (uncompressed public key)
        assert!(secp256k1_verify(
            &message_hash,
            signature.as_bytes(),
            public_key.to_encoded_point(false).as_bytes()
        )
        .unwrap());

        // Verification (compressed public key)
        assert!(secp256k1_verify(
            &message_hash,
            signature.as_bytes(),
            public_key.to_encoded_point(true).as_bytes()
        )
        .unwrap());

        // Wrong message fails
        let bad_message_hash = Sha256::new().chain([MSG, "\0"].concat()).finalize();
        assert!(!secp256k1_verify(
            &bad_message_hash,
            signature.as_bytes(),
            public_key.to_encoded_point(false).as_bytes()
        )
        .unwrap());

        // Other pubkey fails
        let other_secret_key = SigningKey::random(&mut OsRng);
        let other_public_key = VerifyingKey::from(&other_secret_key);
        assert!(!secp256k1_verify(
            &message_hash,
            signature.as_bytes(),
            other_public_key.to_encoded_point(false).as_bytes()
        )
        .unwrap());
    }

    #[test]
    fn test_cosmos_secp256k1_verify() {
        let public_key = base64::decode(COSMOS_SECP256K1_PUBKEY_BASE64).unwrap();

        for ((i, msg), sig) in (1..)
            .zip(&[
                COSMOS_SECP256K1_MSG_HEX1,
                COSMOS_SECP256K1_MSG_HEX2,
                COSMOS_SECP256K1_MSG_HEX3,
            ])
            .zip(&[
                COSMOS_SECP256K1_SIGNATURE_HEX1,
                COSMOS_SECP256K1_SIGNATURE_HEX2,
                COSMOS_SECP256K1_SIGNATURE_HEX3,
            ])
        {
            let message = hex::decode(msg).unwrap();
            let signature = hex::decode(sig).unwrap();

            // Explicit hash
            let message_hash = Sha256::new().chain(&message).finalize();

            // secp256k1_verify works
            assert!(
                secp256k1_verify(&message_hash, &signature, &public_key).unwrap(),
                format!("secp256k1_verify() failed (test case {})", i)
            );
        }
    }

    #[test]
    fn test_cosmos_extra_secp256k1_verify() {
        use std::fs::File;
        use std::io::BufReader;

        use serde::Deserialize;

        #[derive(Deserialize, Debug)]
        struct Encoded {
            message: String,
            message_hash: String,
            signature: String,
            #[serde(rename = "pubkey")]
            public_key: String,
        }

        // Open the file in read-only mode with buffer.
        let file = File::open(COSMOS_SECP256K1_TESTS_JSON).unwrap();
        let reader = BufReader::new(file);

        let codes: Vec<Encoded> = serde_json::from_reader(reader).unwrap();

        for (i, encoded) in (1..).zip(codes) {
            let message = hex::decode(&encoded.message).unwrap();

            let hash = hex::decode(&encoded.message_hash).unwrap();
            let message_hash = Sha256::new().chain(&message).finalize();
            assert_eq!(hash.as_slice(), message_hash.as_slice());

            let signature = hex::decode(&encoded.signature).unwrap();

            let public_key = hex::decode(&encoded.public_key).unwrap();

            // secp256k1_verify() works
            assert!(
                secp256k1_verify(&message_hash, &signature, &public_key).unwrap(),
                format!("verify() failed (test case {})", i)
            );
        }
    }
}
