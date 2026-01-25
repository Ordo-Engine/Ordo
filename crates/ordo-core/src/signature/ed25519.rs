//! Ed25519 signature utilities

use crate::error::{OrdoError, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};

pub const PUBLIC_KEY_LEN: usize = 32;
pub const SIGNATURE_LEN: usize = 64;
pub const PRIVATE_KEY_LEN: usize = 32;

pub fn decode_public_key(encoded: &str) -> Result<VerifyingKey> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| OrdoError::parse_error(format!("Invalid public key base64: {e}")))?;
    let bytes: [u8; PUBLIC_KEY_LEN] = bytes.try_into().map_err(|_| {
        OrdoError::parse_error(format!(
            "Invalid public key length: expected {PUBLIC_KEY_LEN}"
        ))
    })?;
    VerifyingKey::from_bytes(&bytes)
        .map_err(|e| OrdoError::parse_error(format!("Invalid public key: {e}")))
}

pub fn decode_private_key(encoded: &str) -> Result<SigningKey> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| OrdoError::parse_error(format!("Invalid private key base64: {e}")))?;
    let bytes: [u8; PRIVATE_KEY_LEN] = bytes.try_into().map_err(|_| {
        OrdoError::parse_error(format!(
            "Invalid private key length: expected {PRIVATE_KEY_LEN}"
        ))
    })?;
    Ok(SigningKey::from_bytes(&bytes))
}

pub fn decode_signature(encoded: &str) -> Result<Signature> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|e| OrdoError::parse_error(format!("Invalid signature base64: {e}")))?;
    let bytes: [u8; SIGNATURE_LEN] = bytes.try_into().map_err(|_| {
        OrdoError::parse_error(format!(
            "Invalid signature length: expected {SIGNATURE_LEN}"
        ))
    })?;
    Ok(Signature::from_bytes(&bytes))
}

pub fn encode_public_key(key: &VerifyingKey) -> String {
    STANDARD.encode(key.as_bytes())
}

pub fn encode_private_key(key: &SigningKey) -> String {
    STANDARD.encode(key.as_bytes())
}

pub fn encode_signature(signature: &Signature) -> String {
    STANDARD.encode(signature.to_bytes())
}
