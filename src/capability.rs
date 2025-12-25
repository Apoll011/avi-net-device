use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    pub compute: Option<ComputeCapability>,
    pub sensors: HashMap<String, SensorCapability>,
    pub connectivity: HashMap<String, ConnectivityCapability>,
    pub power: Option<PowerCapability>,
    pub health: Option<HealthCapability>,
    pub display: Option<DisplayCapability>,
    pub audio: Option<AudioCapability>,
    pub extended: HashMap<String, ExtendedCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeCapability {
    pub cpu: CpuInfo,
    pub gpu: Option<GpuInfo>,
    pub npu: Option<NpuInfo>,
    pub ram: RamInfo,
    pub storage: StorageInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub cores: u8,
    pub architecture: String,
    pub features: Vec<String>,
    pub nominal_mhz: u32,
    pub current_mhz: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub present: bool,
    #[serde(rename = "type")]
    pub gpu_type: String,
    pub tflops: f32,
    pub memory_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpuInfo {
    pub present: bool,
    pub tops: f32,
    pub supported_precisions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RamInfo {
    pub total_mb: u32,
    pub free_mb: u32,
    #[serde(rename = "type")]
    pub ram_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageInfo {
    pub total_gb: u32,
    pub free_gb: u32,
    #[serde(rename = "type")]
    pub storage_type: String,
    pub throughput_mbps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SensorCapability {
    Microphone {
        present: bool,
        array_size: u8,
        sampling_rate_khz: u32,
        max_spl_db: u8,
    },
    Camera {
        present: bool,
        resolution_mp: u8,
        fov_degrees: u16,
        features: Vec<String>,
    },
    Temperature {
        present: bool,
        accuracy_celsius: f32,
        current_value: Option<f32>,
    },
    Imu {
        present: bool,
        axes: u8,
        update_rate_hz: u32,
    },
    Lidar {
        present: bool,
        points_per_second: u32,
        max_range_m: u8,
    },
    Proximity {
        present: bool,
        max_range_cm: u16,
        accuracy_cm: f32,
    },
    AmbientLight {
        present: bool,
        max_lux: u32,
        current_value: Option<f32>,
    },
    Pressure {
        present: bool,
        range_hpa: (f32, f32),
        accuracy_hpa: f32,
    },
    Humidity {
        present: bool,
        accuracy_pct: f32,
        current_value: Option<f32>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConnectivityCapability {
    Wifi {
        present: bool,
        standards: Vec<String>,
        bands: Vec<String>,
        mimo_streams: u8,
        rssi_dbm: Option<i8>,
    },
    Bluetooth {
        present: bool,
        version: String,
        features: Vec<String>,
        rssi_dbm: Option<i8>,
    },
    Ethernet {
        present: bool,
        speed_mbps: u32,
        full_duplex: bool,
    },
    Cellular {
        present: bool,
        technology: String,
        signal_strength_dbm: Option<i8>,
        carrier: String,
    },
    Uwb {
        present: bool,
        ranging_supported: bool,
        max_range_m: u8,
    },
    Zigbee {
        present: bool,
        protocol_version: String,
        network_role: String,
    },
    Thread {
        present: bool,
        network_role: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerCapability {
    pub source: PowerSource,
    pub battery_pct: Option<u8>,
    pub estimated_runtime_mins: Option<u32>,
    pub charging: bool,
    pub max_power_draw_w: f32,
    pub current_power_draw_w: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PowerSource {
    Battery,
    Wired,
    Solar,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCapability {
    pub uptime_s: u64,
    pub thermal_headroom_pct: u8,
    pub cpu_load_pct: u8,
    pub memory_pressure_pct: u8,
    pub network_stability_pct: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayCapability {
    pub present: bool,
    pub resolution: (u16, u16),
    pub refresh_rate_hz: u8,
    pub touch_capable: bool,
    pub brightness_nits: u16,
    pub current_brightness_pct: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCapability {
    pub output: Option<AudioOutput>,
    pub spatial_audio: bool,
    pub formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioOutput {
    pub present: bool,
    pub channels: u8,
    pub max_spl_db: u8,
    pub frequency_response: (u32, u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExtendedCapability {
    Boolean(bool),
    Number(f64),
    Text(String),
    List(Vec<String>),
    Object(HashMap<String, serde_json::Value>),
}

pub struct CapabilityBuilder {
    compute: Option<ComputeCapability>,
    sensors: HashMap<String, SensorCapability>,
    connectivity: HashMap<String, ConnectivityCapability>,
    power: Option<PowerCapability>,
    health: Option<HealthCapability>,
    display: Option<DisplayCapability>,
    audio: Option<AudioCapability>,
    extended: HashMap<String, ExtendedCapability>,
}

impl CapabilityBuilder {
    pub fn new() -> Self {
        Self {
            compute: None,
            sensors: HashMap::new(),
            connectivity: HashMap::new(),
            power: None,
            health: None,
            display: None,
            audio: None,
            extended: HashMap::new(),
        }
    }

    pub fn compute(mut self, compute: ComputeCapability) -> Self {
        self.compute = Some(compute);
        self
    }

    pub fn sensor(mut self, name: impl Into<String>, sensor: SensorCapability) -> Self {
        self.sensors.insert(name.into(), sensor);
        self
    }

    pub fn connectivity(mut self, name: impl Into<String>, conn: ConnectivityCapability) -> Self {
        self.connectivity.insert(name.into(), conn);
        self
    }

    pub fn power(mut self, power: PowerCapability) -> Self {
        self.power = Some(power);
        self
    }

    pub fn health(mut self, health: HealthCapability) -> Self {
        self.health = Some(health);
        self
    }

    pub fn display(mut self, display: DisplayCapability) -> Self {
        self.display = Some(display);
        self
    }

    pub fn audio(mut self, audio: AudioCapability) -> Self {
        self.audio = Some(audio);
        self
    }

    pub fn extended(mut self, name: impl Into<String>, value: ExtendedCapability) -> Self {
        self.extended.insert(name.into(), value);
        self
    }

    pub fn build(self) -> DeviceCapabilities {
        DeviceCapabilities {
            compute: self.compute,
            sensors: self.sensors,
            connectivity: self.connectivity,
            power: self.power,
            health: self.health,
            display: self.display,
            audio: self.audio,
            extended: self.extended,
        }
    }
}

impl Default for CapabilityBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_builder() {
        let caps = CapabilityBuilder::new()
            .compute(ComputeCapability {
                cpu: CpuInfo {
                    cores: 8,
                    architecture: "arm64".to_string(),
                    features: vec!["neon".to_string(), "sve".to_string()],
                    nominal_mhz: 2400,
                    current_mhz: 1800,
                },
                gpu: Some(GpuInfo {
                    present: true,
                    gpu_type: "integrated".to_string(),
                    tflops: 1.2,
                    memory_mb: 2048,
                }),
                npu: Some(NpuInfo {
                    present: true,
                    tops: 5.0,
                    supported_precisions: vec!["int8".to_string(), "fp16".to_string()],
                }),
                ram: RamInfo {
                    total_mb: 8192,
                    free_mb: 4096,
                    ram_type: "lpddr5".to_string(),
                },
                storage: StorageInfo {
                    total_gb: 128,
                    free_gb: 75,
                    storage_type: "ufs".to_string(),
                    throughput_mbps: 800,
                },
            })
            .sensor("microphone", SensorCapability::Microphone {
                present: true,
                array_size: 4,
                sampling_rate_khz: 48,
                max_spl_db: 120,
            })
            .sensor("temperature", SensorCapability::Temperature {
                present: true,
                accuracy_celsius: 0.5,
                current_value: Some(22.5),
            })
            .connectivity("wifi", ConnectivityCapability::Wifi {
                present: true,
                standards: vec!["802.11ax".to_string(), "802.11ac".to_string()],
                bands: vec!["2.4".to_string(), "5".to_string()],
                mimo_streams: 2,
                rssi_dbm: Some(-65),
            })
            .power(PowerCapability {
                source: PowerSource::Battery,
                battery_pct: Some(85),
                estimated_runtime_mins: Some(480),
                charging: false,
                max_power_draw_w: 15.0,
                current_power_draw_w: 5.2,
            })
            .health(HealthCapability {
                uptime_s: 7200,
                thermal_headroom_pct: 70,
                cpu_load_pct: 25,
                memory_pressure_pct: 40,
                network_stability_pct: 98,
            })
            .extended("device_orientation", ExtendedCapability::Text("landscape".to_string()))
            .extended("attention_detection", ExtendedCapability::Boolean(true))
            .build();

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&caps).unwrap();
        println!("{}", json);

        // Deserialize back
        let deserialized: DeviceCapabilities = serde_json::from_str(&json).unwrap();
        assert!(deserialized.compute.is_some());
    }
}