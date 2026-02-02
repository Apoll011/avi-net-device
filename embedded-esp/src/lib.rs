#![no_std]

extern crate alloc;
use alloc::vec::Vec;

use esp_idf_svc::sys as esp_idf_sys;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi, ClientConfiguration, Configuration};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::hal::prelude::Peripherals;
use embedded_svc::wifi::{AuthMethod, Wifi};

use lwip_sys::*;
use core::ptr;
use core::mem;

use avi_p2p_embedded::{UdpClient, AviEmbedded, AviEmbeddedConfig, MessageHandler};

pub struct EspUdpSocket {
    sock_fd: i32,
    target_addr: sockaddr_in,
}

impl EspUdpSocket {
    pub fn new(gateway_ip: [u8; 4], gateway_port: u16) -> Result<Self, &'static str> {
        unsafe {
            let sock_fd = lwip_socket(AF_INET as i32, SOCK_DGRAM as i32, 0);
            if sock_fd < 0 {
                return Err("Failed to create socket");
            }

            // Set non-blocking
            let flags = lwip_fcntl(sock_fd, F_GETFL, 0);
            lwip_fcntl(sock_fd, F_SETFL, flags | O_NONBLOCK);

            let mut target_addr: sockaddr_in = mem::zeroed();
            target_addr.sin_family = AF_INET as u8;
            target_addr.sin_port = gateway_port.to_be();
            target_addr.sin_addr.s_addr = u32::from_ne_bytes(gateway_ip);

            Ok(Self {
                sock_fd,
                target_addr,
            })
        }
    }
}

impl Drop for EspUdpSocket {
    fn drop(&mut self) {
        unsafe {
            lwip_close(self.sock_fd);
        }
    }
}

impl UdpClient for EspUdpSocket {
    type Error = &'static str;

    async fn send(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        unsafe {
            let sent = lwip_sendto(
                self.sock_fd,
                buf.as_ptr() as *const _,
                buf.len(),
                0,
                &self.target_addr as *const _ as *const sockaddr,
                mem::size_of::<sockaddr_in>() as u32,
            );

            if sent < 0 {
                Err("Send failed")
            } else {
                Ok(())
            }
        }
    }

    async fn receive(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        unsafe {
            let mut from_addr: sockaddr_in = mem::zeroed();
            let mut from_len = mem::size_of::<sockaddr_in>() as u32;

            let received = lwip_recvfrom(
                self.sock_fd,
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                MSG_DONTWAIT,
                &mut from_addr as *mut _ as *mut sockaddr,
                &mut from_len,
            );

            if received < 0 {
                let errno = *__errno();
                if errno == EWOULDBLOCK || errno == EAGAIN {
                    // No data available, sleep briefly
                    esp_idf_sys::vTaskDelay(10);
                    return Err("No data");
                }
                Err("Receive failed")
            } else {
                Ok(received as usize)
            }
        }
    }
}

pub struct WifiConfig<'a> {
    pub ssid: &'a str,
    pub password: &'a str,
}

pub fn init_wifi(config: WifiConfig) -> Result<(), esp_idf_svc::sys::EspError> {
    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;

    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: config.ssid.into(),
        password: config.password.into(),
        auth_method: AuthMethod::WPA2Personal,
        ..Default::default()
    }))?;

    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;

    Ok(())
}

// Example usage with a simple message handler
pub struct SimpleHandler;

impl MessageHandler for SimpleHandler {
    fn on_message(&mut self, topic: &str, data: &[u8]) {
        esp_println::println!("📨 Received on topic '{}': {} bytes", topic, data.len());
        
        // Try to parse as string if possible
        if let Ok(msg) = core::str::from_utf8(data) {
            esp_println::println!("   Content: {}", msg);
        }
    }
}

// Helper module for common operations
pub mod helpers {
    use super::*;
    use avi_p2p_protocol::{PressType, SensorValue};
    
    pub async fn report_button<S: UdpClient, H: MessageHandler>(
        device: &mut AviEmbedded<'_, S, H>,
        button_id: u8,
        press_type: PressType,
    ) -> Result<(), S::Error> {
        device.button_pressed(button_id, press_type).await
    }
    
    pub async fn report_temperature<S: UdpClient, H: MessageHandler>(
        device: &mut AviEmbedded<'_, S, H>,
        sensor_name: &str,
        celsius: f32,
    ) -> Result<(), S::Error> {
        device.update_sensor(sensor_name, SensorValue::Temperature(celsius)).await
    }
    
    pub async fn report_humidity<S: UdpClient, H: MessageHandler>(
        device: &mut AviEmbedded<'_, S, H>,
        sensor_name: &str,
        percent: f32,
    ) -> Result<(), S::Error> {
        device.update_sensor(sensor_name, SensorValue::Humidity(percent)).await
    }
}
