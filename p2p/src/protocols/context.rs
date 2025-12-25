use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cmp::Ordering;

/// Logical timestamp for causal ordering
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VectorClock(pub HashMap<String, u64>);

impl VectorClock {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Increment the clock for a specific actor (usually local peer ID)
    pub fn increment(&mut self, actor: &str) {
        let counter = self.0.entry(actor.to_string()).or_insert(0);
        *counter += 1;
    }

    /// Compare two vector clocks to determine order
    /// Returns:
    /// - Less: self happened before other
    /// - Greater: self happened after other
    /// - Equal: states are identical
    /// - None: concurrent updates (conflict)
    pub fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let mut self_has_greater = false;
        let mut other_has_greater = false;

        // Get union of all keys
        let mut all_keys: Vec<&String> = self.0.keys().chain(other.0.keys()).collect();
        all_keys.sort();
        all_keys.dedup();

        for key in all_keys {
            let v1 = self.0.get(key).unwrap_or(&0);
            let v2 = other.0.get(key).unwrap_or(&0);

            if v1 > v2 {
                self_has_greater = true;
            } else if v2 > v1 {
                other_has_greater = true;
            }
        }

        if self_has_greater && other_has_greater {
            None // Concurrent
        } else if self_has_greater {
            Some(Ordering::Greater)
        } else if other_has_greater {
            Some(Ordering::Less)
        } else {
            Some(Ordering::Equal)
        }
    }
}

/// The Core Context Object
/// Designed to be flexible ("dict-like") using serde_json::Value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AviContext {
    pub device_id: String,
    pub timestamp: u64, // Unix Timestamp
    pub vector_clock: VectorClock,
    pub data: serde_json::Value, // The "dict" (device, user, task, environment)
}

impl AviContext {
    pub fn new(device_id: String) -> Self {
        // Initialize with empty skeleton based on your schema
        let data = serde_json::json!({
            "environment": {},
            "device": {},
            "user": {},
            "task": {}
        });

        Self {
            device_id,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            vector_clock: VectorClock::new(),
            data,
        }
    }


    pub fn apply_patch(&mut self, patch: serde_json::Value) {
        merge_json(&mut self.data, patch);
        // Update timestamp on change
        self.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

fn merge_json(a: &mut serde_json::Value, b: serde_json::Value) {
    match (a, b) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                merge_json(a.entry(k).or_insert(serde_json::Value::Null), v);
            }
        }
        (a, b) => *a = b,
    }
}