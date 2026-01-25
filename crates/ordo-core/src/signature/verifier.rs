//! Rule signature verification helpers

use crate::error::{OrdoError, Result};
use crate::signature::ed25519::{decode_public_key, decode_signature};
use crate::signature::{canonical_json_bytes, SignatureConfig};
use ed25519_dalek::Verifier;
use ed25519_dalek::VerifyingKey;
use serde_json::Value as JsonValue;

#[derive(Clone, Default)]
pub struct RuleVerifier {
    trusted_keys: Vec<VerifyingKey>,
    require_signature: bool,
}

impl RuleVerifier {
    pub fn new(trusted_keys: Vec<VerifyingKey>, require_signature: bool) -> Self {
        Self {
            trusted_keys,
            require_signature,
        }
    }

    pub fn require_signature(&self) -> bool {
        self.require_signature
    }

    pub fn trusted_keys(&self) -> &[VerifyingKey] {
        &self.trusted_keys
    }

    pub fn verify_json_value(
        &self,
        value: &JsonValue,
        signature: Option<&SignatureConfig>,
    ) -> Result<()> {
        match signature {
            Some(sig) => self.verify_with_signature(value, sig),
            None => {
                if self.require_signature {
                    Err(OrdoError::parse_error("Missing signature"))
                } else {
                    Ok(())
                }
            }
        }
    }

    pub fn verify_with_signature(
        &self,
        value: &JsonValue,
        signature: &SignatureConfig,
    ) -> Result<()> {
        let bytes = canonical_json_bytes(value)?;
        self.verify_bytes(&bytes, signature)
    }

    pub fn verify_bytes(&self, data: &[u8], signature: &SignatureConfig) -> Result<()> {
        let verifying_key = decode_public_key(&signature.public_key)?;
        let signature = decode_signature(&signature.signature)?;

        if !self.trusted_keys.is_empty()
            && !self.trusted_keys.iter().any(|key| key == &verifying_key)
        {
            return Err(OrdoError::parse_error("Untrusted public key"));
        }

        verifying_key
            .verify(data, &signature)
            .map_err(|_| OrdoError::parse_error("Signature verification failed"))
    }
}
