//! Read-only WebSocket transport from the native simulation to the Three.js viewer.
use super::*;
use flybrain_engine::display_protocol::{body_poses, scene_descriptor};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tungstenite::{Message, WebSocket, accept};

type RetinaMailbox = Arc<Mutex<Option<(u64, Vec<u8>)>>>;

pub struct Server {
    stop: Arc<AtomicBool>,
    retina: RetinaMailbox,
    thread: Option<JoinHandle<Result<()>>>,
}

impl Server {
    pub fn start(
        bind: SocketAddr,
        publish_hz: u32,
        frame: Arc<Mutex<view_worker::Frame>>,
    ) -> Result<Self> {
        let listener = TcpListener::bind(bind)
            .with_context(|| format!("binding native viewer WebSocket to {bind}"))?;
        listener.set_nonblocking(true)?;
        let scene = {
            let frame = frame.lock().unwrap();
            serde_json::to_string(&json!({
                "type": "scene",
                "scene": scene_descriptor(
                    frame.data.model(),
                    frame.neurons,
                    frame.brain_model.as_deref(),
                    frame.brain_backend.as_deref(),
                ),
            }))?
        };
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let retina = Arc::new(Mutex::new(None));
        let worker_retina = retina.clone();
        let thread = std::thread::Builder::new()
            .name("flybrain-websocket".into())
            .spawn(move || {
                serve(
                    listener,
                    publish_hz,
                    frame,
                    scene,
                    worker_retina,
                    worker_stop,
                )
            })?;
        Ok(Self {
            stop,
            retina,
            thread: Some(thread),
        })
    }

    pub fn update_retina(&self, sequence: u64, pixels: Vec<u8>) {
        *self.retina.lock().unwrap() = Some((sequence, pixels));
    }

    pub fn finish(&mut self) -> Result<()> {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| anyhow::anyhow!("WebSocket publisher panicked"))??;
        }
        Ok(())
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            eprintln!("WebSocket publisher: {error:#}");
        }
    }
}

fn serve(
    listener: TcpListener,
    publish_hz: u32,
    frame: Arc<Mutex<view_worker::Frame>>,
    scene: String,
    retina: RetinaMailbox,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let period = Duration::from_secs_f64(1.0 / f64::from(publish_hz));
    let mut clients: Vec<WebSocket<std::net::TcpStream>> = Vec::new();
    let mut next_publish = Instant::now();
    let mut last_sequence = u64::MAX;
    let mut last_retina_sequence = u64::MAX;
    while !stop.load(Ordering::Relaxed) {
        loop {
            match listener.accept() {
                Ok((stream, peer)) => {
                    stream.set_nodelay(true)?;
                    stream.set_write_timeout(Some(Duration::from_millis(100)))?;
                    match accept(stream) {
                        Ok(mut socket) => {
                            socket.send(Message::Text(scene.clone().into()))?;
                            eprintln!("Native browser viewer connected: {peer}");
                            clients.push(socket);
                        }
                        Err(error) => eprintln!("Rejected WebSocket client {peer}: {error}"),
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => return Err(error.into()),
            }
        }
        let now = Instant::now();
        if now >= next_publish {
            let payload = {
                let frame = frame.lock().unwrap();
                (frame.sequence != last_sequence).then(|| {
                    last_sequence = frame.sequence;
                    serde_json::to_string(&json!({
                        "type": "frame",
                        "sequence": frame.sequence,
                        "epoch": frame.epoch,
                        "poses": body_poses(&frame.data),
                        "snapshot": snapshot_payload(&frame),
                    }))
                })
            };
            if let Some(payload) = payload.transpose()? {
                clients.retain_mut(|client| {
                    client.send(Message::Text(payload.clone().into())).is_ok()
                });
            }
            next_publish = now + period;
        }
        let retina_message = {
            let retina = retina.lock().unwrap();
            retina.as_ref().and_then(|(sequence, pixels)| {
                (*sequence != last_retina_sequence).then(|| {
                    last_retina_sequence = *sequence;
                    encode_retina_preview(*sequence, pixels)
                })
            })
        };
        if let Some(message) = retina_message {
            clients
                .retain_mut(|client| client.send(Message::Binary(message.clone().into())).is_ok());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

fn encode_retina_preview(sequence: u64, pixels: &[u8]) -> Vec<u8> {
    const WIDTH: u16 = 450;
    const HEIGHT: u16 = 256;
    debug_assert_eq!(pixels.len(), usize::from(WIDTH) * usize::from(HEIGHT));
    let mut message = Vec::with_capacity(16 + pixels.len());
    message.extend_from_slice(b"FBR1");
    message.extend_from_slice(&WIDTH.to_le_bytes());
    message.extend_from_slice(&HEIGHT.to_le_bytes());
    message.extend_from_slice(&sequence.to_le_bytes());
    message.extend_from_slice(pixels);
    message
}

#[cfg(test)]
mod tests {
    use super::encode_retina_preview;

    #[test]
    fn retina_preview_binary_header_is_stable() {
        let pixels = vec![7; 450 * 256];
        let message = encode_retina_preview(42, &pixels);
        assert_eq!(&message[..4], b"FBR1");
        assert_eq!(u16::from_le_bytes(message[4..6].try_into().unwrap()), 450);
        assert_eq!(u16::from_le_bytes(message[6..8].try_into().unwrap()), 256);
        assert_eq!(u64::from_le_bytes(message[8..16].try_into().unwrap()), 42);
        assert_eq!(&message[16..], pixels);
    }
}

fn snapshot_payload(frame: &view_worker::Frame) -> serde_json::Value {
    let snapshot = frame.snapshot;
    json!({
        "time_seconds": snapshot.time_seconds,
        "root_position": snapshot.root_position,
        "horizontal_speed_mm_s": snapshot.horizontal_speed_mm_s,
        "body_pitch_deg": snapshot.body_pitch_deg,
        "flight_mode": format!("{:?}", snapshot.flight_mode),
        "behavior_mode": format!("{:?}", snapshot.behavior_mode),
        "foraging_mode": format!("{:?}", snapshot.foraging_mode),
        "food_center": snapshot.food_center,
        "food_enabled": snapshot.food_enabled,
        "food_distance": snapshot.food_distance,
        "flight_allowed": snapshot.flight_allowed,
        "taste_active": snapshot.taste_active,
        "tasted_resource": frame.tasted_resource,
        "nearest_resource": frame.nearest_resource,
        "odor_left_ppm": snapshot.odor_left_ppm,
        "odor_right_ppm": snapshot.odor_right_ppm,
        "visual_left": snapshot.visual_left,
        "visual_right": snapshot.visual_right,
        "visual_event_delta": snapshot.visual_event_delta,
        "population_spike_delta": snapshot.population_spike_delta,
        "filtered_population_rate_hz": snapshot.filtered_population_rate_hz,
        "cumulative_spiking_neuron_count": snapshot.cumulative_spiking_neuron_count,
        "brain_flight_drive": snapshot.brain_flight_drive,
        "brain_walking_drive": snapshot.brain_walking_drive,
        "brain_walking_steering": snapshot.brain_walking_steering,
        "brain_flight_steering": snapshot.brain_flight_steering,
        "grooming_active": snapshot.grooming_active,
        "feeding_extension": snapshot.feeding_extension,
        "brain_wall_seconds": snapshot.brain_wall_seconds,
        "physics_wall_seconds": snapshot.physics_wall_seconds,
        "paused": frame.paused,
        "realtime_factor": frame.realtime_factor,
    })
}
