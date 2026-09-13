//! Isolated production take. No runtime viewer or sensory behavior is changed.
use flybrain_engine::{display_protocol::{body_poses,scene_descriptor},live_viewer::LiveViewer,
    retina::FlyGymRetina,world_sim::{SimulationStepper,SimulationParameters}};
use serde_json::json;
fn main()->anyhow::Result<()> {
    let output=std::env::args().nth(1).unwrap_or_else(||"outputs/social-video-v2/sources/stereo".to_owned());
    let out=std::path::Path::new(&output);
    std::fs::create_dir_all(out)?;
    anyhow::ensure!(!out.join("take.json").exists(),"take exists; refuse to overwrite");
    let assets="assets/neuromechfly";
    let mut sim=SimulationStepper::new_with_parameters_physics_and_scene(assets,Some("outputs/packs/male_cns_v1"),500.0,0.5,SimulationParameters::default(),Some(0.0002),"indoor-v2")?;
    sim.set_behavior_seed(11)?;
    sim.set_brain_telemetry_enabled(true)?;
    let scene=scene_descriptor(sim.world().model(),sim.brain_neuron_count(),sim.brain_model_name(),sim.brain_device_name());
    LiveViewer::configure_render_quality(sim.world_mut().data_mut(),1024,0)?;
    let mut renderer=LiveViewer::new_hidden_sensor(sim.world().model(),assets,0)?;
    let mut processors=[FlyGymRetina::load(assets)?,FlyGymRetina::load(assets)?];
    let mut frames=Vec::new();let mut next=10.0;
    while sim.snapshot().time_seconds<27.0 {
        let s=sim.step_window()?;
        if s.time_seconds+1e-8<next {continue;}
        anyhow::ensure!(s.filtered_population_rate_hz>0.0 && s.cumulative_spiking_neuron_count>0,"neural telemetry is missing");
        next+=1.0/30.0;
        let time=sim.world().data().time();let qpos=sim.world().data().qpos().to_vec();
        let poses=body_poses(sim.world().data());
        let pixels=renderer.capture_film_stereo(sim.world_mut().data_mut(),&mut processors,s.food_center,s.food_enabled)?;
        anyhow::ensure!(sim.world().data().time()==time && sim.world().data().qpos()==qpos,"capture changed physical state");
        let filename=format!("{:04}.gray",frames.len());std::fs::write(out.join(&filename),pixels)?;
        frames.push(json!({"poses":poses,"retina_file":filename,"left_time":s.time_seconds,"right_time":s.time_seconds,
            "snapshot":{"time_seconds":s.time_seconds,"root_position":s.root_position,"flight_mode":format!("{:?}",s.flight_mode),
            "hunger":s.hunger,"filtered_population_rate_hz":s.filtered_population_rate_hz,
            "cumulative_spiking_neuron_count":s.cumulative_spiking_neuron_count,
            "wing_display":{"envelope":s.flight_amplitude_scale,"physical_frequency_hz":218.0*s.flight_frequency_scale,"steering":s.flight_steering}}}));
        if frames.len()%60==0 {eprintln!("paired film frame {} at {:.3}s {:?}, {:.0} spikes/s",frames.len(),s.time_seconds,s.flight_mode,s.filtered_population_rate_hz);}
    }
    std::fs::write(out.join("take.json"),serde_json::to_vec(&json!({"display_replay":{"scene":scene,"frames":frames},"seed":11,"sync":"body, both native eyes and neural telemetry from identical unadvanced physical state; independent presentation processors; no live sensory feedback"}))?)?;
    Ok(())
}
