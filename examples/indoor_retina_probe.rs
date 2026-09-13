//! Native sensory evidence, on the main thread required by GLFW.
use flybrain_engine::live_viewer::LiveViewer;
use flybrain_engine::world::{MuJoCoWorld, DEFAULT_ASSETS_DIR};

fn main() -> anyhow::Result<()> {
    let mut world = MuJoCoWorld::from_assets_dir_and_scene(DEFAULT_ASSETS_DIR, "indoor-v2")?;
    LiveViewer::configure_render_quality(world.data_mut(), 1024, 0)?;
    let mut sensor = LiveViewer::new_hidden_sensor(world.model(), DEFAULT_ASSETS_DIR, 0)?;
    let output = std::path::Path::new("outputs/indoor-v2/stage-2/fixed-retina");
    std::fs::create_dir_all(output)?;
    for (name, position) in [("sugar", [26.0,-12.0,32.1]), ("nectar", [-60.0,12.0,67.1])] {
        world.data_mut().qpos_mut()[0..3].copy_from_slice(&position);
        world.data_mut().forward();
        sensor.clear_retina();
        for frame in 0..12 {
            // Rendering-only pose fixture: no neural/physics runtime clock is advanced.
            world.data_mut().set_time(frame as f64 * 0.04);
            sensor.capture_retina_sensor(world.data_mut(), [48.0,-12.0,30.6], true)?;
        }
        anyhow::ensure!(sensor.retina_capture_sequence() > 2, "retina did not deliver");
        for summary in sensor.retina_summaries() {
            anyhow::ensure!(summary.mean_intensity > 0.0 && summary.spatial_contrast > 0.0, "empty retina");
        }
        let mut pgm = b"P5\n450 256\n255\n".to_vec();
        pgm.extend_from_slice(&sensor.retina_preview_gray_half());
        std::fs::write(output.join(format!("{name}.pgm")), pgm)?;
        println!("fixed retina {name}: root={position:?}, summaries={:?}", sensor.retina_summaries());
    }
    Ok(())
}
