//! Native display transport with a restricted lifecycle-only control channel.
use super::*;
use flybrain_engine::display_protocol::{body_poses, scene_descriptor};
use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tungstenite::{Message, WebSocket, accept};

type RetinaMailbox = Arc<Mutex<Option<(u64, u64, Vec<u8>)>>>;

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
        commands: std::sync::mpsc::SyncSender<view_worker::Command>,
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
                    commands,
                )
            })?;
        Ok(Self {
            stop,
            retina,
            thread: Some(thread),
        })
    }

    pub fn update_retina(&self, sequence: u64, epoch: u64, pixels: Vec<u8>) {
        *self.retina.lock().unwrap() = Some((sequence, epoch, pixels));
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
    commands: std::sync::mpsc::SyncSender<view_worker::Command>,
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
                    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
                    match accept(stream) {
                        Ok(mut socket) => {
                            socket.send(Message::Text(scene.clone().into()))?;
                            socket.get_mut().set_nonblocking(true)?;
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
        clients.retain_mut(|client| {
            for _ in 0..8 {
                match client.read() {
                    Ok(Message::Text(text)) if text.len() < 1024 => {
                        #[derive(serde::Deserialize)]
                        struct Request { r#type: String, command: view_worker::ViewerControl }
                        if let Ok(request) = serde_json::from_str::<Request>(&text)
                            && request.r#type == "control" {
                            if commands.try_send(view_worker::Command::Viewer(request.command)).is_err() {
                                return false;
                            }
                        }
                    }
                    Ok(Message::Close(_)) => return false,
                    Ok(_) => {},
                    Err(tungstenite::Error::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(_) => return false,
                }
            }
            true
        });
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
            retina.as_ref().and_then(|(sequence, epoch, pixels)| {
                (*sequence != last_retina_sequence && *epoch == frame.lock().unwrap().epoch).then(|| {
                    last_retina_sequence = *sequence;
                    encode_retina_preview(*sequence, *epoch, pixels)
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

fn encode_retina_preview(sequence: u64, epoch: u64, pixels: &[u8]) -> Vec<u8> {
    const WIDTH: u16 = 450;
    const HEIGHT: u16 = 256;
    debug_assert_eq!(pixels.len(), usize::from(WIDTH) * usize::from(HEIGHT));
    let mut message = Vec::with_capacity(24 + pixels.len());
    message.extend_from_slice(b"FBR2");
    message.extend_from_slice(&WIDTH.to_le_bytes());
    message.extend_from_slice(&HEIGHT.to_le_bytes());
    message.extend_from_slice(&sequence.to_le_bytes());
    message.extend_from_slice(&epoch.to_le_bytes());
    message.extend_from_slice(pixels);
    message
}

#[cfg(test)]
mod tests {
    use super::encode_retina_preview;

    #[test]
    fn retina_preview_binary_header_is_stable() {
        let pixels = vec![7; 450 * 256];
        let message = encode_retina_preview(42, 3, &pixels);
        assert_eq!(&message[..4], b"FBR2");
        assert_eq!(u16::from_le_bytes(message[4..6].try_into().unwrap()), 450);
        assert_eq!(u16::from_le_bytes(message[6..8].try_into().unwrap()), 256);
        assert_eq!(u64::from_le_bytes(message[8..16].try_into().unwrap()), 42);
        assert_eq!(u64::from_le_bytes(message[16..24].try_into().unwrap()), 3);
        assert_eq!(&message[24..], pixels);
    }
}

fn snapshot_payload(frame: &view_worker::Frame) -> serde_json::Value {
    let snapshot = frame.snapshot;
    let wing_envelope =
        if snapshot.flight_mode == flybrain_engine::flight_behavior::FlightMode::Grounded {
            0.0
        } else {
            snapshot.flight_amplitude_scale.clamp(0.0, 1.0)
        };
    let mut payload=json!({
        "time_seconds": snapshot.time_seconds,
        "root_position": snapshot.root_position,
        "horizontal_speed_mm_s": snapshot.horizontal_speed_mm_s,
        "flight_command_velocity_mm_s": snapshot.flight_command_velocity_mm_s,
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
        "wing_display": {
            "source": "cns-hybrid-flight-command",
            "physical_frequency_hz": if wing_envelope > 0.0 { 218.0 * snapshot.flight_frequency_scale } else { 0.0 },
            "phase_cycles": (snapshot.time_seconds * 218.0).rem_euclid(1.0),
            "envelope": wing_envelope,
            "steering": snapshot.flight_steering.clamp(-1.0, 1.0),
        },
        "grooming_active": snapshot.grooming_active,
        "feeding_extension": snapshot.feeding_extension,
        "brain_wall_seconds": snapshot.brain_wall_seconds,
        "physics_wall_seconds": snapshot.physics_wall_seconds,
        "paused": frame.paused,
        "hunger": snapshot.hunger,
        "hungry": snapshot.hungry,
        "flight_fatigue": snapshot.fatigue,
        "grooming_urge": snapshot.dirt,
        "takeoff_inhibited": !matches!(snapshot.takeoff_inhibited_reason,""|"none"),
        "takeoff_inhibited_reason": snapshot.takeoff_inhibited_reason,
        "food_search": snapshot.food_search,
        "realtime_factor": frame.realtime_factor,
    });
    payload["behavior_seed"] = json!(snapshot.behavior_seed);
    payload["grooming_cooldown_seconds"]=json!(snapshot.grooming_cooldown_seconds);
    payload["grooming_completed_bouts"]=json!(snapshot.grooming_completed_bouts);
    payload["grooming_opportunity_count"]=json!(snapshot.grooming_opportunity_count);
    payload
}
