// Trace formatters

use crate::lifecycle_tracer::entry::{TraceEntry, TraceEntryType};
use std::time::{SystemTime, UNIX_EPOCH};

/// Trait for formatting trace entries
pub trait TraceFormatter: Send + Sync {
    fn format(&self, entry: &TraceEntry) -> String;
}

/// JSON formatter for structured output
pub struct JsonFormatter;

impl TraceFormatter for JsonFormatter {
    fn format(&self, entry: &TraceEntry) -> String {
        // Manual JSON construction for better control
        let mut json = String::from("{");
        
        // Add step if present
        if let Some(step) = entry.step {
            json.push_str(&format!(r#""step":{},"#, step));
        }
        
        // Add entry type
        let entry_type_str = match entry.entry_type {
            TraceEntryType::Step => "step",
            TraceEntryType::Milestone => "milestone",
            TraceEntryType::Error => "error",
        };
        json.push_str(&format!(r#""type":"{}","#, entry_type_str));
        
        // Add location
        json.push_str(&format!(r#""location":"{}","#, escape_json(&entry.location)));
        
        // Add message
        json.push_str(&format!(r#""message":"{}","#, escape_json(&entry.message)));
        
        // Add timestamp
        let timestamp = format_timestamp(&entry.timestamp);
        json.push_str(&format!(r#""timestamp":"{}","#, timestamp));
        
        // Add thread ID if present
        if let Some(thread_id) = entry.thread_id {
            json.push_str(&format!(r#""thread_id":"{:?}","#, thread_id));
        }
        
        // Add metadata if present
        if let Some(metadata) = &entry.metadata {
            json.push_str(r#""metadata":{"#);
            let mut first = true;
            
            if let Some(block_number) = metadata.block_number {
                json.push_str(&format!(r#""block_number":{}"#, block_number));
                first = false;
            }
            
            if let Some(block_hash) = &metadata.block_hash {
                if !first { json.push(','); }
                json.push_str(&format!(r#""block_hash":"{}""#, escape_json(block_hash)));
                first = false;
            }
            
            if let Some(author) = &metadata.author {
                if !first { json.push(','); }
                json.push_str(&format!(r#""author":"{}""#, escape_json(author)));
                first = false;
            }
            
            if let Some(extrinsic_count) = metadata.extrinsic_count {
                if !first { json.push(','); }
                json.push_str(&format!(r#""extrinsic_count":{}"#, extrinsic_count));
                first = false;
            }
            
            for (key, value) in &metadata.custom {
                if !first { json.push(','); }
                json.push_str(&format!(r#""{}":"{}""#, escape_json(key), escape_json(value)));
                first = false;
            }
            
            json.push_str("},");
        }
        
        // Remove trailing comma if present
        if json.ends_with(',') {
            json.pop();
        }
        
        json.push('}');
        json
    }
}

/// Human-readable formatter for console output
pub struct HumanReadableFormatter;

impl TraceFormatter for HumanReadableFormatter {
    fn format(&self, entry: &TraceEntry) -> String {
        let timestamp = format_timestamp(&entry.timestamp);
        let thread_info = entry.thread_id
            .map(|id| format!("[{:?}] ", id))
            .unwrap_or_default();
        
        match entry.entry_type {
            TraceEntryType::Step => {
                let step = entry.step.map(|s| s.to_string()).unwrap_or_else(|| "?".to_string());
                format!(
                    "[{}] {}============== [CBC-TRACE] {}. [{}] {} ==============",
                    timestamp, thread_info, step, entry.location, entry.message
                )
            }
            TraceEntryType::Milestone => {
                format!(
                    "[{}] {}🎯 MILESTONE: {}",
                    timestamp, thread_info, entry.message
                )
            }
            TraceEntryType::Error => {
                let step = entry.step.map(|s| s.to_string()).unwrap_or_else(|| "?".to_string());
                format!(
                    "[{}] {}❌ ERROR at STEP {}: [{}] {}",
                    timestamp, thread_info, step, entry.location, entry.message
                )
            }
        }
    }
}

/// Format a SystemTime as ISO 8601 timestamp
fn format_timestamp(time: &SystemTime) -> String {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs();
            let nanos = duration.subsec_nanos();
            let millis = nanos / 1_000_000;
            
            // Convert to datetime components
            let days_since_epoch = secs / 86400;
            let secs_today = secs % 86400;
            let hours = secs_today / 3600;
            let minutes = (secs_today % 3600) / 60;
            let seconds = secs_today % 60;
            
            // Simple date calculation (approximate, good enough for logging)
            let year = 1970 + (days_since_epoch / 365);
            let day_of_year = days_since_epoch % 365;
            let month = (day_of_year / 30) + 1;
            let day = (day_of_year % 30) + 1;
            
            format!(
                "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
                year, month, day, hours, minutes, seconds, millis
            )
        }
        Err(_) => "INVALID_TIME".to_string(),
    }
}

/// Escape special characters for JSON strings
fn escape_json(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            '\n' => vec!['\\', 'n'],
            '\r' => vec!['\\', 'r'],
            '\t' => vec!['\\', 't'],
            c if c.is_control() => format!("\\u{:04x}", c as u32).chars().collect(),
            c => vec![c],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle_tracer::entry::TraceMetadata;

    #[test]
    fn test_json_formatter_basic() {
        let formatter = JsonFormatter;
        let entry = TraceEntry {
            step: Some(1),
            location: "test.rs".to_string(),
            message: "Test message".to_string(),
            timestamp: SystemTime::now(),
            thread_id: None,
            metadata: None,
            entry_type: TraceEntryType::Step,
        };
        
        let formatted = formatter.format(&entry);
        assert!(formatted.contains(r#""step":1"#));
        assert!(formatted.contains(r#""type":"step""#));
        assert!(formatted.contains(r#""location":"test.rs""#));
        assert!(formatted.contains(r#""message":"Test message""#));
    }

    #[test]
    fn test_json_formatter_with_metadata() {
        let formatter = JsonFormatter;
        let mut metadata = TraceMetadata::new();
        metadata.block_number = Some(42);
        metadata.author = Some("Alice".to_string());
        
        let entry = TraceEntry {
            step: Some(10),
            location: "block.rs".to_string(),
            message: "Block created".to_string(),
            timestamp: SystemTime::now(),
            thread_id: None,
            metadata: Some(metadata),
            entry_type: TraceEntryType::Step,
        };
        
        let formatted = formatter.format(&entry);
        assert!(formatted.contains(r#""block_number":42"#));
        assert!(formatted.contains(r#""author":"Alice""#));
    }

    #[test]
    fn test_human_readable_formatter() {
        let formatter = HumanReadableFormatter;
        let entry = TraceEntry {
            step: Some(5),
            location: "service.rs".to_string(),
            message: "Service initialized".to_string(),
            timestamp: SystemTime::now(),
            thread_id: None,
            metadata: None,
            entry_type: TraceEntryType::Step,
        };
        
        let formatted = formatter.format(&entry);
        assert!(formatted.contains("[CBC-TRACE]"));
        assert!(formatted.contains("5."));
        assert!(formatted.contains("[service.rs]"));
        assert!(formatted.contains("Service initialized"));
    }

    #[test]
    fn test_milestone_formatter() {
        let formatter = HumanReadableFormatter;
        let entry = TraceEntry {
            step: None,
            location: "milestone".to_string(),
            message: "First block produced".to_string(),
            timestamp: SystemTime::now(),
            thread_id: None,
            metadata: None,
            entry_type: TraceEntryType::Milestone,
        };
        
        let formatted = formatter.format(&entry);
        assert!(formatted.contains("MILESTONE"));
        assert!(formatted.contains("First block produced"));
    }

    #[test]
    fn test_error_formatter() {
        let formatter = HumanReadableFormatter;
        let entry = TraceEntry {
            step: Some(15),
            location: "consensus.rs".to_string(),
            message: "Failed to produce block".to_string(),
            timestamp: SystemTime::now(),
            thread_id: None,
            metadata: None,
            entry_type: TraceEntryType::Error,
        };
        
        let formatted = formatter.format(&entry);
        assert!(formatted.contains("ERROR"));
        assert!(formatted.contains("STEP 15"));
        assert!(formatted.contains("Failed to produce block"));
    }

    #[test]
    fn test_json_escape() {
        let input = r#"Test "quoted" and \backslash\ and newline
"#;
        let escaped = escape_json(input);
        assert!(escaped.contains(r#"\"quoted\""#));
        assert!(escaped.contains(r#"\\backslash\\"#));
        assert!(escaped.contains(r#"\n"#));
    }
}
