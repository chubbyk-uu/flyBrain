use std::f64::consts::PI;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::flight_behavior::FlightMode;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct HomeostasisParameters {
    pub initial_hunger: f64,
    pub hunger_rate_per_second: f64,
    pub feeding_relief_per_second: f64,
    pub hunger_enter: f64,
    pub hunger_release: f64,
    pub feeding_extension_threshold: f64,
    pub feeding_mn9_threshold_hz: f64,
    pub initial_fatigue: f64,
    pub flight_fatigue_rate_per_second: f64,
    pub support_recovery_rate_per_second: f64,
    pub fatigue_landing_enter: f64,
    pub fatigue_takeoff_release: f64,
    pub minimum_rest_seconds: f64,
    pub minimum_grounded_seconds: f64,
    pub waypoint_min_seconds: f64,
    pub waypoint_max_seconds: f64,
    pub waypoint_margin_fraction: f64,
    pub exploration_steering_gain: f64,
    pub exploration_flight_speed_scale: f64,
}

impl Default for HomeostasisParameters {
    fn default() -> Self {
        Self {
            initial_hunger: 0.72,
            hunger_rate_per_second: (0.55-0.30)/60.0,
            feeding_relief_per_second: 0.22,
            hunger_enter: 0.55,
            hunger_release: 0.30,
            feeding_extension_threshold: 0.10,
            feeding_mn9_threshold_hz: 1.0,
            initial_fatigue: 0.05,
            flight_fatigue_rate_per_second: 0.020,
            support_recovery_rate_per_second: 0.060,
            fatigue_landing_enter: 0.68,
            fatigue_takeoff_release: 0.25,
            minimum_rest_seconds: 0.0,
            minimum_grounded_seconds: 10.0,
            waypoint_min_seconds: 2.0,
            waypoint_max_seconds: 4.0,
            waypoint_margin_fraction: 0.18,
            exploration_steering_gain: 0.65,
            exploration_flight_speed_scale: 0.48,
        }
    }
}

