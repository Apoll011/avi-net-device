use crate::{AviEmbedded, AviEmbeddedConfig, MessageHandler, UdpClient};
use avi_p2p_protocol::{PressType, SensorValue};
use core::cell::RefCell;
use core::ffi::{c_char, c_void};
use core::slice;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use heapless::String;

// Command queue for async operations
const QUEUE_SIZE: usize = 16;

#[derive(Debug, Clone)]
pub enum AviCommand {
    Connect,
    Subscribe {
        topic: String<64>,
    },
    Unsubscribe {
        topic: String<64>,
    },
    Publish {
        topic: String<64>,
        data: [u8; 256],
        len: usize,
    },
    StartStream {
        id: u8,
        target: String<64>,
        reason: String<64>,
    },
    SendAudio {
        id: u8,
        data: [u8; 512],
        len: usize,
    },
    CloseStream {
        id: u8,
    },
    ButtonPress {
        button_id: u8,
        press_type: u8,
    },
    SensorFloat {
        name: String<32>,
        value: f32,
    },
    SensorInt {
        name: String<32>,
        value: i32,
    },
    Poll,
}

pub struct AviCommandQueue {
    channel: Channel<CriticalSectionRawMutex, AviCommand, QUEUE_SIZE>,
}

impl AviCommandQueue {
    pub const fn new() -> Self {
        Self {
            channel: Channel::new(),
        }
    }

    pub fn sender(&self) -> Sender<'_, CriticalSectionRawMutex, AviCommand, QUEUE_SIZE> {
        self.channel.sender()
    }

    pub fn receiver(&self) -> Receiver<'_, CriticalSectionRawMutex, AviCommand, QUEUE_SIZE> {
        self.channel.receiver()
    }
}

// Opaque types for C
pub struct CAviEmbedded {
    _private: [u8; 0],
}

// C callback types
pub type CMessageCallback = extern "C" fn(
    user_data: *mut c_void,
    topic: *const c_char,
    topic_len: usize,
    data: *const u8,
    data_len: usize,
);
pub type CUdpSendCallback =
    extern "C" fn(user_data: *mut c_void, buf: *const u8, len: usize) -> i32;
pub type CUdpReceiveCallback =
    extern "C" fn(user_data: *mut c_void, buf: *mut u8, buf_len: usize) -> i32;

// C-compatible UDP client wrapper
struct CUdpClientWrapper {
    user_data: *mut c_void,
    send_fn: CUdpSendCallback,
    recv_fn: CUdpReceiveCallback,
}

// Safety: We ensure these are only used from a single task
unsafe impl Send for CUdpClientWrapper {}

struct NoError;

impl UdpClient for CUdpClientWrapper {
    type Error = NoError;

    async fn send(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        let result = (self.send_fn)(self.user_data, buf.as_ptr(), buf.len());
        if result == 0 {
            Ok(())
        } else {
            Ok(()) // In no_std we can't really fail elegantly
        }
    }

    async fn receive(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let result = (self.recv_fn)(self.user_data, buf.as_mut_ptr(), buf.len());
        if result >= 0 {
            Ok(result as usize)
        } else {
            Ok(0)
        }
    }
}

// C-compatible message handler wrapper
struct CMessageHandlerWrapper {
    user_data: *mut c_void,
    callback: CMessageCallback,
}

// Safety: We ensure these are only used from a single task
unsafe impl Send for CMessageHandlerWrapper {}

impl MessageHandler for CMessageHandlerWrapper {
    fn on_message(&mut self, topic: &str, data: &[u8]) {
        (self.callback)(
            self.user_data,
            topic.as_ptr() as *const c_char,
            topic.len(),
            data.as_ptr(),
            data.len(),
        );
    }
}

// The actual wrapper that holds everything
pub struct AviWrapper {
    avi: RefCell<AviEmbedded<'static, CUdpClientWrapper, CMessageHandlerWrapper>>,
    command_queue: &'static AviCommandQueue,
}

