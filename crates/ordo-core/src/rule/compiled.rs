//! Compiled ruleset structures and serialization
//!
//! Provides binary format support for compiled rulesets to protect rule logic.

use crate::context::Value;
use crate::error::{OrdoError, Result};
use crate::expr::CompiledExpr;
use crate::signature::ed25519::{PUBLIC_KEY_LEN, SIGNATURE_LEN};
use crate::signature::{SignatureAlgorithm, SignatureConfig};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use hashbrown::HashMap as HbMap;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;

const MAGIC: &[u8; 4] = b"ORDO";
const VERSION: u16 = 1;
const FLAG_HAS_SIGNATURE: u16 = 0b0001;

/// Maximum allowed size for collections during deserialization (prevent DoS attacks)
const MAX_COLLECTION_SIZE: usize = 1_000_000;
/// Maximum recursion depth for nested values (prevent stack overflow)
const MAX_VALUE_DEPTH: usize = 64;

/// Field missing behavior constants (must match FieldMissingBehavior enum)
pub const FIELD_MISSING_LENIENT: u8 = 0;
pub const FIELD_MISSING_STRICT: u8 = 1;
pub const FIELD_MISSING_DEFAULT: u8 = 2;

// ============================================================================
// Enterprise Plugin System
// ============================================================================

/// Enterprise plugin interface.
///
/// Allows enterprise edition to extend compilation and execution features,
/// such as string encryption, audit logging, etc.
/// Open source edition uses the default `NoOpEnterprisePlugin` implementation.
pub trait EnterprisePlugin: Send + Sync + 'static {
    /// Plugin name
    fn name(&self) -> &str {
        "noop"
    }

    /// Process string during serialization (e.g., encryption)
    /// - `data`: Original string bytes
    /// - `index`: String index in the pool
    /// - `salt`: Random salt value
    fn process_string_write(&self, data: &[u8], _index: u32, _salt: u32) -> Vec<u8> {
        data.to_vec()
    }

    /// Process string during deserialization (e.g., decryption)
    fn process_string_read(&self, data: &[u8], _index: u32, _salt: u32) -> Result<String> {
        String::from_utf8(data.to_vec()).map_err(|_| OrdoError::parse_error("Invalid UTF-8 string"))
    }

    /// Whether string processing is enabled
    fn is_string_processing_enabled(&self) -> bool {
        false
    }

    /// Generate random salt value
    fn generate_salt(&self) -> u32 {
        0
    }

    /// Audit log: rule compilation
    fn audit_compile(&self, _ruleset_name: &str, _success: bool) {}

    /// Audit log: rule execution
    fn audit_execute(&self, _ruleset_name: &str, _result_code: &str, _duration_us: u64) {}

    /// License verification
    fn verify_license(&self) -> Result<()> {
        Ok(())
    }
}

/// Default plugin: no-op (open source edition)
#[derive(Debug, Clone, Default)]
pub struct NoOpEnterprisePlugin;

impl EnterprisePlugin for NoOpEnterprisePlugin {}

/// Global enterprise plugin registry
static ENTERPRISE_PLUGIN: std::sync::OnceLock<Arc<dyn EnterprisePlugin>> =
    std::sync::OnceLock::new();

/// Register enterprise plugin (should be called once at program startup)
pub fn register_enterprise_plugin(plugin: Arc<dyn EnterprisePlugin>) -> Result<()> {
    ENTERPRISE_PLUGIN
        .set(plugin)
        .map_err(|_| OrdoError::parse_error("Enterprise plugin already registered"))
}

/// Get current enterprise plugin
pub fn get_enterprise_plugin() -> Arc<dyn EnterprisePlugin> {
    ENTERPRISE_PLUGIN
        .get()
        .cloned()
        .unwrap_or_else(|| Arc::new(NoOpEnterprisePlugin))
}

// ============================================================================
// Compiled RuleSet Structures
// ============================================================================

#[derive(Debug, Clone)]
pub struct CompiledMetadata {
    pub name: u32,
    pub tenant_id: Option<u32>,
    pub version: u32,
    pub description: u32,
    pub field_missing: u8,
    pub max_depth: u32,
    pub timeout_ms: u64,
    pub enable_trace: bool,
    pub metadata: Vec<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct CompiledRuleSet {
    pub metadata: CompiledMetadata,
    pub entry_step: u32,
    pub steps: Vec<CompiledStep>,
    pub expressions: Vec<CompiledExpr>,
    pub string_pool: Vec<String>,
    pub signature: Option<CompiledSignature>,
    step_index: HashMap<u32, usize>,
}

#[derive(Debug, Clone)]
pub struct CompiledSignature {
    pub public_key: [u8; PUBLIC_KEY_LEN],
    pub signature: [u8; SIGNATURE_LEN],
}

impl CompiledRuleSet {
    pub fn new(
        metadata: CompiledMetadata,
        entry_step: u32,
        steps: Vec<CompiledStep>,
        expressions: Vec<CompiledExpr>,
        string_pool: Vec<String>,
    ) -> Self {
        let mut ruleset = Self {
            metadata,
            entry_step,
            steps,
            expressions,
            string_pool,
            signature: None,
            step_index: HashMap::new(),
        };
        ruleset.rebuild_index();
        ruleset
    }

