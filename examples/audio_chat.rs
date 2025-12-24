use avi_p2p::{AviP2p, AviP2pConfig, AviEvent, AviP2pHandle};
use std::time::Duration;
use tokio::time::sleep;

// Simulates an audio source (e.g., Microphone)
async fn run_sender(handle: AviP2pHandle, target_peer: String) {
    let peer_id = avi_p2p::PeerId::new(&target_peer);

    println!("Requesting audio stream to {}...", peer_id);
    match handle.request_audio_stream(peer_id).await {
        Ok(stream_id) => {
            println!("Stream request sent. ID: {}", stream_id);
            // Note: In a real app, we wait for AviEvent::AudioStreamAccepted
            // before sending heavy data, but for this demo we start sending chunks.

            for i in 0..5 {
                let dummy_audio = vec![0u8; 1024]; // 1KB of silence
                if let Err(e) = handle.send_audio(stream_id, dummy_audio).await {
                    eprintln!("Failed to send audio: {}", e);
                    break;
                }
                println!("Sent chunk #{}", i);
                sleep(Duration::from_millis(200)).await;
            }

            let _ = handle.close_audio_stream(stream_id).await;
            println!("Stream closed locally.");
        }
        Err(e) => eprintln!("Failed to request stream: {}", e),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args();
    let _ = args.next();
    let mode = args.next().expect("Usage: cargo run --example audio_chat <listen|dial> [peer_addr]");

    let config = AviP2pConfig::default(); // Uses mDNS for local discovery
    let (node, mut event_rx) = AviP2p::start(config).await?;
    let handle = node.handle();

    // Event Loop
    let handle_clone = handle.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            match event {
                AviEvent::AudioStreamRequested { from, stream_id } => {
                    println!("📞 Call from {} (Stream {})", from, stream_id);
                    // Auto-accept incoming calls
                    if let Err(e) = handle_clone.accept_audio_stream(stream_id).await {
                        eprintln!("Failed to accept: {}", e);
                    } else {
                        println!("✅ Accepted stream {}", stream_id);
                    }
                },
                AviEvent::AudioStreamAccepted { peer_id, stream_id } => {
                    println!("✅ Peer {} accepted stream {}", peer_id, stream_id);
                },
                AviEvent::AudioData { from, stream_id, data } => {
                    println!("🔊 Audio data from {} (Stream {}): {} bytes", from, stream_id, data.len());
                },
                AviEvent::AudioStreamClosed { peer_id, stream_id, reason } => {
                    println!("❌ Stream {} with {} ended: {:?}", stream_id, peer_id, reason);
                },
                _ => {}
            }
        }
    });

    if mode == "dial" {
        let target = args.next().expect("Provide peer ID to dial");
        // Give mDNS a second to find the peer before we try to stream
        println!("Waiting for discovery...");
        sleep(Duration::from_secs(2)).await;
        run_sender(handle, target).await;
    } else {
        println!("Listening for incoming streams... (Ctrl+C to quit)");
        println!("My Peer ID can be seen in the 'Started' event logs of other nodes.");
        tokio::signal::ctrl_c().await?;
    }

    node.shutdown().await?;
    Ok(())
}