// Configuration struct for C
#[repr(C)]
pub struct CAviEmbeddedConfig {
    pub device_id: u64,
}

// Global command queue (will be initialized once)
static mut COMMAND_QUEUE: Option<AviCommandQueue> = None;

// Initialize the command queue (call once at startup)
#[no_mangle]
pub extern "C" fn avi_embedded_init() {
    unsafe {
        if COMMAND_QUEUE.is_none() {
            COMMAND_QUEUE = Some(AviCommandQueue::new());
        }
    }
}

// Create instance
#[no_mangle]
pub extern "C" fn avi_embedded_new(
    config: CAviEmbeddedConfig,
    buffer: *mut u8,
    buffer_len: usize,
    udp_user_data: *mut c_void,
    udp_send_fn: CUdpSendCallback,
    udp_recv_fn: CUdpReceiveCallback,
    msg_user_data: *mut c_void,
    msg_callback: CMessageCallback,
) -> *mut CAviEmbedded {
    // Get the command queue
    let command_queue = unsafe {
        match &COMMAND_QUEUE {
            Some(q) => q as *const AviCommandQueue,
            None => return core::ptr::null_mut(),
        }
    };

    // SAFETY: We're leaking the buffer to make it 'static
    // The caller must ensure the buffer outlives the AVI instance
    let buffer_slice = unsafe { core::slice::from_raw_parts_mut(buffer, buffer_len) };
    let buffer_static: &'static mut [u8] = unsafe { core::mem::transmute(buffer_slice) };

    let udp_client = CUdpClientWrapper {
        user_data: udp_user_data,
        send_fn: udp_send_fn,
        recv_fn: udp_recv_fn,
    };

    let handler = CMessageHandlerWrapper {
        user_data: msg_user_data,
        callback: msg_callback,
    };

    let config = AviEmbeddedConfig {
        device_id: config.device_id,
    };

    let avi = AviEmbedded::new(udp_client, config, buffer_static, handler);

    let wrapper = AviWrapper {
        avi: RefCell::new(avi),
        command_queue: unsafe { &*command_queue },
    };

    Box::into_raw(Box::new(wrapper)) as *mut CAviEmbedded
}

// Destroy instance
#[no_mangle]
pub extern "C" fn avi_embedded_free(avi: *mut CAviEmbedded) {
    if !avi.is_null() {
        unsafe {
            let _ = Box::from_raw(avi as *mut AviWrapper);
        }
    }
}

// Helper to send command
fn send_command(avi: *mut CAviEmbedded, cmd: AviCommand) -> i32 {
    if avi.is_null() {
        return -1;
    }

    let wrapper = unsafe { &*(avi as *const AviWrapper) };
    match wrapper.command_queue.sender().try_send(cmd) {
        Ok(_) => 0,
        Err(_) => -2, // Queue full
    }
}

// Connect (non-blocking - queues the command)
#[no_mangle]
pub extern "C" fn avi_embedded_connect(avi: *mut CAviEmbedded) -> i32 {
    send_command(avi, AviCommand::Connect)
}

// Is connected
#[no_mangle]
pub extern "C" fn avi_embedded_is_connected(avi: *const CAviEmbedded) -> bool {
    if avi.is_null() {
        return false;
    }

    let wrapper = unsafe { &*(avi as *const AviWrapper) };
    wrapper.avi.borrow().is_connected()
}

// Subscribe
#[no_mangle]
pub extern "C" fn avi_embedded_subscribe(
    avi: *mut CAviEmbedded,
    topic: *const c_char,
    topic_len: usize,
) -> i32 {
    if avi.is_null() || topic.is_null() {
        return -1;
    }

    let topic_slice = unsafe { slice::from_raw_parts(topic as *const u8, topic_len) };
    let topic_str = core::str::from_utf8(topic_slice).unwrap_or("");

    let mut topic_string = String::<64>::new();
    if topic_string.push_str(topic_str).is_err() {
        return -1;
    }

    send_command(
        avi,
        AviCommand::Subscribe {
            topic: topic_string,
        },
    )
}