impl HomeostasisParameters {
    pub fn validate(self) -> Result<Self> {
        let unit = [
            self.initial_hunger,
            self.hunger_enter,
            self.hunger_release,
            self.initial_fatigue,
            self.fatigue_landing_enter,
            self.fatigue_takeoff_release,
            self.waypoint_margin_fraction,
            self.exploration_steering_gain,
            self.exploration_flight_speed_scale,
        ];
        let positive = [
            self.hunger_rate_per_second,
            self.feeding_relief_per_second,
            self.feeding_extension_threshold,
            self.feeding_mn9_threshold_hz,
            self.flight_fatigue_rate_per_second,
            self.support_recovery_rate_per_second,
            self.minimum_grounded_seconds,
            self.waypoint_min_seconds,
            self.waypoint_max_seconds,
        ];
        if unit
            .into_iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || positive
                .into_iter()
                .any(|value| !value.is_finite() || value <= 0.0)
            || self.hunger_release >= self.hunger_enter
            || self.fatigue_takeoff_release >= self.fatigue_landing_enter
            || self.minimum_rest_seconds > self.minimum_grounded_seconds
            || !self.minimum_rest_seconds.is_finite() || self.minimum_rest_seconds<0.0
            || self.waypoint_min_seconds > self.waypoint_max_seconds
        {
            bail!("homeostasis parameters are invalid")
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct HomeostaticState {
    pub hunger: f64,
    pub fatigue: f64,
    pub hungry: bool,
    pub fatigue_landing_latched: bool,
    pub supported_seconds: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HomeostaticInput {
    pub dt_seconds: f64,
    pub flight_mode: FlightMode,
    pub contact_count: usize,
    pub support_contact: bool,
    pub horizontal_speed_mm_s: f64,
    pub flight_amplitude: f64,
    pub taste_active: bool,
    pub feeding_extension: f64,
    pub mn9_rate_hz: f64,
    pub motor_outputs_connected: bool,
    pub target_sensed: bool,
    pub position_mm: [f64; 3],
    pub forward_xy: [f64; 2],
    pub room_half_extents_mm: [f64; 3],
    pub planar_wall_clearance_mm: f64,
    pub collision_escape_active: bool,
}

impl Default for HomeostaticInput {
    fn default() -> Self {
        Self {
            dt_seconds: 0.0,
            flight_mode: FlightMode::Grounded,
            contact_count: 0,
            support_contact: false,
            horizontal_speed_mm_s: 0.0,
            flight_amplitude: 0.0,
            taste_active: false,
            feeding_extension: 0.0,
            mn9_rate_hz: 0.0,
            motor_outputs_connected: true,
            target_sensed: false,
            position_mm: [0.0; 3],
            forward_xy: [1.0, 0.0],
            room_half_extents_mm: [100.0; 3],
            planar_wall_clearance_mm: 100.0,
            collision_escape_active: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HomeostaticCommand {
    pub hunger: f64,
    pub fatigue: f64,
    pub hungry: bool,
    pub landing_request: bool,
    pub takeoff_inhibited: bool,
    pub resting: bool,
    pub exploration_steering: f64,
    pub feeding_gate: bool,
    pub flight_speed_scale: f64,
}

pub struct HomeostaticController {
    parameters: HomeostasisParameters,
    state: HomeostaticState,
    random_state: u64,
    waypoint_mm: [f64; 2],
    waypoint_seconds: f64,
    collision_was_active: bool,
    quiet_seconds: f64,
    quiet_choice: bool,
}

impl HomeostaticController {
    pub fn new(seed: u64, parameters: HomeostasisParameters) -> Result<Self> {
        let parameters = parameters.validate()?;
        Ok(Self {
            parameters,
            state: HomeostaticState {
                hunger: parameters.initial_hunger,
                fatigue: parameters.initial_fatigue,
                hungry: parameters.initial_hunger >= parameters.hunger_enter,
                ..HomeostaticState::default()
            },
            random_state: seed,
            waypoint_mm: [0.0; 2],
            waypoint_seconds: 0.0,
            collision_was_active: false,
            quiet_seconds: 0.0,
            quiet_choice: false,
        })
    }

    pub fn reset(&mut self, seed: u64) {
        *self = Self::new(seed, self.parameters).expect("existing parameters are valid");
    }

    pub fn state(&self) -> HomeostaticState {
        self.state
    }

    pub fn set_initial_hunger(&mut self, hunger: f64) -> Result<()> {
        if !hunger.is_finite() || !(0.0..=1.0).contains(&hunger) {
            bail!("initial hunger must be in [0,1]")
        }
        self.state.hunger = hunger;
        self.state.hungry = hunger >= self.parameters.hunger_enter;
        self.parameters.initial_hunger=hunger;
        Ok(())
    }

    pub fn interrupt_exploration(&mut self) {
        self.waypoint_seconds = 0.0;
    }

    pub fn update(&mut self, input: HomeostaticInput) -> Result<HomeostaticCommand> {
        validate_input(input)?;
        if input.dt_seconds == 0.0 {
            return Ok(self.command(input, 0.0));
        }

        let feeding = input.motor_outputs_connected
            && input.flight_mode==FlightMode::Grounded && input.contact_count>=2
            && input.taste_active
            && input.support_contact
            && input.feeding_extension >= self.parameters.feeding_extension_threshold
            && input.mn9_rate_hz >= self.parameters.feeding_mn9_threshold_hz;
        let hunger_delta = if feeding {
            -self.parameters.feeding_relief_per_second * input.dt_seconds
        } else {
            self.parameters.hunger_rate_per_second * input.dt_seconds
        };
        self.state.hunger = (self.state.hunger + hunger_delta).clamp(0.0, 1.0);
        if self.state.hungry && self.state.hunger <= self.parameters.hunger_release {
            self.state.hungry = false;
        } else if !self.state.hungry && self.state.hunger >= self.parameters.hunger_enter {
            self.state.hungry = true;
        }

        let airborne = input.flight_mode != FlightMode::Grounded;
        let powered_flight = airborne && input.flight_amplitude > 0.1;
        let stable_support = !airborne && input.contact_count >= 3;
        if powered_flight {
            // Holding the body aloft has a cost even at zero horizontal velocity.
            let effort = 0.75 + 0.25*input.flight_amplitude.clamp(0.0,1.0);
            self.state.fatigue = (self.state.fatigue
                + self.parameters.flight_fatigue_rate_per_second * effort * input.dt_seconds)
                .clamp(0.0, 1.0);
        } else if stable_support {
            self.state.fatigue = (self.state.fatigue
                - self.parameters.support_recovery_rate_per_second * input.dt_seconds)
                .clamp(0.0, 1.0);
        }

        if airborne && self.state.fatigue >= self.parameters.fatigue_landing_enter {
            self.state.fatigue_landing_latched = true;
            self.state.supported_seconds = 0.0;
        }
        if self.state.fatigue_landing_latched && stable_support {
            self.state.supported_seconds += input.dt_seconds;
            if self.state.supported_seconds >= self.parameters.minimum_grounded_seconds
                && self.state.fatigue <= self.parameters.fatigue_takeoff_release
            {
                self.state.fatigue_landing_latched = false;
                self.state.supported_seconds = 0.0;
            }
        } else if airborne {
            self.state.supported_seconds = 0.0;
        } else if self.state.fatigue_landing_latched {
            self.state.supported_seconds =
                (self.state.supported_seconds - input.dt_seconds).max(0.0);
        }

        if self.state.fatigue_landing_latched && !airborne && !self.state.hungry {
            self.quiet_seconds=(self.quiet_seconds-input.dt_seconds).max(0.0);
            if self.quiet_seconds==0.0 && stable_support {
                self.quiet_seconds=2.0+2.0*self.next_unit();
                self.quiet_choice=self.next_unit()<0.5;
            }
        } else {self.quiet_seconds=0.0;self.quiet_choice=false;}

        self.waypoint_seconds = (self.waypoint_seconds - input.dt_seconds).max(0.0);
        let collision_started = input.collision_escape_active && !self.collision_was_active;
        self.collision_was_active = input.collision_escape_active;
        let dx = self.waypoint_mm[0] - input.position_mm[0];
        let dy = self.waypoint_mm[1] - input.position_mm[1];
        if self.waypoint_seconds == 0.0 || collision_started || dx.hypot(dy) < 15.0 {
            self.select_waypoint(input.room_half_extents_mm);
        }
        let exploration_steering = if input.motor_outputs_connected
            && !input.target_sensed
            && !self.resting(input.flight_mode)
        {
            waypoint_steering(
                input.position_mm,
                input.forward_xy,
                if self.state.fatigue_landing_latched && airborne {
                    [0.0, 0.0]
                } else {
                    self.waypoint_mm
                },
                self.parameters.exploration_steering_gain,
            )
        } else {
            0.0
        };
        Ok(self.command(input, exploration_steering))
    }

    fn command(&self, input: HomeostaticInput, exploration_steering: f64) -> HomeostaticCommand {
        HomeostaticCommand {
            hunger: self.state.hunger,
            fatigue: self.state.fatigue,
            hungry: self.state.hungry,
            landing_request: self.state.fatigue_landing_latched
                && input.flight_mode != FlightMode::Grounded
                && input.planar_wall_clearance_mm >= 30.0
                && !input.collision_escape_active,
            takeoff_inhibited: self.state.fatigue_landing_latched
                || (self.state.hungry
                    && input.target_sensed
                    && input.flight_mode == FlightMode::Grounded),
            resting: self.resting(input.flight_mode),
            exploration_steering,
            feeding_gate: self.state.hungry,
            flight_speed_scale: if input.target_sensed {
                1.0
            } else {
                self.parameters.exploration_flight_speed_scale
            },
        }
    }

    fn resting(&self, flight_mode: FlightMode) -> bool {
        self.state.fatigue_landing_latched
            && flight_mode == FlightMode::Grounded
            && (self.state.supported_seconds < self.parameters.minimum_rest_seconds || self.quiet_choice)
    }

    fn select_waypoint(&mut self, room: [f64; 3]) {
        let usable = 1.0 - self.parameters.waypoint_margin_fraction;
        self.waypoint_mm = [
            (self.next_unit() * 2.0 - 1.0) * room[0] * usable,
            (self.next_unit() * 2.0 - 1.0) * room[1] * usable,
        ];
        self.waypoint_seconds = self.parameters.waypoint_min_seconds
            + self.next_unit()
                * (self.parameters.waypoint_max_seconds - self.parameters.waypoint_min_seconds);
    }

    fn next_unit(&mut self) -> f64 {
        self.random_state = self
            .random_state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        f64::from((self.random_state >> 32) as u32) / f64::from(u32::MAX)
    }
}

fn waypoint_steering(position: [f64; 3], forward: [f64; 2], waypoint: [f64; 2], gain: f64) -> f64 {
    let target = [waypoint[0] - position[0], waypoint[1] - position[1]];
    let cross = forward[0] * target[1] - forward[1] * target[0];
    let dot = forward[0] * target[0] + forward[1] * target[1];
    (gain * cross.atan2(dot) / PI).clamp(-gain, gain)
}

fn validate_input(input: HomeostaticInput) -> Result<()> {
    if !input.dt_seconds.is_finite()
        || input.dt_seconds < 0.0
        || !input.horizontal_speed_mm_s.is_finite()
        || input.horizontal_speed_mm_s < 0.0
        || !input.flight_amplitude.is_finite()
        || input.flight_amplitude < 0.0
        || !input.feeding_extension.is_finite()
        || input.feeding_extension < 0.0
        || !input.mn9_rate_hz.is_finite()
        || input.mn9_rate_hz < 0.0
        || input.position_mm.iter().any(|value| !value.is_finite())
        || input.forward_xy.iter().any(|value| !value.is_finite())
        || !input.planar_wall_clearance_mm.is_finite()
        || input.planar_wall_clearance_mm < 0.0
        || input
            .room_half_extents_mm
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        bail!("homeostasis input is invalid")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller() -> HomeostaticController {
        HomeostaticController::new(7, HomeostasisParameters::default()).unwrap()
    }

    #[test]
    fn hunger_requires_taste_extension_mn9_contact_and_connected_output() {
        let valid = HomeostaticInput {
            dt_seconds: 1.0,
            taste_active: true,
            feeding_extension: 0.5,
            mn9_rate_hz: 20.0,
            contact_count: 4,
            support_contact: true,
            ..HomeostaticInput::default()
        };
        let baseline = controller().update(valid).unwrap().hunger;
        assert!(baseline < HomeostasisParameters::default().initial_hunger);
        for invalid in [
            HomeostaticInput {flight_mode:FlightMode::Cruise,..valid},
            HomeostaticInput {contact_count:1,..valid},
            HomeostaticInput {
                taste_active: false,
                ..valid
            },
            HomeostaticInput {
                feeding_extension: 0.0,
                ..valid
            },
            HomeostaticInput {
                mn9_rate_hz: 0.0,
                ..valid
            },
            HomeostaticInput {
                support_contact: false,
                ..valid
            },
            HomeostaticInput {
                motor_outputs_connected: false,
                ..valid
            },
        ] {
            assert!(
                controller().update(invalid).unwrap().hunger
                    > HomeostasisParameters::default().initial_hunger
            );
        }
    }

    #[test]
    fn fatigue_rises_in_powered_flight_and_recovers_on_stable_support() {
        let mut state = controller();
        let before = state.state().fatigue;
        let flight = HomeostaticInput {
            dt_seconds: 1.0,
            flight_mode: FlightMode::Cruise,
            horizontal_speed_mm_s: 200.0,
            flight_amplitude: 0.9,
            ..HomeostaticInput::default()
        };
        let after_flight = state.update(flight).unwrap().fatigue;
        assert!(after_flight > before);
        let after_rest = state
            .update(HomeostaticInput {
                dt_seconds: 1.0,
                contact_count: 4,
                ..HomeostaticInput::default()
            })
            .unwrap()
            .fatigue;
        assert!(after_rest < after_flight);
    }

    #[test]
    fn fatigue_latch_requests_landing_and_allows_walking_during_recovery() {
        let mut state = controller();
        let flight = HomeostaticInput {
            dt_seconds: 1.0,
            flight_mode: FlightMode::Cruise,
            horizontal_speed_mm_s: 300.0,
            flight_amplitude: 1.0,
            ..HomeostaticInput::default()
        };
        let mut command = HomeostaticCommand::default();
        for _ in 0..40 {
            command = state.update(flight).unwrap();
        }
        assert!(command.landing_request && command.takeoff_inhibited);
        let support = HomeostaticInput {
            dt_seconds: 0.5,
            contact_count: 4,
            ..HomeostaticInput::default()
        };
        for _ in 0..6 {
            command = state.update(support).unwrap();
            assert!(command.takeoff_inhibited);
            assert!(!command.resting,"hungry recovery must not force the old 3.5-second rest");
        }
        for _ in 0..20 {
            command = state.update(support).unwrap();
        }
        assert!(!command.takeoff_inhibited);
    }

    #[test]
    fn registered_hunger_and_flight_fatigue_time_fixtures() {
        let mut hungry=HomeostaticController::new(11,HomeostasisParameters {initial_hunger:0.30,..Default::default()}).unwrap();
        let mut onset=None;
        for i in 1..=30002 {
            if hungry.update(HomeostaticInput {dt_seconds:0.002,..Default::default()}).unwrap().hungry {onset=Some(i as f64*0.002);break;}
        }
        assert!((onset.unwrap()-60.0).abs()<=0.002001,"hunger onset {onset:?}");
        for speed in [60.0,0.0,0.5] {
            let mut state=controller();
            let mut fatigue_onset=None;
            for i in 1..=22501 {
                let before=state.state().fatigue;
                let command=state.update(HomeostaticInput {dt_seconds:0.002,flight_mode:FlightMode::Cruise,
                    horizontal_speed_mm_s:speed,flight_amplitude:0.85,..Default::default()}).unwrap();
                assert!(command.fatigue>before);
                if command.landing_request {fatigue_onset=Some(i as f64*0.002);break;}
            }
            let time=fatigue_onset.unwrap();
            assert!((30.0..=45.0).contains(&time));
            eprintln!("speed={speed} mm/s: fatigue threshold at {time}s");
        }
        for speed in [0.0,4.0] {
            let mut state=HomeostaticController::new(11,HomeostasisParameters {initial_hunger:0.3,initial_fatigue:0.72,..Default::default()}).unwrap();
            state.state.fatigue_landing_latched=true;
            let mut release=None;
            for i in 1..=5501 {
                let before=state.state().fatigue;
                let command=state.update(HomeostaticInput {dt_seconds:0.002,contact_count:4,
                    horizontal_speed_mm_s:speed,..Default::default()}).unwrap();
                assert!(command.fatigue<=before);
                if !command.takeoff_inhibited {release=Some(i as f64*0.002);break;}
            }
            assert!((9.0..=11.0).contains(&release.unwrap()));
            eprintln!("supported speed={speed} mm/s: flight eligibility at {release:?}s");
        }
        let mut falling=controller();
        let before=falling.state().fatigue;
        falling.update(HomeostaticInput {dt_seconds:5.0,flight_mode:FlightMode::Landing,
            contact_count:0,flight_amplitude:0.0,..Default::default()}).unwrap();
        assert_eq!(falling.state().fatigue,before);
    }

    #[test]
    fn custom_initial_hunger_survives_reset() {
        let mut state=controller();
        state.set_initial_hunger(0.3).unwrap();
        state.update(HomeostaticInput {dt_seconds:10.0,..Default::default()}).unwrap();
        assert!(state.state().hunger>0.3);
        state.reset(7);
        assert_eq!(state.state().hunger,0.3);
    }

    #[test]
    fn pause_reset_dt_and_seed_are_deterministic_and_serializable() {
        let mut a = controller();
        let paused = a.state();
        a.update(HomeostaticInput::default()).unwrap();
        assert_eq!(a.state(), paused);
        let input = HomeostaticInput {
            dt_seconds: 0.1,
            position_mm: [10.0, -5.0, 2.0],
            forward_xy: [0.0, 1.0],
            ..HomeostaticInput::default()
        };
        let mut b = controller();
        assert_eq!(a.update(input).unwrap(), b.update(input).unwrap());
        a.reset(7);
        b.reset(7);
        assert_eq!(a.update(input).unwrap(), b.update(input).unwrap());
        let json = serde_json::to_string(&a.state()).unwrap();
        assert_eq!(
            serde_json::from_str::<HomeostaticState>(&json).unwrap(),
            a.state()
        );

        let mut fine = controller();
        let mut coarse = controller();
        for _ in 0..10 {
            fine.update(HomeostaticInput {
                dt_seconds: 0.1,
                ..input
            })
            .unwrap();
        }
        coarse
            .update(HomeostaticInput {
                dt_seconds: 1.0,
                ..input
            })
            .unwrap();
        assert!((fine.state().hunger - coarse.state().hunger).abs() < 1e-12);
        assert!(fine.state().hunger <= 1.0 && fine.state().fatigue <= 1.0);
    }
}
