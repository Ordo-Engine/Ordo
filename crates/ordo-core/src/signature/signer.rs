//! Rule signing helpers

use crate::error::{OrdoError, Result};
use crate::signature::ed25519::{
    decode_private_key, encode_private_key, encode_public_key, encode_signature,
};
use crate::signature::{canonical_json_bytes, SignatureAlgorithm, SignatureConfig};
use ed25519_dalek::Signer;
use ed25519_dalek::SigningKey;
use serde_json::Value as JsonValue;

pub struct RuleSigner {
    signing_key: SigningKey,
}

impl RuleSigner {
    pub fn from_private_key_base64(encoded: &str) -> Result<Self> {
        Ok(Self {
            signing_key: decode_private_key(encoded)?,
        })
    }

    pub fn signing_key(&self) -> &SigningKey {
        &self.signing_key
    }

    pub fn sign_json_value(
        &self,
        value: &JsonValue,
        signed_at: Option<String>,
    ) -> Result<SignatureConfig> {
        let bytes = canonical_json_bytes(value)?;
        let signature = self.signing_key.sign(&bytes);
        let public_key = self.signing_key.verifying_key();

        Ok(SignatureConfig {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: encode_public_key(&public_key),
            signature: encode_signature(&signature),
            signed_at,
        })
    }

    pub fn sign_bytes(&self, data: &[u8]) -> String {
        let signature = self.signing_key.sign(data);
        encode_signature(&signature)
    }

    pub fn public_key_base64(&self) -> String {
        encode_public_key(&self.signing_key.verifying_key())
    }

    pub fn private_key_base64(&self) -> String {
        encode_private_key(&self.signing_key)
    }

    pub fn generate_keypair() -> (String, String) {
        let mut rng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        (
            encode_public_key(&signing_key.verifying_key()),
            encode_private_key(&signing_key),
        )
    }
}

impl TryFrom<&str> for RuleSigner {
    type Error = OrdoError;

    fn try_from(encoded: &str) -> Result<Self> {
        Self::from_private_key_base64(encoded)
    }
}
