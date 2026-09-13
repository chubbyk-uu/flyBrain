pub mod aerodynamics;
pub mod behavior;
pub mod brain_signal;
pub mod cns_olfaction;
pub mod cns_pathway;
#[cfg(any(
    target_os = "macos",
    target_os = "emscripten",
    all(target_os = "linux", feature = "cuda")
))]
pub mod display_protocol;
pub mod embodiment;
pub mod fixture;
#[cfg(any(
    target_os = "macos",
    target_os = "emscripten",
    all(target_os = "linux", feature = "cuda")
))]
pub mod flight;
pub mod flight_behavior;
pub mod flight_targets;
pub mod flybody_policy;
pub mod foraging;
pub mod gait;
pub mod grooming;
pub mod habitat;
pub mod homeostasis;
pub mod neural_io;
pub mod npy;
pub mod obstacle_avoidance;
pub mod odor_guidance;
pub mod search_progress;
pub mod food_search;
pub mod olfaction;
pub mod pack;
pub mod parameters;
pub mod protocol;
pub mod reference;
pub mod stimulus;
pub mod system_id;

#[cfg(target_os = "macos")]
pub mod output;

#[cfg(any(
    target_os = "macos",
    target_os = "emscripten",
    all(target_os = "linux", feature = "cuda")
))]
pub mod brain_bridge;

#[cfg(target_os = "macos")]
pub mod metal_engine;

#[cfg(all(feature = "cuda", target_os = "linux"))]
pub mod cuda_engine;

#[cfg(target_os = "emscripten")]
pub mod browser_engine;

#[cfg(target_os = "emscripten")]
pub mod browser;

#[cfg(any(
    target_os = "macos",
    target_os = "emscripten",
    all(target_os = "linux", feature = "cuda")
))]
pub mod world;

#[cfg(any(target_os = "macos", all(target_os = "linux", feature = "cuda")))]
pub mod render;

#[cfg(any(target_os = "macos", all(target_os = "linux", feature = "cuda")))]
pub mod live_viewer;

#[cfg(any(
    target_os = "macos",
    target_os = "emscripten",
    all(target_os = "linux", feature = "cuda")
))]
pub mod retina;

#[cfg(any(
    target_os = "macos",
    target_os = "emscripten",
    all(target_os = "linux", feature = "cuda")
))]
pub mod scene_layout;

#[cfg(any(
    target_os = "macos",
    target_os = "emscripten",
    all(target_os = "linux", feature = "cuda")
))]
pub mod world_sim;

#[cfg(any(target_os = "macos", all(target_os = "linux", feature = "cuda")))]
pub mod flight_system_id_world;