// Unsubscribe
#[no_mangle]
pub extern "C" fn avi_embedded_unsubscribe(
    avi: *mut CAviEmbedded,
    topic: *const c_char,
    topic_len: usize,
) -> i32 {
    if avi.is_null() || topic.is_null() {
        return -1;
    }

    let topic_slice = unsafe { slice::from_raw_parts(topic as *const u8, topic_len) };
    let topic_str = core::str::from_utf8(topic_slice).unwrap_or("");

    let mut topic_string = String::<64>::new();
    if topic_string.push_str(topic_str).is_err() {
        return -1;
    }

    send_command(
        avi,
        AviCommand::Unsubscribe {
            topic: topic_string,
        },
    )
}

// Publish
#[no_mangle]
pub extern "C" fn avi_embedded_publish(
    avi: *mut CAviEmbedded,
    topic: *const c_char,
    topic_len: usize,
    data: *const u8,
    data_len: usize,
) -> i32 {
    if avi.is_null() || topic.is_null() || data.is_null() || data_len > 256 {
        return -1;
    }

    let topic_slice = unsafe { slice::from_raw_parts(topic as *const u8, topic_len) };
    let topic_str = core::str::from_utf8(topic_slice).unwrap_or("");
    let data_slice = unsafe { slice::from_raw_parts(data, data_len) };

    let mut topic_string = String::<64>::new();
    if topic_string.push_str(topic_str).is_err() {
        return -1;
    }

    let mut data_buf = [0u8; 256];
    data_buf[..data_len].copy_from_slice(data_slice);

    send_command(
        avi,
        AviCommand::Publish {
            topic: topic_string,
            data: data_buf,
            len: data_len,
        },
    )
}

// Start stream
#[no_mangle]
pub extern "C" fn avi_embedded_start_stream(
    avi: *mut CAviEmbedded,
    stream_id: u8,
    target_peer: *const c_char,
    target_peer_len: usize,
    reason: *const c_char,
    reason_len: usize,
) -> i32 {
    if avi.is_null() || target_peer.is_null() || reason.is_null() {
        return -1;
    }

    let target_slice = unsafe { slice::from_raw_parts(target_peer as *const u8, target_peer_len) };
    let reason_slice = unsafe { slice::from_raw_parts(reason as *const u8, reason_len) };

    let mut target_string = String::<64>::new();
    let mut reason_string = String::<64>::new();

    if target_string
        .push_str(core::str::from_utf8(target_slice).unwrap_or(""))
        .is_err()
    {
        return -1;
    }
    if reason_string
        .push_str(core::str::from_utf8(reason_slice).unwrap_or(""))
        .is_err()
    {
        return -1;
    }

    send_command(
        avi,
        AviCommand::StartStream {
            id: stream_id,
            target: target_string,
            reason: reason_string,
        },
    )
}

// Send audio
#[no_mangle]
pub extern "C" fn avi_embedded_send_audio(
    avi: *mut CAviEmbedded,
    stream_id: u8,
    pcm_data: *const u8,
    pcm_len: usize,
) -> i32 {
    if avi.is_null() || pcm_data.is_null() || pcm_len > 512 {
        return -1;
    }

    let data_slice = unsafe { slice::from_raw_parts(pcm_data, pcm_len) };
    let mut data_buf = [0u8; 512];
    data_buf[..pcm_len].copy_from_slice(data_slice);

    send_command(
        avi,
        AviCommand::SendAudio {
            id: stream_id,
            data: data_buf,
            len: pcm_len,
        },
    )
}

// Close stream
#[no_mangle]
pub extern "C" fn avi_embedded_close_stream(avi: *mut CAviEmbedded, stream_id: u8) -> i32 {
    send_command(avi, AviCommand::CloseStream { id: stream_id })
}

// Button pressed
#[no_mangle]
pub extern "C" fn avi_embedded_button_pressed(
    avi: *mut CAviEmbedded,
    button_id: u8,
    press_type: u8,
) -> i32 {
    send_command(
        avi,
        AviCommand::ButtonPress {
            button_id,
            press_type,
        },
    )
}

