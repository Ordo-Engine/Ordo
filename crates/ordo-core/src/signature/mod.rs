//! Rule signature support

use crate::error::{OrdoError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;

pub mod ed25519;
pub mod signer;
pub mod verifier;

pub use signer::RuleSigner;
pub use verifier::RuleVerifier;

/// Reserved field name for rule signatures
pub const SIGNATURE_FIELD: &str = "_signature";

/// Supported signature algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureAlgorithm {
    Ed25519,
}

/// Signature metadata embedded in rule files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureConfig {
    pub algorithm: SignatureAlgorithm,
    pub public_key: String,
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_at: Option<String>,
}

/// Remove the signature field from JSON and return it if present.
pub fn strip_signature(value: &mut JsonValue) -> Result<Option<SignatureConfig>> {
    let JsonValue::Object(map) = value else {
        return Ok(None);
    };

    let Some(signature_value) = map.remove(SIGNATURE_FIELD) else {
        return Ok(None);
    };

    serde_json::from_value(signature_value)
        .map(Some)
        .map_err(|e| OrdoError::parse_error(format!("Invalid signature field: {e}")))
}

/// Convert JSON into a canonical form with sorted object keys.
pub fn canonicalize_json(value: &JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut sorted = BTreeMap::new();
            for (key, value) in map {
                sorted.insert(key.clone(), canonicalize_json(value));
            }
            JsonValue::Object(sorted.into_iter().collect())
        }
        JsonValue::Array(values) => {
            JsonValue::Array(values.iter().map(canonicalize_json).collect())
        }
        _ => value.clone(),
    }
}

/// Serialize JSON into canonical bytes for signing.
pub fn canonical_json_bytes(value: &JsonValue) -> Result<Vec<u8>> {
    let canonical = canonicalize_json(value);
    serde_json::to_vec(&canonical)
        .map_err(|e| OrdoError::parse_error(format!("Canonical JSON error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signature::ed25519::decode_public_key;
    use crate::signature::signer::RuleSigner;
    use crate::signature::verifier::RuleVerifier;
    use serde_json::json;

    #[test]
    fn test_signature_roundtrip() {
        let (public_key, private_key) = RuleSigner::generate_keypair();
        let signer = RuleSigner::from_private_key_base64(&private_key).unwrap();
        let verifier = RuleVerifier::new(vec![decode_public_key(&public_key).unwrap()], true);

        let value = json!({
            "config": { "name": "test", "entry_step": "start" },
            "steps": {}
        });
        let signature = signer.sign_json_value(&value, None).unwrap();
        verifier
            .verify_json_value(&value, Some(&signature))
            .unwrap();
    }

    #[test]
    fn test_strip_signature() {
        let mut value = json!({
            "config": { "name": "test", "entry_step": "start" },
            "_signature": {
                "algorithm": "ed25519",
                "public_key": "pub",
                "signature": "sig"
            }
        });

        let signature = strip_signature(&mut value).unwrap();
        assert!(signature.is_some());
        assert!(value.get(SIGNATURE_FIELD).is_none());
    }

    #[test]
    fn test_signature_detects_tamper() {
        let (public_key, private_key) = RuleSigner::generate_keypair();
        let signer = RuleSigner::from_private_key_base64(&private_key).unwrap();
        let verifier = RuleVerifier::new(vec![decode_public_key(&public_key).unwrap()], true);

        let mut value = json!({
            "config": { "name": "test", "entry_step": "start" },
            "steps": { "start": { "id": "start", "type": "terminal", "result": { "code": "OK" } } }
        });
        let signature = signer.sign_json_value(&value, None).unwrap();

        if let JsonValue::Object(map) = &mut value {
            map.insert(
                SIGNATURE_FIELD.to_string(),
                serde_json::to_value(signature).unwrap(),
            );
            map.insert("extra".to_string(), JsonValue::Bool(true));
        }

        let signature = strip_signature(&mut value).unwrap();
        let result = verifier.verify_json_value(&value, signature.as_ref());
        assert!(result.is_err());
    }
}
