// Trace entry structures

use std::collections::HashMap;
use std::time::SystemTime;
use std::thread::ThreadId;
use serde::{Deserialize, Serialize};

/// A single trace entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntry {
    /// Trace step identifier
    pub step: Option<u32>,
    /// Trace location string
    pub location: String,
    /// Trace message content
    pub message: String,
    /// Timestamp of trace creation
    #[serde(with = "systemtime_serde")]
    pub timestamp: SystemTime,
    /// Target thread ID
    #[serde(skip)]
    pub thread_id: Option<ThreadId>,
    /// Meta data associated with trace
    pub metadata: Option<TraceMetadata>,
    /// Type of the trace entry
    pub entry_type: TraceEntryType,
}

/// Type of trace entry
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceEntryType {
    /// Step type
    Step,
    /// Milestone type
    Milestone,
    /// Error type
    Error,
}

/// Metadata associated with a trace entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceMetadata {
    /// Optional block number
    pub block_number: Option<u32>,
    /// Optional block hash
    pub block_hash: Option<String>,
    /// Optional author account
    pub author: Option<String>,
    /// Optional extrinsic count
    pub extrinsic_count: Option<usize>,
    /// Set of custom properties
    pub custom: HashMap<String, String>,
}

impl TraceMetadata {
    /// Create new empty trace metadata
    pub fn new() -> Self {
        Self {
            block_number: None,
            block_hash: None,
            author: None,
            extrinsic_count: None,
            custom: HashMap::new(),
        }
    }

    /// Add block number
    pub fn with_block_number(mut self, block_number: u32) -> Self {
        self.block_number = Some(block_number);
        self
    }

    /// Add block hash
    pub fn with_block_hash(mut self, block_hash: String) -> Self {
        self.block_hash = Some(block_hash);
        self
    }

    /// Add author
    pub fn with_author(mut self, author: String) -> Self {
        self.author = Some(author);
        self
    }

    /// Add extrinsic count
    pub fn with_extrinsic_count(mut self, count: usize) -> Self {
        self.extrinsic_count = Some(count);
        self
    }

    /// Add custom property
    pub fn with_custom(mut self, key: String, value: String) -> Self {
        self.custom.insert(key, value);
        self
    }
}

// Custom serialization for SystemTime
mod systemtime_serde {
    use std::time::{SystemTime, UNIX_EPOCH};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH)
            .map_err(|e| serde::ser::Error::custom(format!("SystemTime error: {}", e)))?;
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(secs))
    }
}
