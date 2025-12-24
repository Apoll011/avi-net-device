use avi_p2p::{AviP2p, AviP2pConfig, AviEvent, AviP2pHandle, StreamId, PeerId};
use tokio::io::{self, AsyncBufReadExt};
use tokio::time::{sleep, Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Simulates capturing audio from a microphone and sending it over the network.
/// Sends ~1KB of data every 50ms (simulating 20Hz updates).
async fn run_simulated_microphone(handle: AviP2pHandle, stream_id: StreamId, active: Arc<AtomicBool>) {
    println!("🎙️  Microphone ACTIVE for Stream {}", stream_id);
    let mut packet_count = 0u64;

    while active.load(Ordering::Relaxed) {
        // Simulate recording audio (silence/noise)
        let mut dummy_audio = vec![0u8; 1024];
        // Put the packet count in the first bytes so we can track it on the other side
        let count_bytes = packet_count.to_be_bytes();
        dummy_audio[0..8].copy_from_slice(&count_bytes);

        // Send data
        if let Err(e) = handle.send_audio(stream_id, dummy_audio).await {
            eprintln!("🔴 Microphone stopped (Error sending: {})", e);
            break;
        }

        packet_count += 1;
        // 50ms interval
        sleep(Duration::from_millis(50)).await;
    }
    println!("🎙️  Microphone STOPPED for Stream {}", stream_id);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Zero-Config Start (Uses mDNS + Auto-Dial internally)
    let config = AviP2pConfig::default();

    println!("🚀 Starting AVI Audio Node...");
    println!("   Wait for '🔗 CONNECTED' before calling.");

    let (node, mut event_rx) = AviP2p::start(config).await?;
    let handle = node.handle();

    // Track active calls to stop them later
    let active_call = Arc::new(AtomicBool::new(false));

    // 2. Event Handling Loop
    let handle_clone = handle.clone();
    let active_call_clone = active_call.clone();

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AviEvent::Started { local_peer_id, .. } => {
                    println!("✅ Node Online. My ID: {}", local_peer_id);
                },
                AviEvent::PeerConnected { peer_id, .. } => {
                    println!("🔗 CONNECTED to {}. Ready to call.", peer_id);
                },
                AviEvent::PeerDisconnected { peer_id } => {
                    println!("🔌 Disconnected from {}", peer_id);
                },

                // --- Audio Logic ---

                // Case A: Someone is calling us
                AviEvent::AudioStreamRequested { from, stream_id } => {
                    println!("\n📞 INCOMING CALL from {} (Stream {})", from, stream_id);
                    println!("   Auto-accepting call...");

                    if let Err(e) = handle_clone.accept_audio_stream(stream_id).await {
                        eprintln!("   Error accepting call: {}", e);
                    } else {
                        println!("✅ Call Accepted!");
                        // Start sending our own audio back
                        active_call_clone.store(true, Ordering::Relaxed);
                        let h = handle_clone.clone();
                        let ac = active_call_clone.clone();
                        tokio::spawn(async move {
                            run_simulated_microphone(h, stream_id, ac).await;
                        });
                    }
                },

                // Case B: We called someone and they accepted
                AviEvent::AudioStreamAccepted { peer_id, stream_id } => {
                    println!("\n✅ CALL ESTABLISHED with {} (Stream {})", peer_id, stream_id);
                    // Start sending our audio
                    active_call_clone.store(true, Ordering::Relaxed);
                    let h = handle_clone.clone();
                    let ac = active_call_clone.clone();
                    tokio::spawn(async move {
                        run_simulated_microphone(h, stream_id, ac).await;
                    });
                },

                // Case C: Receiving Audio Data
                AviEvent::AudioData { from, stream_id, data } => {
                    // Extract the packet count we put in earlier
                    let mut count_bytes = [0u8; 8];
                    if data.len() >= 8 {
                        count_bytes.copy_from_slice(&data[0..8]);
                        let seq = u64::from_be_bytes(count_bytes);
                        // Print on same line to avoid spamming
                        print!("\r🔊 Hearing {} [Stream {}] | Pkt #{} | Size: {}b   ", from, stream_id, seq, data.len());
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    }
                },

                // Case D: Call Ended
                AviEvent::AudioStreamClosed { peer_id, stream_id, reason } => {
                    println!("\n❌ Call with {} ended (Stream {}). Reason: {:?}", peer_id, stream_id, reason);
                    active_call_clone.store(false, Ordering::Relaxed);
                },

                _ => {} // Ignore PubSub/Discovery events
            }
        }
    });

    // 3. Command Loop
    println!("\nCommands:");
    println!("  call <peer_id>  -> Start an audio stream");
    println!("  hangup <id>     -> End stream");
    println!("  peers           -> List connected peers");
    println!("  quit            -> Exit");

    let mut stdin = io::BufReader::new(io::stdin()).lines();
    while let Ok(Some(line)) = stdin.next_line().await {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "call" if parts.len() > 1 => {
                let target = PeerId::new(parts[1]);
                println!("📞 Dialing {}...", target);
                match handle.request_audio_stream(target).await {
                    Ok(id) => println!("   Request sent. Stream ID: {}", id),
                    Err(e) => eprintln!("   Failed to request call: {}", e),
                }
            },
            "hangup" if parts.len() > 1 => {
                if let Ok(id) = parts[1].parse::<u64>() {
                    active_call.store(false, Ordering::Relaxed);
                    let _ = handle.close_audio_stream(StreamId(id)).await;
                    println!("   Hung up stream {}", id);
                }
            },
            "peers" => {
                match handle.connected_peers().await {
                    Ok(peers) => {
                        println!("--- Connected Peers ---");
                        for p in peers { println!("  {}", p); }
                        println!("-----------------------");
                    }
                    Err(e) => eprintln!("Error fetching peers: {}", e),
                }
            },
            "quit" => break,
            _ => println!("Unknown command"),
        }
    }

    node.shutdown().await?;
    Ok(())
}