    pub fn get_step(&self, step_hash: u32) -> Result<&CompiledStep> {
        let index =
            self.step_index
                .get(&step_hash)
                .copied()
                .ok_or_else(|| OrdoError::StepNotFound {
                    step_id: format!("{step_hash}"),
                })?;
        self.steps
            .get(index)
            .ok_or_else(|| OrdoError::StepNotFound {
                step_id: format!("{step_hash}"),
            })
    }

    pub fn get_string(&self, index: u32) -> Result<&str> {
        self.string_pool
            .get(index as usize)
            .map(|s| s.as_str())
            .ok_or_else(|| OrdoError::parse_error("String pool index out of range"))
    }

    pub fn rebuild_index(&mut self) {
        self.step_index.clear();
        for (idx, step) in self.steps.iter().enumerate() {
            self.step_index.insert(step.id_hash(), idx);
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        write_u16(&mut out, VERSION);
        let mut flags = 0u16;
        if self.signature.is_some() {
            flags |= FLAG_HAS_SIGNATURE;
        }
        write_u16(&mut out, flags);
        // Reserve space for checksum (will be filled later)
        let checksum_pos = out.len();
        write_u32(&mut out, 0); // placeholder for checksum
        write_u32(&mut out, 0); // reserved

        if let Some(signature) = &self.signature {
            let length = (PUBLIC_KEY_LEN + SIGNATURE_LEN) as u16;
            write_u16(&mut out, length);
            out.extend_from_slice(&signature.public_key);
            out.extend_from_slice(&signature.signature);
        }

        self.serialize_payload(&mut out);

        // Calculate and write checksum (CRC32 of data after header)
        let checksum = crc32_hash(&out[16..]);
        out[checksum_pos..checksum_pos + 4].copy_from_slice(&checksum.to_le_bytes());

        out
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 16 {
            return Err(OrdoError::parse_error("Compiled ruleset too short"));
        }

        let mut cursor = Cursor::new(bytes);
        let magic = read_bytes(&mut cursor, 4)?;
        if magic.as_slice() != MAGIC {
            return Err(OrdoError::parse_error("Invalid compiled ruleset header"));
        }

        let version = read_u16(&mut cursor)?;
        if version > VERSION {
            return Err(OrdoError::parse_error(format!(
                "Unsupported compiled ruleset version: {} (max supported: {})",
                version, VERSION
            )));
        }

        let flags = read_u16(&mut cursor)?;
        let stored_checksum = read_u32(&mut cursor)?;
        let _reserved = read_u32(&mut cursor)?;

        // Verify checksum
        let computed_checksum = crc32_hash(&bytes[16..]);
        if stored_checksum != computed_checksum {
            return Err(OrdoError::parse_error(format!(
                "Checksum mismatch: expected {:08x}, got {:08x}",
                stored_checksum, computed_checksum
            )));
        }

        let signature = if flags & FLAG_HAS_SIGNATURE != 0 {
            let length = read_u16(&mut cursor)? as usize;
            if length != PUBLIC_KEY_LEN + SIGNATURE_LEN {
                return Err(OrdoError::parse_error(format!(
                    "Invalid signature length: expected {}, got {}",
                    PUBLIC_KEY_LEN + SIGNATURE_LEN,
                    length
                )));
            }
            let public_key = read_bytes(&mut cursor, PUBLIC_KEY_LEN)?;
            let signature = read_bytes(&mut cursor, SIGNATURE_LEN)?;
            Some(CompiledSignature {
                public_key: public_key
                    .as_slice()
                    .try_into()
                    .map_err(|_| OrdoError::parse_error("Invalid public key length"))?,
                signature: signature
                    .as_slice()
                    .try_into()
                    .map_err(|_| OrdoError::parse_error("Invalid signature length"))?,
            })
        } else {
            None
        };

        let string_count = read_u32(&mut cursor)? as usize;
        if string_count > MAX_COLLECTION_SIZE {
            return Err(OrdoError::parse_error(format!(
                "String pool size {} exceeds maximum {}",
                string_count, MAX_COLLECTION_SIZE
            )));
        }
        let mut string_pool = Vec::with_capacity(string_count);
        for _ in 0..string_count {
            string_pool.push(read_string(&mut cursor)?);
        }

        let metadata = read_metadata(&mut cursor)?;

        let expr_count = read_u32(&mut cursor)? as usize;
        if expr_count > MAX_COLLECTION_SIZE {
            return Err(OrdoError::parse_error(format!(
                "Expression count {} exceeds maximum {}",
                expr_count, MAX_COLLECTION_SIZE
            )));
        }
        let mut expressions = Vec::with_capacity(expr_count);
        for _ in 0..expr_count {
            let len = read_u32(&mut cursor)? as usize;
            let bytes = read_bytes(&mut cursor, len)?;
            expressions.push(CompiledExpr::deserialize(&bytes)?);
        }

        let step_count = read_u32(&mut cursor)? as usize;
        if step_count > MAX_COLLECTION_SIZE {
            return Err(OrdoError::parse_error(format!(
                "Step count {} exceeds maximum {}",
                step_count, MAX_COLLECTION_SIZE
            )));
        }
        let mut steps = Vec::with_capacity(step_count);
        for _ in 0..step_count {
            steps.push(CompiledStep::deserialize(&mut cursor)?);
        }

        let entry_step = read_u32(&mut cursor)?;

        let mut ruleset = Self::new(metadata, entry_step, steps, expressions, string_pool);
        ruleset.signature = signature;
        Ok(ruleset)
    }

    pub fn deserialize_with_verifier(
        bytes: &[u8],
        verifier: &crate::signature::verifier::RuleVerifier,
    ) -> Result<Self> {
        verify_compiled_signature_bytes(bytes, verifier)?;
        Self::deserialize(bytes)
    }

    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.serialize();
        fs::write(path, bytes).map_err(|e| OrdoError::parse_error(e.to_string()))
    }

    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let bytes = fs::read(path).map_err(|e| OrdoError::parse_error(e.to_string()))?;
        Self::deserialize(&bytes)
    }

