/* Usage:
let query = DeviceQuery::all()
    .compute(|c| c.cpu.cores >= 8 && c.ram.free_mb > 4096)
    .connectivity("wifi", |conn| {
        matches!(conn, ConnectivityCapability::Wifi { rssi_dbm: Some(rssi), .. } if *rssi > -50)
    });
let result = device.execute_query(query);

// Find devices with microphone OR camera
let query = DeviceQuery::any()
    .sensor("microphone", |_| true)
    .sensor("camera", |_| true);
let av_devices = device.execute_query(query);

// Complex query: powerful devices with low battery
let query = DeviceQuery::all()
    .compute(|c| c.cpu.cores >= 4)
    .power(|p| p.battery_pct.map(|pct| pct < 30).unwrap_or(false))
    .health(|h| h.thermal_headroom_pct > 50);
let critical_devices = device.execute_query(query);
*/
use std::collections::HashMap;
use crate::capability::{AudioCapability, ComputeCapability, ConnectivityCapability, DisplayCapability, HealthCapability, PowerCapability, SensorCapability};
use crate::DeviceCapabilities;

#[derive(Debug, Clone)]
pub enum CapabilityType {
    Compute,
    Health,
    Power,
    Display,
    Audio,
    Sensor(String),
    Connectivity(String),
    Extended(String),
    Any,
}

pub struct CapabilityFilter {
    capability_type: CapabilityType,
    predicate: Box<dyn Fn(&DeviceCapabilities) -> bool>,
}

impl CapabilityFilter {
    pub fn new<F>(capability_type: CapabilityType, predicate: F) -> Self
    where
        F: Fn(&DeviceCapabilities) -> bool + 'static,
    {
        Self {
            capability_type,
            predicate: Box::new(predicate),
        }
    }

    pub fn compute<F>(predicate: F) -> Self
    where
        F: Fn(&ComputeCapability) -> bool + 'static,
    {
        Self::new(CapabilityType::Compute, move |caps| {
            caps.compute
                .as_ref()
                .map(|c| predicate(c))
                .unwrap_or(false)
        })
    }

    pub fn health<F>(predicate: F) -> Self
    where
        F: Fn(&HealthCapability) -> bool + 'static,
    {
        Self::new(CapabilityType::Health, move |caps| {
            caps.health
                .as_ref()
                .map(|h| predicate(h))
                .unwrap_or(false)
        })
    }

    pub fn power<F>(predicate: F) -> Self
    where
        F: Fn(&PowerCapability) -> bool + 'static,
    {
        Self::new(CapabilityType::Power, move |caps| {
            caps.power
                .as_ref()
                .map(|p| predicate(p))
                .unwrap_or(false)
        })
    }

    pub fn display<F>(predicate: F) -> Self
    where
        F: Fn(&DisplayCapability) -> bool + 'static,
    {
        Self::new(CapabilityType::Display, move |caps| {
            caps.display
                .as_ref()
                .map(|d| predicate(d))
                .unwrap_or(false)
        })
    }

    pub fn audio<F>(predicate: F) -> Self
    where
        F: Fn(&AudioCapability) -> bool + 'static,
    {
        Self::new(CapabilityType::Audio, move |caps| {
            caps.audio
                .as_ref()
                .map(|a| predicate(a))
                .unwrap_or(false)
        })
    }

    pub fn sensor<F>(sensor_name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&SensorCapability) -> bool + 'static,
    {
        let name = sensor_name.into();
        Self::new(CapabilityType::Sensor(name.clone()), move |caps| {
            caps.sensors
                .get(&name)
                .map(|s| predicate(s))
                .unwrap_or(false)
        })
    }

    pub fn connectivity<F>(conn_name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&ConnectivityCapability) -> bool + 'static,
    {
        let name = conn_name.into();
        Self::new(CapabilityType::Connectivity(name.clone()), move |caps| {
            caps.connectivity
                .get(&name)
                .map(|c| predicate(c))
                .unwrap_or(false)
        })
    }

    pub fn apply(&self, devices: &HashMap<String, DeviceCapabilities>) -> Vec<String> {
        devices
            .iter()
            .filter(|(_, caps)| self.check_capability(caps))
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn check_capability(&self, caps: &DeviceCapabilities) -> bool {
        let has_capability = match &self.capability_type {
            CapabilityType::Compute => caps.compute.is_some(),
            CapabilityType::Health => caps.health.is_some(),
            CapabilityType::Power => caps.power.is_some(),
            CapabilityType::Display => caps.display.is_some(),
            CapabilityType::Audio => caps.audio.is_some(),
            CapabilityType::Sensor(name) => caps.sensors.contains_key(name),
            CapabilityType::Connectivity(name) => caps.connectivity.contains_key(name),
            CapabilityType::Extended(key) => caps.extended.contains_key(key),
            CapabilityType::Any => true,
        };

        has_capability && (self.predicate)(caps)
    }
}

pub struct DeviceQuery {
    filters: Vec<CapabilityFilter>,
    combine_mode: CombineMode,
}

#[derive(Debug, Clone, Copy)]
pub enum CombineMode {
    All,
    Any,
}

impl DeviceQuery {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
            combine_mode: CombineMode::All,
        }
    }

    pub fn all() -> Self {
        Self {
            filters: Vec::new(),
            combine_mode: CombineMode::All,
        }
    }

    pub fn any() -> Self {
        Self {
            filters: Vec::new(),
            combine_mode: CombineMode::Any,
        }
    }

    pub fn filter(mut self, filter: CapabilityFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn compute<F>(self, predicate: F) -> Self
    where
        F: Fn(&ComputeCapability) -> bool + 'static,
    {
        self.filter(CapabilityFilter::compute(predicate))
    }

    pub fn health<F>(self, predicate: F) -> Self
    where
        F: Fn(&HealthCapability) -> bool + 'static,
    {
        self.filter(CapabilityFilter::health(predicate))
    }

    pub fn power<F>(self, predicate: F) -> Self
    where
        F: Fn(&PowerCapability) -> bool + 'static,
    {
        self.filter(CapabilityFilter::power(predicate))
    }

    pub fn sensor<F>(self, sensor_name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&SensorCapability) -> bool + 'static,
    {
        self.filter(CapabilityFilter::sensor(sensor_name, predicate))
    }

    pub fn connectivity<F>(self, conn_name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&ConnectivityCapability) -> bool + 'static,
    {
        self.filter(CapabilityFilter::connectivity(conn_name, predicate))
    }

    pub fn execute(&self, devices: &HashMap<String, DeviceCapabilities>) -> Vec<String> {
        devices
            .iter()
            .filter(|(_, caps)| self.matches(caps))
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn matches(&self, caps: &DeviceCapabilities) -> bool {
        if self.filters.is_empty() {
            return true;
        }

        match self.combine_mode {
            CombineMode::All => {
                self.filters.iter().all(|f| f.check_capability(caps))
            }
            CombineMode::Any => {
                self.filters.iter().any(|f| f.check_capability(caps))
            }
        }
    }
}