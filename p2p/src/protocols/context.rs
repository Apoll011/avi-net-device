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

    /// Merge another vector clock into this one by taking the maximum of each component
    pub fn merge(&mut self, other: &Self) {
        for (actor, &counter) in &other.0 {
            let entry = self.0.entry(actor.clone()).or_insert(0);
            if counter > *entry {
                *entry = counter;
            }
        }
    }
}

/// The Core Context Object
/// Designed to be flexible ("dict-like") using serde_json::Value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
            "avi":  {
                "device": {},
            }
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

    /// Merge another context into this one
    /// Returns true if the context was updated
    pub fn merge(&mut self, other: AviContext) -> bool {
        let cmp = self.vector_clock.partial_cmp(&other.vector_clock);

        match cmp {
            Some(Ordering::Less) => {
                // Other is strictly newer, but we still want to keep our unique data if any
                // Wait, if it's strictly newer, it means it's an update to our state.
                // However, the user said "if the new one has some data the old one dont have the old on add the missing data to jis"
                // This suggests we should always deep merge.
                let mut updated = false;
                if self.data != other.data {
                    deep_merge(&mut self.data, other.data, false); // prefer other
                    updated = true;
                }
                self.timestamp = other.timestamp;
                self.vector_clock.merge(&other.vector_clock);
                updated
            }
            Some(Ordering::Greater) => {
                // We are strictly newer. Other might have some missing data?
                // "if the new one has some data the old one dont have the old on add the missing data to jis"
                // Even if we are newer, we should take missing keys from other.
                if self.data != other.data {
                    deep_merge(&mut self.data, other.data, true); // prefer self
                    // No need to update timestamp/vector_clock as we are already newer
                    true
                } else {
                    false
                }
            }
            Some(Ordering::Equal) => false,
            None => {
                // Concurrent update, use "oldest wins" tie-breaker for data conflicts
                let prefer_self = self.timestamp <= other.timestamp;
                let mut updated = false;

                if self.data != other.data {
                    deep_merge(&mut self.data, other.data, prefer_self);
                    updated = true;
                }

                if !prefer_self {
                    self.timestamp = other.timestamp;
                }
                self.vector_clock.merge(&other.vector_clock);
                updated
            }
        }
    }
}

fn deep_merge(a: &mut serde_json::Value, b: serde_json::Value, prefer_a: bool) {
    match (a, b) {
        (serde_json::Value::Object(a_obj), serde_json::Value::Object(b_obj)) => {
            for (k, v) in b_obj {
                if let Some(a_val) = a_obj.get_mut(&k) {
                    deep_merge(a_val, v, prefer_a);
                } else {
                    a_obj.insert(k, v);
                }
            }
        }
        (a_val, b_val) => {
            if !prefer_a {
                *a_val = b_val;
            }
        }
    }
}

fn merge_json(a: &mut serde_json::Value, b: serde_json::Value) {
    deep_merge(a, b, false);
}