// Update sensor
#[no_mangle]
pub extern "C" fn avi_embedded_update_sensor_float(
    avi: *mut CAviEmbedded,
    name: *const c_char,
    name_len: usize,
    value: f32,
) -> i32 {
    if avi.is_null() || name.is_null() {
        return -1;
    }

    let name_slice = unsafe { slice::from_raw_parts(name as *const u8, name_len) };
    let mut name_string = String::<32>::new();
    if name_string
        .push_str(core::str::from_utf8(name_slice).unwrap_or(""))
        .is_err()
    {
        return -1;
    }

    send_command(
        avi,
        AviCommand::SensorFloat {
            name: name_string,
            value,
        },
    )
}

#[no_mangle]
pub extern "C" fn avi_embedded_update_sensor_int(
    avi: *mut CAviEmbedded,
    name: *const c_char,
    name_len: usize,
    value: i32,
) -> i32 {
    if avi.is_null() || name.is_null() {
        return -1;
    }

    let name_slice = unsafe { slice::from_raw_parts(name as *const u8, name_len) };
    let mut name_string = String::<32>::new();
    if name_string
        .push_str(core::str::from_utf8(name_slice).unwrap_or(""))
        .is_err()
    {
        return -1;
    }

    send_command(
        avi,
        AviCommand::SensorInt {
            name: name_string,
            value,
        },
    )
}

// Poll for messages
#[no_mangle]
pub extern "C" fn avi_embedded_poll(avi: *mut CAviEmbedded) -> i32 {
    send_command(avi, AviCommand::Poll)
}

// Get the receiver for the async task (call this once to start the async executor)
#[no_mangle]
pub extern "C" fn avi_embedded_get_command_receiver() -> *const c_void {
    unsafe {
        match &COMMAND_QUEUE {
            Some(q) => &q.channel as *const _ as *const c_void,
            None => core::ptr::null(),
        }
    }
}

// Async task executor function (to be called from embassy task)
pub async fn avi_process_commands(avi: &AviWrapper) {
    let receiver = avi.command_queue.receiver();

    loop {
        let cmd = receiver.receive().await;

        let mut avi_mut = avi.avi.borrow_mut();

        match cmd {
            AviCommand::Connect => {
                let _ = avi_mut.connect().await;
            }
            AviCommand::Subscribe { topic } => {
                let _ = avi_mut.subscribe(topic.as_str()).await;
            }
            AviCommand::Unsubscribe { topic } => {
                let _ = avi_mut.unsubscribe(topic.as_str()).await;
            }
            AviCommand::Publish { topic, data, len } => {
                let _ = avi_mut.publish(topic.as_str(), &data[..len]).await;
            }
            AviCommand::StartStream { id, target, reason } => {
                let _ = avi_mut
                    .start_stream(id, target.as_str(), reason.as_str())
                    .await;
            }
            AviCommand::SendAudio { id, data, len } => {
                let _ = avi_mut.send_audio(id, &data[..len]).await;
            }
            AviCommand::CloseStream { id } => {
                let _ = avi_mut.close_stream(id).await;
            }
            AviCommand::ButtonPress {
                button_id,
                press_type,
            } => {
                let press = match press_type {
                    0 => PressType::Short,
                    1 => PressType::Long,
                    2 => PressType::Double,
                    _ => PressType::Short,
                };
                let _ = avi_mut.button_pressed(button_id, press).await;
            }
            AviCommand::SensorFloat { name, value } => {
                let _ = avi_mut
                    .update_sensor(name.as_str(), SensorValue::Float(value))
                    .await;
            }
            AviCommand::SensorInt { name, value } => {
                let _ = avi_mut
                    .update_sensor(name.as_str(), SensorValue::Int(value))
                    .await;
            }
            AviCommand::Poll => {
                let _ = avi_mut.poll().await;
            }
        }
    }
}