    pub fn load_from_file_with_verifier(
        path: impl AsRef<Path>,
        verifier: &crate::signature::verifier::RuleVerifier,
    ) -> Result<Self> {
        let bytes = fs::read(path).map_err(|e| OrdoError::parse_error(e.to_string()))?;
        Self::deserialize_with_verifier(&bytes, verifier)
    }

    pub fn sign_with_signer(
        &mut self,
        signer: &crate::signature::signer::RuleSigner,
    ) -> Result<()> {
        use ed25519_dalek::Signer;

        let payload = self.serialize_payload_bytes();
        let signature = signer.signing_key().sign(&payload);
        let public_key = signer.signing_key().verifying_key();

        self.signature = Some(CompiledSignature {
            public_key: public_key.to_bytes(),
            signature: signature.to_bytes(),
        });
        Ok(())
    }

    fn serialize_payload_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.serialize_payload(&mut out);
        out
    }

    fn serialize_payload(&self, out: &mut Vec<u8>) {
        write_u32(out, self.string_pool.len() as u32);
        for value in &self.string_pool {
            write_string(out, value);
        }

        write_metadata(out, &self.metadata);

        write_u32(out, self.expressions.len() as u32);
        for expr in &self.expressions {
            let bytes = expr.serialize();
            write_u32(out, bytes.len() as u32);
            out.extend_from_slice(&bytes);
        }

        write_u32(out, self.steps.len() as u32);
        for step in &self.steps {
            step.serialize(out);
        }

        write_u32(out, self.entry_step);
    }
}

fn verify_compiled_signature_bytes(
    bytes: &[u8],
    verifier: &crate::signature::verifier::RuleVerifier,
) -> Result<()> {
    if bytes.len() < 16 {
        return Err(OrdoError::parse_error("Compiled ruleset too short"));
    }

    let mut cursor = Cursor::new(bytes);
    let magic = read_bytes(&mut cursor, 4)?;
    if magic.as_slice() != MAGIC {
        return Err(OrdoError::parse_error("Invalid compiled ruleset header"));
    }

    let version = read_u16(&mut cursor)?;
    if version > VERSION {
        return Err(OrdoError::parse_error(format!(
            "Unsupported compiled ruleset version: {} (max supported: {})",
            version, VERSION
        )));
    }

    let flags = read_u16(&mut cursor)?;
    let _checksum = read_u32(&mut cursor)?;
    let _reserved = read_u32(&mut cursor)?;

    if flags & FLAG_HAS_SIGNATURE == 0 {
        if verifier.require_signature() {
            return Err(OrdoError::parse_error("Missing compiled ruleset signature"));
        }
        return Ok(());
    }

    let length = read_u16(&mut cursor)? as usize;
    if length != PUBLIC_KEY_LEN + SIGNATURE_LEN {
        return Err(OrdoError::parse_error("Invalid compiled signature length"));
    }
    let public_key = read_bytes(&mut cursor, PUBLIC_KEY_LEN)?;
    let signature = read_bytes(&mut cursor, SIGNATURE_LEN)?;

    let signature_config = SignatureConfig {
        algorithm: SignatureAlgorithm::Ed25519,
        public_key: STANDARD.encode(public_key),
        signature: STANDARD.encode(signature),
        signed_at: None,
    };

    let payload = &bytes[cursor.pos..];
    verifier.verify_bytes(payload, &signature_config)
}

#[derive(Debug, Clone)]
pub enum CompiledStep {
    Decision {
        id_hash: u32,
        branches: Vec<CompiledBranch>,
        default_next: Option<u32>,
    },
    Action {
        id_hash: u32,
        actions: Vec<CompiledAction>,
        next_step: u32,
    },
    Terminal {
        id_hash: u32,
        code: u32,
        message: u32,
        outputs: Vec<CompiledOutput>,
        data: Value,
    },
}

impl CompiledStep {
    pub fn id_hash(&self) -> u32 {
        match self {
            CompiledStep::Decision { id_hash, .. } => *id_hash,
            CompiledStep::Action { id_hash, .. } => *id_hash,
            CompiledStep::Terminal { id_hash, .. } => *id_hash,
        }
    }

