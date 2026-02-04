use crate::{set_nested_value, AviEvent, AviP2pHandle, PeerId, StreamId};
use avi_p2p_protocol::{DownlinkMessage, UplinkMessage, MAX_PACKET_SIZE};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

pub struct BridgeConfig {
    pub udp_port: u16,
}

struct DeviceSession {
    pub device_id: u64,
    pub active_streams: HashMap<u8, StreamId>,
    pub subscriptions: HashSet<String>,
}

pub struct EmbeddedBridge {
    #[allow(dead_code)]
    socket: Arc<UdpSocket>,
    #[allow(dead_code)]
    handle: AviP2pHandle,

    #[allow(dead_code)]
    sessions: Arc<Mutex<HashMap<SocketAddr, DeviceSession>>>,
}

impl EmbeddedBridge {
    pub async fn start(handle: AviP2pHandle, config: BridgeConfig) -> Result<(), String> {
        let addr = format!("0.0.0.0:{}", config.udp_port);
        let socket = UdpSocket::bind(&addr).await.map_err(|e| e.to_string())?;
        let socket = Arc::new(socket);

        let sessions = Arc::new(Mutex::new(HashMap::new()));

        println!("Embedded Bridge Listening on UDP {}", config.udp_port);

        // Spawn uplink handler (embedded -> gateway)
        let uplink_socket = socket.clone();
        let uplink_handle = handle.clone();
        let uplink_sessions = sessions.clone();

        tokio::spawn(async move {
            let mut buf = [0u8; MAX_PACKET_SIZE];

            loop {
                let (len, remote_addr) = match uplink_socket.recv_from(&mut buf).await {
                    Ok(res) => res,
                    Err(_) => continue,
                };

                let packet: Result<UplinkMessage, _> = postcard::from_bytes(&buf[..len]);

                if let Ok(msg) = packet {
                    Self::handle_uplink_packet(
                        msg,
                        remote_addr,
                        uplink_socket.clone(),
                        uplink_handle.clone(),
                        uplink_sessions.clone(),
                    )
                    .await;
                }
            }
        });

        // Spawn downlink handler (gateway -> embedded)
        let downlink_socket = socket.clone();
        let downlink_sessions = sessions.clone();
        let mut event_rx = handle.subscribe_events().await.map_err(|e| e.to_string())?;

        tokio::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                Self::handle_downlink_event(
                    event,
                    downlink_socket.clone(),
                    downlink_sessions.clone(),
                )
                .await;
            }
        });

        Ok(())
    }

    async fn handle_uplink_packet(
        msg: UplinkMessage<'_>,
        addr: SocketAddr,
        socket: Arc<UdpSocket>,
        handle: AviP2pHandle,
        sessions: Arc<Mutex<HashMap<SocketAddr, DeviceSession>>>,
    ) {
        let mut sessions_lock = sessions.lock().await;

        match msg {
            UplinkMessage::Hello { device_id } => {
                sessions_lock.insert(
                    addr,
                    DeviceSession {
                        device_id,
                        active_streams: HashMap::new(),
                        subscriptions: HashSet::new(),
                    },
                );

                let welcome = DownlinkMessage::Welcome;
                let mut tx_buf = [0u8; 64];
                if let Ok(data) = postcard::to_slice(&welcome, &mut tx_buf) {
                    let _ = socket.send_to(data, addr).await;
                }

                println!("✅ Device {} connected from {}", device_id, addr);
            }

            UplinkMessage::Subscribe { topic } => {
                if let Some(session) = sessions_lock.get_mut(&addr) {
                    println!("📥 Device {} subscribing to: {}", session.device_id, topic);

                    // Subscribe on the P2P mesh
                    if let Ok(_) = handle.subscribe(topic).await {
                        session.subscriptions.insert(topic.to_string());

                        // Send acknowledgment
                        let ack = DownlinkMessage::SubscribeAck { topic };
                        let mut tx_buf = [0u8; 256];
                        if let Ok(data) = postcard::to_slice(&ack, &mut tx_buf) {
                            let _ = socket.send_to(data, addr).await;
                        }
                    }
                }
            }

            UplinkMessage::Unsubscribe { topic } => {
                if let Some(session) = sessions_lock.get_mut(&addr) {
                    println!(
                        "📤 Device {} unsubscribing from: {}",
                        session.device_id, topic
                    );

                    session.subscriptions.remove(topic);
                    let _ = handle.unsubscribe(topic).await;

                    // Send acknowledgment
                    let ack = DownlinkMessage::UnsubscribeAck { topic };
                    let mut tx_buf = [0u8; 256];
                    if let Ok(data) = postcard::to_slice(&ack, &mut tx_buf) {
                        let _ = socket.send_to(data, addr).await;
                    }
                }
            }

            UplinkMessage::Publish { topic, data } => {
                if let Some(_session) = sessions_lock.get(&addr) {
                    let _ = handle.publish(topic, data.to_vec()).await;
                }
            }

            UplinkMessage::StreamStart {
                local_stream_id,
                target_peer_id,
                reason,
            } => {
                if let Some(session) = sessions_lock.get_mut(&addr) {
                    if target_peer_id.is_empty() {
                        println!("Device requested stream with no target.");
                        return;
                    }

                    let peer_id = PeerId::new(target_peer_id);
                    println!(
                        "Bridging Stream {} -> Mesh Peer {}",
                        local_stream_id, peer_id
                    );

                    match handle.request_stream(peer_id, reason.to_string()).await {
                        Ok(mesh_stream_id) => {
                            session
                                .active_streams
                                .insert(local_stream_id, mesh_stream_id);
                        }
                        Err(e) => eprintln!("Bridge Failed to open mesh stream: {}", e),
                    }
                }
            }

            UplinkMessage::StreamData {
                local_stream_id,
                data,
            } => {
                if let Some(session) = sessions_lock.get(&addr) {
                    if let Some(mesh_id) = session.active_streams.get(&local_stream_id) {
                        let _ = handle.send_stream_data(*mesh_id, data.to_vec()).await;
                    }
                }
            }

            UplinkMessage::StreamClose { local_stream_id } => {
                if let Some(session) = sessions_lock.get_mut(&addr) {
                    if let Some(mesh_id) = session.active_streams.remove(&local_stream_id) {
                        let _ = handle.close_stream(mesh_id).await;
                    }
                }
            }

            UplinkMessage::ButtonPress {
                button_id,
                press_type,
                custom_data,
            } => {
                if let Some(session) = sessions_lock.get(&addr) {
                    let dev_id = session.device_id;

                    let topic = format!("device/{}/button", dev_id);

                    let payload = json!({
                        "button_id": button_id,
                        "type": format!("{:?}", press_type),
                        "data": custom_data.to_string(),
                        "ts": std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default().as_secs()
                    });

                    let _ = handle
                        .publish(&topic, serde_json::to_vec(&payload).unwrap())
                        .await;
                }
            }

            UplinkMessage::SensorUpdate {
                sensor_name,
                data,
                custom_data,
            } => {
                if let Some(session) = sessions_lock.get(&addr) {
                    let dev_id = session.device_id;

                    let topic = format!("device/{}/sensor/{}", dev_id, sensor_name);

                    let val = match data {
                        avi_p2p_protocol::SensorValue::Temperature(v) => json!(v),
                        avi_p2p_protocol::SensorValue::Humidity(v) => json!(v),
                        avi_p2p_protocol::SensorValue::Battery(v) => json!(v),
                        avi_p2p_protocol::SensorValue::Status(v) => json!(v),
                        avi_p2p_protocol::SensorValue::Raw(v) => json!(v),
                    };

                    let payload = json!({
                        "name": sensor_name,
                        "data": {
                            "value": val,
                            "unit": match data {
                                avi_p2p_protocol::SensorValue::Temperature(_) => "C",
                                avi_p2p_protocol::SensorValue::Humidity(_) => "%",
                                _ => ""
                            },
                            "custom": custom_data.to_string()
                        },
                         "ts": std::time::SystemTime::now()
                           .duration_since(std::time::UNIX_EPOCH)
                           .unwrap_or_default().as_secs()
                    });

                    let _ = handle
                        .publish(&topic, serde_json::to_vec(&payload).unwrap())
                        .await;

                    match handle.get_ctx("").await {
                        Ok(v) => {
                            let mut current_ctx = v;

                            match set_nested_value(
                                &mut current_ctx,
                                &format!("avi.sensors.{}.{}", dev_id, sensor_name),
                                payload,
                            ) {
                                Ok(..) => handle
                                    .update_context(current_ctx)
                                    .await
                                    .unwrap_or_else(|_| println!("Failed to update context")),
                                Err(e) => eprintln!("Failed to update context: {}", e),
                            }
                        }
                        Err(e) => eprintln!("Failed to get current context: {}", e),
                    }
                }
            }
        }
    }

    async fn handle_downlink_event(
        event: AviEvent,
        socket: Arc<UdpSocket>,
        sessions: Arc<Mutex<HashMap<SocketAddr, DeviceSession>>>,
    ) {
        if let AviEvent::Message { topic, data, .. } = event {
            let sessions_lock = sessions.lock().await;

            // Send to all devices subscribed to this topic
            for (addr, session) in sessions_lock.iter() {
                if session.subscriptions.contains(&topic) {
                    let msg = DownlinkMessage::Message {
                        topic: &topic,
                        data: &data,
                    };

                    let mut tx_buf = [0u8; MAX_PACKET_SIZE];
                    if let Ok(encoded) = postcard::to_slice(&msg, &mut tx_buf) {
                        let _ = socket.send_to(encoded, addr).await;
                    }
                }
            }
        }
    }
}
