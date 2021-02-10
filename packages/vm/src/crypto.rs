#[cfg(test)]
mod tests {
    use elliptic_curve::rand_core::OsRng;
    use k256::{
        ecdsa::signature::{DigestVerifier, Signature as _, Signer, Verifier}, // traits
        ecdsa::{Signature, SigningKey, VerifyingKey},                         // type aliases
    };
    use sha2::{Digest, Sha256};

    // Generic signature verification
    const MSG: &str = "Hello World!";

    // Cosmos signature verification
    // tendermint/PubKeySecp256k1 pubkey
    const COSMOS_PUBKEY_BASE64: &str = "A08EGB7ro1ORuFhjOnZcSgwYlpe0DSFjVNUIkNNQxwKQ";

    const COSMOS_MSG_HEX1: &str = "0a93010a90010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e6412700a2d636f736d6f7331706b707472653766646b6c366766727a6c65736a6a766878686c63337234676d6d6b38727336122d636f736d6f7331717970717870713971637273737a673270767871367273307a716733797963356c7a763778751a100a0575636f736d12073132333435363712650a4e0a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a21034f04181eeba35391b858633a765c4a0c189697b40d216354d50890d350c7029012040a02080112130a0d0a0575636f736d12043230303010c09a0c1a0c73696d642d74657374696e672001";
    const COSMOS_MSG_HEX2: &str = "0a93010a90010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e6412700a2d636f736d6f7331706b707472653766646b6c366766727a6c65736a6a766878686c63337234676d6d6b38727336122d636f736d6f7331717970717870713971637273737a673270767871367273307a716733797963356c7a763778751a100a0575636f736d12073132333435363712670a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a21034f04181eeba35391b858633a765c4a0c189697b40d216354d50890d350c7029012040a020801180112130a0d0a0575636f736d12043230303010c09a0c1a0c73696d642d74657374696e672001";
    const COSMOS_MSG_HEX3: &str = "0a93010a90010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e6412700a2d636f736d6f7331706b707472653766646b6c366766727a6c65736a6a766878686c63337234676d6d6b38727336122d636f736d6f7331717970717870713971637273737a673270767871367273307a716733797963356c7a763778751a100a0575636f736d12073132333435363712670a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a21034f04181eeba35391b858633a765c4a0c189697b40d216354d50890d350c7029012040a020801180212130a0d0a0575636f736d12043230303010c09a0c1a0c73696d642d74657374696e672001";

    const COSMOS_SIGNATURE_HEX1: &str = "c9dd20e07464d3a688ff4b710b1fbc027e495e797cfa0b4804da2ed117959227772de059808f765aa29b8f92edf30f4c2c5a438e30d3fe6897daa7141e3ce6f9";
    const COSMOS_SIGNATURE_HEX2: &str = "525adc7e61565a509c60497b798c549fbf217bb5cd31b24cc9b419d098cc95330c99ecc4bc72448f85c365a4e3f91299a3d40412fb3751bab82f1940a83a0a4c";
    const COSMOS_SIGNATURE_HEX3: &str = "f3f2ca73806f2abbf6e0fe85f9b8af66f0e9f7f79051fdb8abe5bb8633b17da132e82d577b9d5f7a6dae57a144efc9ccc6eef15167b44b3b22a57240109762af";

    // Test data originally from https://github.com/cosmos/cosmjs/blob/v0.24.0-alpha.22/packages/crypto/src/secp256k1.spec.ts#L195-L394
    const COSMOS_TESTS_JSON: &str = "./testdata/secp256k1_tests.json";

    #[test]
    fn secp256k1_verify() {
        // Signing
        let secret_key = SigningKey::random(&mut OsRng); // Serialize with `::to_bytes()`

        // Note: the signature type must be annotated or otherwise inferrable as
        // `Signer` has many impls of the `Signer` trait (for both regular and
        // recoverable signature types).
        let signature: Signature = secret_key.sign(MSG.as_bytes()); // Message is internally digested

        // Verification
        let public_key = VerifyingKey::from(&secret_key); // Serialize with `::to_encoded_point()`
        assert!(public_key.verify(MSG.as_bytes(), &signature).is_ok());

        // Wrong message fails
        let message_bad = [MSG, "\0"].concat();
        assert!(public_key
            .verify(message_bad.as_bytes(), &signature)
            .is_err());

        // Other pubkey fails
        let other_secret_key = SigningKey::random(&mut OsRng); // Serialize with `::to_bytes()`
        let other_public_key = VerifyingKey::from(&other_secret_key); // Serialize with `::to_encoded_point()`
        assert!(other_public_key.verify(MSG.as_bytes(), &signature).is_err());
    }

    #[test]
    fn cosmos_secp256k1_verify() {
        let public_key =
            VerifyingKey::from_sec1_bytes(&base64::decode(COSMOS_PUBKEY_BASE64).unwrap()).unwrap();

        for ((i, msg), sig) in (1..)
            .zip(&[COSMOS_MSG_HEX1, COSMOS_MSG_HEX2, COSMOS_MSG_HEX3])
            .zip(&[
                COSMOS_SIGNATURE_HEX1,
                COSMOS_SIGNATURE_HEX2,
                COSMOS_SIGNATURE_HEX3,
            ])
        {
            let message = hex::decode(msg).unwrap();
            let signature = hex::decode(sig).unwrap();
            let signature = Signature::from_bytes(signature.as_slice()).unwrap();

            // Manual hash
            let hash = Sha256::new().chain(message.as_slice());

            // Verify works
            assert!(
                public_key.verify_digest(hash, &signature).is_ok(),
                format!("verify failed (test case {})", i)
            );
        }
    }

    #[test]
    fn cosmos_extra_secp256k1_verify() {
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
        };

        // Open the file in read-only mode with buffer.
        let file = File::open(COSMOS_TESTS_JSON).unwrap();
        let reader = BufReader::new(file);

        let codes: Vec<Encoded> = serde_json::from_reader(reader).unwrap();

        for (i, encoded) in (1..).zip(codes) {
            let message = hex::decode(&encoded.message).unwrap();

            let hash = hex::decode(&encoded.message_hash).unwrap();
            let message_hash = Sha256::new().chain(message.as_slice());
            assert_eq!(hash.as_slice(), &*message_hash.clone().finalize());

            let public_key =
                VerifyingKey::from_sec1_bytes(&hex::decode(&encoded.public_key).unwrap()).unwrap();

            let signature = hex::decode(&encoded.signature).unwrap();
            let mut signature = Signature::from_bytes(signature.as_slice()).unwrap();
            // Non low-S signatures require normalization
            signature.normalize_s().unwrap();

            // Verify works
            assert!(
                public_key.verify_digest(message_hash, &signature).is_ok(),
                format!("verify() failed (test case {})", i)
            );
        }
    }
}