    fn serialize(&self, out: &mut Vec<u8>) {
        match self {
            CompiledStep::Decision {
                id_hash,
                branches,
                default_next,
            } => {
                write_u8(out, 0);
                write_u32(out, *id_hash);
                write_u32(out, branches.len() as u32);
                for branch in branches {
                    branch.serialize(out);
                }
                write_option_u32(out, *default_next);
            }
            CompiledStep::Action {
                id_hash,
                actions,
                next_step,
            } => {
                write_u8(out, 1);
                write_u32(out, *id_hash);
                write_u32(out, actions.len() as u32);
                for action in actions {
                    action.serialize(out);
                }
                write_u32(out, *next_step);
            }
            CompiledStep::Terminal {
                id_hash,
                code,
                message,
                outputs,
                data,
            } => {
                write_u8(out, 2);
                write_u32(out, *id_hash);
                write_u32(out, *code);
                write_u32(out, *message);
                write_u32(out, outputs.len() as u32);
                for output in outputs {
                    output.serialize(out);
                }
                write_value(out, data);
            }
        }
    }

    fn deserialize(cursor: &mut Cursor<'_>) -> Result<Self> {
        let tag = read_u8(cursor)?;
        match tag {
            0 => {
                let id_hash = read_u32(cursor)?;
                let count = read_u32(cursor)? as usize;
                let mut branches = Vec::with_capacity(count);
                for _ in 0..count {
                    branches.push(CompiledBranch::deserialize(cursor)?);
                }
                let default_next = read_option_u32(cursor)?;
                Ok(CompiledStep::Decision {
                    id_hash,
                    branches,
                    default_next,
                })
            }
            1 => {
                let id_hash = read_u32(cursor)?;
                let count = read_u32(cursor)? as usize;
                let mut actions = Vec::with_capacity(count);
                for _ in 0..count {
                    actions.push(CompiledAction::deserialize(cursor)?);
                }
                let next_step = read_u32(cursor)?;
                Ok(CompiledStep::Action {
                    id_hash,
                    actions,
                    next_step,
                })
            }
            2 => {
                let id_hash = read_u32(cursor)?;
                let code = read_u32(cursor)?;
                let message = read_u32(cursor)?;
                let count = read_u32(cursor)? as usize;
                let mut outputs = Vec::with_capacity(count);
                for _ in 0..count {
                    outputs.push(CompiledOutput::deserialize(cursor)?);
                }
                let data = read_value(cursor)?;
                Ok(CompiledStep::Terminal {
                    id_hash,
                    code,
                    message,
                    outputs,
                    data,
                })
            }
            _ => Err(OrdoError::parse_error("Unknown compiled step tag")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompiledBranch {
    pub condition: CompiledCondition,
    pub next_step: u32,
    pub actions: Vec<CompiledAction>,
}

impl CompiledBranch {
    fn serialize(&self, out: &mut Vec<u8>) {
        self.condition.serialize(out);
        write_u32(out, self.next_step);
        write_u32(out, self.actions.len() as u32);
        for action in &self.actions {
            action.serialize(out);
        }
    }

    fn deserialize(cursor: &mut Cursor<'_>) -> Result<Self> {
        let condition = CompiledCondition::deserialize(cursor)?;
        let next_step = read_u32(cursor)?;
        let count = read_u32(cursor)? as usize;
        let mut actions = Vec::with_capacity(count);
        for _ in 0..count {
            actions.push(CompiledAction::deserialize(cursor)?);
        }
        Ok(Self {
            condition,
            next_step,
            actions,
        })
    }
}

#[derive(Debug, Clone)]
pub enum CompiledCondition {
    Always,
    Expr(u32),
}

impl CompiledCondition {
    fn serialize(&self, out: &mut Vec<u8>) {
        match self {
            CompiledCondition::Always => {
                write_u8(out, 0);
            }
            CompiledCondition::Expr(idx) => {
                write_u8(out, 1);
                write_u32(out, *idx);
            }
        }
    }

    fn deserialize(cursor: &mut Cursor<'_>) -> Result<Self> {
        match read_u8(cursor)? {
            0 => Ok(CompiledCondition::Always),
            1 => Ok(CompiledCondition::Expr(read_u32(cursor)?)),
            _ => Err(OrdoError::parse_error("Unknown compiled condition tag")),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CompiledAction {
    SetVariable {
        name: u32,
        value: u32,
    },
    Log {
        message: u32,
        level: u8,
    },
    Metric {
        name: u32,
        value: u32,
        tags: Vec<(u32, u32)>,
    },
}

impl CompiledAction {
    fn serialize(&self, out: &mut Vec<u8>) {
        match self {
            CompiledAction::SetVariable { name, value } => {
                write_u8(out, 0);
                write_u32(out, *name);
                write_u32(out, *value);
            }
            CompiledAction::Log { message, level } => {
                write_u8(out, 1);
                write_u32(out, *message);
                write_u8(out, *level);
            }
            CompiledAction::Metric { name, value, tags } => {
                write_u8(out, 2);
                write_u32(out, *name);
                write_u32(out, *value);
                write_u32(out, tags.len() as u32);
                for (k, v) in tags {
                    write_u32(out, *k);
                    write_u32(out, *v);
                }
            }
        }
    }

    fn deserialize(cursor: &mut Cursor<'_>) -> Result<Self> {
        match read_u8(cursor)? {
            0 => Ok(CompiledAction::SetVariable {
                name: read_u32(cursor)?,
                value: read_u32(cursor)?,
            }),
            1 => Ok(CompiledAction::Log {
                message: read_u32(cursor)?,
                level: read_u8(cursor)?,
            }),
            2 => {
                let name = read_u32(cursor)?;
                let value = read_u32(cursor)?;
                let count = read_u32(cursor)? as usize;
                let mut tags = Vec::with_capacity(count);
                for _ in 0..count {
                    tags.push((read_u32(cursor)?, read_u32(cursor)?));
                }
                Ok(CompiledAction::Metric { name, value, tags })
            }
            _ => Err(OrdoError::parse_error("Unknown compiled action tag")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompiledOutput {
    pub key: u32,
    pub expr: u32,
}

impl CompiledOutput {
    fn serialize(&self, out: &mut Vec<u8>) {
        write_u32(out, self.key);
        write_u32(out, self.expr);
    }

    fn deserialize(cursor: &mut Cursor<'_>) -> Result<Self> {
        Ok(Self {
            key: read_u32(cursor)?,
            expr: read_u32(cursor)?,
        })
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
}

fn read_bytes(cursor: &mut Cursor<'_>, len: usize) -> Result<Vec<u8>> {
    if cursor.pos + len > cursor.bytes.len() {
        return Err(OrdoError::parse_error("Unexpected end of data"));
    }
    let slice = cursor.bytes[cursor.pos..cursor.pos + len].to_vec();
    cursor.pos += len;
    Ok(slice)
}

fn read_u8(cursor: &mut Cursor<'_>) -> Result<u8> {
    let bytes = read_bytes(cursor, 1)?;
    Ok(bytes[0])
}

fn read_u16(cursor: &mut Cursor<'_>) -> Result<u16> {
    let bytes = read_bytes(cursor, 2)?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(cursor: &mut Cursor<'_>) -> Result<u32> {
    let bytes = read_bytes(cursor, 4)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn read_u64(cursor: &mut Cursor<'_>) -> Result<u64> {
    let bytes = read_bytes(cursor, 8)?;
    Ok(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn read_string(cursor: &mut Cursor<'_>) -> Result<String> {
    let len = read_u32(cursor)? as usize;
    let bytes = read_bytes(cursor, len)?;
    std::str::from_utf8(&bytes)
        .map(|s| s.to_string())
        .map_err(|_| OrdoError::parse_error("Invalid UTF-8 string"))
}

fn read_option_u32(cursor: &mut Cursor<'_>) -> Result<Option<u32>> {
    let present = read_u8(cursor)?;
    if present == 0 {
        Ok(None)
    } else {
        Ok(Some(read_u32(cursor)?))
    }
}

fn read_value(cursor: &mut Cursor<'_>) -> Result<Value> {
    read_value_with_depth(cursor, 0)
}

fn read_value_with_depth(cursor: &mut Cursor<'_>, depth: usize) -> Result<Value> {
    if depth > MAX_VALUE_DEPTH {
        return Err(OrdoError::parse_error(format!(
            "Value nesting depth {} exceeds maximum {}",
            depth, MAX_VALUE_DEPTH
        )));
    }

    match read_u8(cursor)? {
        0 => Ok(Value::Null),
        1 => Ok(Value::Bool(read_u8(cursor)? != 0)),
        2 => Ok(Value::Int(read_i64(cursor)?)),
        3 => Ok(Value::Float(read_f64(cursor)?)),
        4 => Ok(Value::string(read_string(cursor)?)),
        5 => {
            let len = read_u32(cursor)? as usize;
            if len > MAX_COLLECTION_SIZE {
                return Err(OrdoError::parse_error(format!(
                    "Array size {} exceeds maximum {}",
                    len, MAX_COLLECTION_SIZE
                )));
            }
            let mut values = Vec::with_capacity(len);
            for _ in 0..len {
                values.push(read_value_with_depth(cursor, depth + 1)?);
            }
            Ok(Value::Array(values))
        }
        6 => {
            let len = read_u32(cursor)? as usize;
            if len > MAX_COLLECTION_SIZE {
                return Err(OrdoError::parse_error(format!(
                    "Object size {} exceeds maximum {}",
                    len, MAX_COLLECTION_SIZE
                )));
            }
            let mut map = HbMap::with_capacity(len);
            for _ in 0..len {
                let key = read_string(cursor)?;
                let value = read_value_with_depth(cursor, depth + 1)?;
                map.insert(std::sync::Arc::from(key.as_str()), value);
            }
            Ok(Value::Object(map))
        }
        _ => Err(OrdoError::parse_error("Unknown value tag")),
    }
}

fn read_i64(cursor: &mut Cursor<'_>) -> Result<i64> {
    let bytes = read_bytes(cursor, 8)?;
    Ok(i64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn read_f64(cursor: &mut Cursor<'_>) -> Result<f64> {
    let bytes = read_bytes(cursor, 8)?;
    Ok(f64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}

fn write_u8(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_string(out: &mut Vec<u8>, value: &str) {
    write_u32(out, value.len() as u32);
    out.extend_from_slice(value.as_bytes());
}

fn write_option_u32(out: &mut Vec<u8>, value: Option<u32>) {
    match value {
        None => write_u8(out, 0),
        Some(v) => {
            write_u8(out, 1);
            write_u32(out, v);
        }
    }
}

fn write_value(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => write_u8(out, 0),
        Value::Bool(v) => {
            write_u8(out, 1);
            write_u8(out, if *v { 1 } else { 0 });
        }
        Value::Int(v) => {
            write_u8(out, 2);
            write_i64(out, *v);
        }
        Value::Float(v) => {
            write_u8(out, 3);
            write_f64(out, *v);
        }
        Value::String(v) => {
            write_u8(out, 4);
            write_string(out, v.as_ref());
        }
        Value::Array(values) => {
            write_u8(out, 5);
            write_u32(out, values.len() as u32);
            for item in values {
                write_value(out, item);
            }
        }
        Value::Object(map) => {
            write_u8(out, 6);
            write_u32(out, map.len() as u32);
            for (key, val) in map {
                write_string(out, key.as_ref());
                write_value(out, val);
            }
        }
    }
}

fn write_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_metadata(out: &mut Vec<u8>, metadata: &CompiledMetadata) {
    write_u32(out, metadata.name);
    write_option_u32(out, metadata.tenant_id);
    write_u32(out, metadata.version);
    write_u32(out, metadata.description);
    write_u8(out, metadata.field_missing);
    write_u32(out, metadata.max_depth);
    write_u64(out, metadata.timeout_ms);
    write_u8(out, if metadata.enable_trace { 1 } else { 0 });
    write_u32(out, metadata.metadata.len() as u32);
    for (k, v) in &metadata.metadata {
        write_u32(out, *k);
        write_u32(out, *v);
    }
}

fn read_metadata(cursor: &mut Cursor<'_>) -> Result<CompiledMetadata> {
    let name = read_u32(cursor)?;
    let tenant_id = read_option_u32(cursor)?;
    let version = read_u32(cursor)?;
    let description = read_u32(cursor)?;
    let field_missing = read_u8(cursor)?;
    let max_depth = read_u32(cursor)?;
    let timeout_ms = read_u64(cursor)?;
    let enable_trace = read_u8(cursor)? != 0;
    let metadata_count = read_u32(cursor)? as usize;
    let mut metadata = Vec::with_capacity(metadata_count);
    for _ in 0..metadata_count {
        metadata.push((read_u32(cursor)?, read_u32(cursor)?));
    }
    Ok(CompiledMetadata {
        name,
        tenant_id,
        version,
        description,
        field_missing,
        max_depth,
        timeout_ms,
        enable_trace,
        metadata,
    })
}

/// CRC32 hash (IEEE polynomial)
fn crc32_hash(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::Expr;
    use crate::rule::{
        Action, ActionKind, CompiledRuleExecutor, Condition, RuleSet, RuleSetCompiler, Step,
        TerminalResult,
    };
    use crate::signature::ed25519::decode_public_key;
    use crate::signature::signer::RuleSigner;
    use crate::signature::verifier::RuleVerifier;

    fn build_ruleset() -> RuleSet {
        let mut ruleset = RuleSet::new("compiled_test", "start");
        ruleset.add_step(
            Step::decision("start", "Start")
                .branch(Condition::from_string("age >= 18"), "set_discount")
                .default("minor")
                .build(),
        );

        ruleset.add_step(Step::action(
            "set_discount",
            "Set Discount",
            vec![Action {
                kind: ActionKind::SetVariable {
                    name: "discount".to_string(),
                    value: Expr::literal(0.1f64),
                },
                description: "set discount".to_string(),
            }],
            "adult",
        ));

        ruleset.add_step(Step::terminal(
            "adult",
            "Adult",
            TerminalResult::new("ADULT").with_output("discount", Expr::field("$discount")),
        ));

        ruleset.add_step(Step::terminal(
            "minor",
            "Minor",
            TerminalResult::new("MINOR").with_output("discount", Expr::literal(0.0f64)),
        ));

        ruleset
    }

    #[test]
    fn test_compiled_ruleset_execute() {
        let ruleset = build_ruleset();
        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let executor = CompiledRuleExecutor::new();

        let input = serde_json::from_str(r#"{"age": 20}"#).unwrap();
        let result = executor.execute(&compiled, input).unwrap();
        assert_eq!(result.code, "ADULT");
        assert_eq!(result.output.get_path("discount"), Some(&Value::float(0.1)));

        let input = serde_json::from_str(r#"{"age": 10}"#).unwrap();
        let result = executor.execute(&compiled, input).unwrap();
        assert_eq!(result.code, "MINOR");
        assert_eq!(result.output.get_path("discount"), Some(&Value::float(0.0)));
    }

    #[test]
    fn test_compiled_ruleset_serialize_roundtrip() {
        let ruleset = build_ruleset();
        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let bytes = compiled.serialize();
        let decoded = CompiledRuleSet::deserialize(&bytes).unwrap();

        let executor = CompiledRuleExecutor::new();
        let input = serde_json::from_str(r#"{"age": 20}"#).unwrap();
        let result = executor.execute(&decoded, input).unwrap();
        assert_eq!(result.code, "ADULT");
    }

    #[test]
    fn test_compiled_ruleset_invalid_magic() {
        let mut bytes = b"XXXX".to_vec();
        bytes.extend_from_slice(&[0u8; 100]);
        let result = CompiledRuleSet::deserialize(&bytes);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid compiled ruleset header"));
    }

    #[test]
    fn test_compiled_ruleset_checksum_mismatch() {
        let ruleset = build_ruleset();
        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let mut bytes = compiled.serialize();
        // Corrupt some data after header
        if bytes.len() > 20 {
            bytes[20] ^= 0xFF;
        }
        let result = CompiledRuleSet::deserialize(&bytes);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Checksum mismatch"));
    }

    #[test]
    fn test_compiled_ruleset_too_short() {
        let bytes = vec![0u8; 10];
        let result = CompiledRuleSet::deserialize(&bytes);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn test_crc32_hash() {
        // Test vector: "123456789" should produce 0xCBF43926
        let data = b"123456789";
        let hash = crc32_hash(data);
        assert_eq!(hash, 0xCBF43926);
    }

    #[test]
    fn test_compiled_ruleset_with_complex_values() {
        let mut ruleset = RuleSet::new("complex_test", "start");
        ruleset.add_step(Step::terminal(
            "start",
            "Start",
            TerminalResult::new("OK")
                .with_output(
                    "array",
                    Expr::Array(vec![
                        Expr::literal(1i64),
                        Expr::literal(2i64),
                        Expr::literal(3i64),
                    ]),
                )
                .with_output("nested", Expr::field("user.profile.name")),
        ));

        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let bytes = compiled.serialize();
        let decoded = CompiledRuleSet::deserialize(&bytes).unwrap();

        let executor = CompiledRuleExecutor::new();
        let input = serde_json::from_str(r#"{"user": {"profile": {"name": "Alice"}}}"#).unwrap();
        let result = executor.execute(&decoded, input).unwrap();
        assert_eq!(result.code, "OK");
        assert_eq!(
            result.output.get_path("nested"),
            Some(&Value::string("Alice"))
        );
    }

    #[test]
    fn test_compiled_ruleset_file_roundtrip() {
        use std::env;

        // 构建一个更复杂的规则集
        let mut ruleset = RuleSet::new("loan_approval", "check_age");

        // Step 1: 检查年龄
        ruleset.add_step(
            Step::decision("check_age", "Check Age")
                .branch(Condition::from_string("age < 18"), "reject_minor")
                .branch(Condition::from_string("age >= 65"), "senior_review")
                .default("check_income")
                .build(),
        );

        // Step 2: 检查收入
        ruleset.add_step(
            Step::decision("check_income", "Check Income")
                .branch(
                    Condition::from_string("income >= 50000"),
                    "approve_standard",
                )
                .branch(Condition::from_string("income >= 30000"), "approve_limited")
                .default("reject_income")
                .build(),
        );

        // Step 3: 设置标准批准
        ruleset.add_step(Step::action(
            "approve_standard",
            "Set Standard Approval",
            vec![
                Action {
                    kind: ActionKind::SetVariable {
                        name: "limit".to_string(),
                        value: Expr::literal(100000i64),
                    },
                    description: "set credit limit".to_string(),
                },
                Action {
                    kind: ActionKind::SetVariable {
                        name: "rate".to_string(),
                        value: Expr::literal(0.05f64),
                    },
                    description: "set interest rate".to_string(),
                },
            ],
            "approved",
        ));

        // Step 4: 设置有限批准
        ruleset.add_step(Step::action(
            "approve_limited",
            "Set Limited Approval",
            vec![Action {
                kind: ActionKind::SetVariable {
                    name: "limit".to_string(),
                    value: Expr::literal(30000i64),
                },
                description: "set credit limit".to_string(),
            }],
            "approved",
        ));

        // Terminal steps
        ruleset.add_step(Step::terminal(
            "approved",
            "Approved",
            TerminalResult::new("APPROVED")
                .with_message("Loan approved")
                .with_output("credit_limit", Expr::field("$limit"))
                .with_output("applicant_age", Expr::field("age")),
        ));

        ruleset.add_step(Step::terminal(
            "reject_minor",
            "Reject Minor",
            TerminalResult::new("REJECTED").with_message("Applicant must be 18 or older"),
        ));

        ruleset.add_step(Step::terminal(
            "reject_income",
            "Reject Income",
            TerminalResult::new("REJECTED").with_message("Income too low"),
        ));

        ruleset.add_step(Step::terminal(
            "senior_review",
            "Senior Review",
            TerminalResult::new("REVIEW").with_message("Senior applicant requires manual review"),
        ));

        // 编译规则集
        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();

        // 保存到临时文件
        let temp_dir = env::temp_dir();
        let file_path = temp_dir.join("test_loan_approval.ordo");

        compiled.save_to_file(&file_path).unwrap();

        // 验证文件存在且大小合理
        let metadata = std::fs::metadata(&file_path).unwrap();
        assert!(metadata.len() > 100, "File should have reasonable size");
        println!("Compiled file size: {} bytes", metadata.len());

        // 从文件加载
        let loaded = CompiledRuleSet::load_from_file(&file_path).unwrap();

        // 创建执行器
        let executor = CompiledRuleExecutor::new();

        // 测试用例 1: 标准批准
        let input1 = serde_json::from_str(r#"{"age": 30, "income": 60000}"#).unwrap();
        let result1 = executor.execute(&loaded, input1).unwrap();
        assert_eq!(result1.code, "APPROVED");
        assert_eq!(
            result1.output.get_path("credit_limit"),
            Some(&Value::Int(100000))
        );
        assert_eq!(
            result1.output.get_path("applicant_age"),
            Some(&Value::Int(30))
        );

        // 测试用例 2: 有限批准
        let input2 = serde_json::from_str(r#"{"age": 25, "income": 35000}"#).unwrap();
        let result2 = executor.execute(&loaded, input2).unwrap();
        assert_eq!(result2.code, "APPROVED");
        assert_eq!(
            result2.output.get_path("credit_limit"),
            Some(&Value::Int(30000))
        );

        // 测试用例 3: 未成年拒绝
        let input3 = serde_json::from_str(r#"{"age": 16, "income": 100000}"#).unwrap();
        let result3 = executor.execute(&loaded, input3).unwrap();
        assert_eq!(result3.code, "REJECTED");
        assert_eq!(result3.message, "Applicant must be 18 or older");

        // 测试用例 4: 收入不足拒绝
        let input4 = serde_json::from_str(r#"{"age": 30, "income": 20000}"#).unwrap();
        let result4 = executor.execute(&loaded, input4).unwrap();
        assert_eq!(result4.code, "REJECTED");
        assert_eq!(result4.message, "Income too low");

        // 测试用例 5: 老年人审核
        let input5 = serde_json::from_str(r#"{"age": 70, "income": 80000}"#).unwrap();
        let result5 = executor.execute(&loaded, input5).unwrap();
        assert_eq!(result5.code, "REVIEW");

        // 保留文件供查看，不删除
        // std::fs::remove_file(&file_path).unwrap();

        println!("All file roundtrip tests passed!");
        println!("File saved at: {:?}", file_path);
    }

    #[test]
    fn test_compiled_ruleset_binary_is_unreadable() {
        let ruleset = build_ruleset();
        let compiled = RuleSetCompiler::compile(&ruleset).unwrap();
        let bytes = compiled.serialize();

        // 验证二进制文件不包含明文规则内容
        let bytes_str = String::from_utf8_lossy(&bytes);

        // 原始规则中的关键字不应该以明文形式出现在关键位置
        // 注意：字符串池中仍会有一些字符串，但结构已被混淆
        assert!(bytes.starts_with(b"ORDO"), "Should have ORDO magic header");

        // 验证不是纯 JSON
        assert!(
            !bytes_str.contains(r#""steps""#),
            "Should not contain JSON structure"
        );
        assert!(
            !bytes_str.contains(r#""branches""#),
            "Should not contain JSON structure"
        );

        println!("Binary format verification passed!");
        println!(
            "First 64 bytes (hex): {:02x?}",
            &bytes[..64.min(bytes.len())]
        );
    }

    #[test]
    fn test_compiled_ruleset_signature_verification() {
        let ruleset = build_ruleset();
        let mut compiled = RuleSetCompiler::compile(&ruleset).unwrap();

        let (public_key, private_key) = RuleSigner::generate_keypair();
        let signer = RuleSigner::from_private_key_base64(&private_key).unwrap();
        compiled.sign_with_signer(&signer).unwrap();
        let bytes = compiled.serialize();

        let verifier = RuleVerifier::new(vec![decode_public_key(&public_key).unwrap()], true);
        let verified = CompiledRuleSet::deserialize_with_verifier(&bytes, &verifier).unwrap();
        let executor = CompiledRuleExecutor::new();
        let input = serde_json::from_str(r#"{"age": 20}"#).unwrap();
        let result = executor.execute(&verified, input).unwrap();
        assert_eq!(result.code, "ADULT");
    }

    #[test]
    fn test_compiled_ruleset_signature_tamper_detected() {
        let ruleset = build_ruleset();
        let mut compiled = RuleSetCompiler::compile(&ruleset).unwrap();

        let (public_key, private_key) = RuleSigner::generate_keypair();
        let signer = RuleSigner::from_private_key_base64(&private_key).unwrap();
        compiled.sign_with_signer(&signer).unwrap();
        let mut bytes = compiled.serialize();

        if bytes.len() > 32 {
            bytes[32] ^= 0xFF;
        }

        let verifier = RuleVerifier::new(vec![decode_public_key(&public_key).unwrap()], true);
        let result = CompiledRuleSet::deserialize_with_verifier(&bytes, &verifier);
        assert!(result.is_err());
    }
}
