use std::f64::consts::TAU;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub const GROOMING_LEG_COUNT: usize = 6;
pub const GROOMING_CONTROL_COUNT: usize = 42;
pub const GROOMING_MIN_SUPPORT_LEGS: usize = 4;
pub const GROOMING_BOUT_DURATION_SECONDS: f64 = 4.0;

// Preserve the proven 2.5-second lift/support/head/return timing, inserting
// 1.5 seconds only in the forefoot-rubbing portion (not in load transfer).
fn motion_phase(elapsed_seconds: f64) -> f64 {
    let inserted = (elapsed_seconds - 1.05).clamp(0.0, 1.5);
    ((elapsed_seconds - inserted) / 2.5).clamp(0.0, 1.0)
}
const DEFAULT_GROOMING_SEED:u64=0x6a00_2026_0913;
pub const GROOMING_FALLBACK_INTERVAL_SECONDS: f64 = 8.0;
const GROOMING_SUPPORT_LOSS_GRACE_SECONDS: f64 = 0.05;
const BRUSH_FREQUENCY_HZ: f64 = 8.0;
const JOINTS_PER_LEG: usize = 7;
#[allow(clippy::approx_constant)]
const JOINT_CONTROL_LIMIT_RAD: f64 = 3.14;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct GroomingParameters {
    pub initial_dirt: f64,
    pub passive_dirt_rate_per_second: f64,
    pub walking_dirt_rate_per_second: f64,
    pub flight_dirt_rate_per_second: f64,
    pub environment_dirt_rate_per_second: f64,
    pub start_threshold: f64,
    pub release_threshold: f64,
    pub stable_support_seconds: f64,
    pub neural_gate_rate_hz: f64,
    pub neural_evidence_hold_seconds: f64,
    pub completed_cleaning_fraction: f64,
}

impl Default for GroomingParameters {
    fn default() -> Self {
        Self {
            initial_dirt: 0.10,
            passive_dirt_rate_per_second: 0.025,
            walking_dirt_rate_per_second: 0.0,
            flight_dirt_rate_per_second: 0.0,
            environment_dirt_rate_per_second: 0.0,
            start_threshold: 0.40,
            release_threshold: 0.25,
            stable_support_seconds: 0.50,
            neural_gate_rate_hz: 0.10,
            neural_evidence_hold_seconds: 12.0,
            completed_cleaning_fraction: 0.90,
        }
    }
}

impl GroomingParameters {
    pub fn validate(self) -> Result<Self> {
        let unit = [
            self.initial_dirt,
            self.start_threshold,
            self.release_threshold,
            self.completed_cleaning_fraction,
        ];
        let nonnegative = [
            self.passive_dirt_rate_per_second,
            self.walking_dirt_rate_per_second,
            self.flight_dirt_rate_per_second,
            self.environment_dirt_rate_per_second,
            self.neural_gate_rate_hz,
        ];
        if unit
            .into_iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || nonnegative
                .into_iter()
                .any(|value| !value.is_finite() || value < 0.0)
            || !self.stable_support_seconds.is_finite()
            || self.stable_support_seconds <= 0.0
            || !self.neural_evidence_hold_seconds.is_finite()
            || self.neural_evidence_hold_seconds <= 0.0
            || self.release_threshold >= self.start_threshold
            || self.completed_cleaning_fraction <= 0.0
        {
            bail!("grooming parameters are invalid")
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct GroomingState {
    pub dirt: f64,
    pub stable_support_seconds: f64,
    pub neural_evidence_seconds: f64,
    pub completed_bouts: u64,
    pub interrupted_bouts: u64,
    pub opportunity_count:u64,
    pub last_opportunity_seconds:Option<f64>,
    pub last_start_seconds:Option<f64>,
    pub last_complete_seconds:Option<f64>,
    pub cooldown_seconds:f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroomingMode {
    #[default]
    None,
    AntennaBilateral,
    AntennaLeft,
    AntennaRight,
}

impl GroomingMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::AntennaBilateral => "ANTENNA-BOTH",
            Self::AntennaLeft => "ANTENNA-L",
            Self::AntennaRight => "ANTENNA-R",
        }
    }

    fn active_front_legs(self) -> [bool; 2] {
        match self {
            Self::None => [false, false],
            Self::AntennaBilateral => [true, true],
            Self::AntennaLeft => [true, false],
            Self::AntennaRight => [false, true],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GroomingTrigger {
    #[default]
    None,
    Manual,
    Fallback,
    Autonomous,
}

impl GroomingTrigger {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Manual => "manual",
            Self::Fallback => "fallback",
            Self::Autonomous => "autonomous",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroomingInput {
    pub dt_seconds: f64,
    pub grounded: bool,
    pub on_ground_surface: bool,
    pub contact_count: usize,
    /// Runtime physical preparation gate; synthetic controller fixtures may omit it.
    pub preparation_pending: bool,
    /// Actual middle/hind support, when supplied by the physical world.
    pub support_legs: Option<usize>,
    pub allow_fallback: bool,
    pub taste_active: bool,
    pub taste_valence: f64,
    pub feeding_extension: f64,
    pub hungry: bool,
    pub horizontal_speed_mm_s: f64,
    pub airborne_powered: bool,
    pub environment_contact_count: usize,
    pub grooming_probe_rate_hz: f64,
    pub neural_outputs_connected: bool,
    pub collision_danger: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GroomingCommand {
    pub mode: GroomingMode,
    pub trigger: GroomingTrigger,
    pub active: bool,
    pub phase: f64,
    pub support_leg_count: usize,
    pub dirt: f64,
    pub stable_support_seconds: f64,
    pub neural_gate_active: bool,
    pub completed_bouts: u64,
    pub interrupted_bouts: u64,
}

pub struct GroomingController {
    parameters: GroomingParameters,
    state: GroomingState,
    mode: GroomingMode,
    trigger: GroomingTrigger,
    elapsed_seconds: f64,
    support_loss_elapsed_seconds: f64,
    fallback_elapsed_seconds: f64,
    pending_manual: bool,
    waiting_for_support: bool,
    next_fallback_left: bool,
    autonomous_latched: bool,
    stable_support_loss_seconds: f64,
    seed:u64,
    random_state:u64,
    total_seconds:f64,
    urge_rate:f64,
}

impl GroomingController {
    pub fn new() -> Self {
        Self::with_parameters(GroomingParameters::default()).expect("default grooming parameters")
    }

    pub fn with_parameters(parameters: GroomingParameters) -> Result<Self> {
        Self::with_seed(parameters,DEFAULT_GROOMING_SEED)
    }

    pub fn with_seed(parameters:GroomingParameters,seed:u64)->Result<Self> {
        let parameters = parameters.validate()?;
        let mut controller=Self {
            parameters,
            state: GroomingState {
                dirt: parameters.initial_dirt,
                ..GroomingState::default()
            },
            mode: GroomingMode::None,
            trigger: GroomingTrigger::None,
            elapsed_seconds: 0.0,
            support_loss_elapsed_seconds: 0.0,
            fallback_elapsed_seconds: 0.0,
            pending_manual: false,
            waiting_for_support: false,
            next_fallback_left: true,
            autonomous_latched: false,
            stable_support_loss_seconds: 0.0,
            seed,random_state:seed,total_seconds:0.0,urge_rate:0.0,
        };
        controller.choose_interval(true);
        Ok(controller)
    }

    pub fn reset(&mut self) {
        *self = Self::with_seed(self.parameters,self.seed).expect("existing parameters are valid");
    }

    pub fn set_seed(&mut self,seed:u64) {
        *self=Self::with_seed(self.parameters,seed).expect("existing parameters are valid");
    }

    pub fn set_initial_dirt(&mut self, dirt: f64) -> Result<()> {
        if !dirt.is_finite() || !(0.0..=1.0).contains(&dirt) || self.elapsed_seconds > 0.0 {
            bail!("initial dirt must be in [0,1] before a bout starts")
        }
        self.parameters.initial_dirt=dirt;
        self.reset();
        Ok(())
    }

    pub fn state(&self) -> GroomingState {
        self.state
    }

    pub fn autonomous_intent(&self, satiated: bool) -> bool {
        satiated && (self.active() || (self.autonomous_latched
            && self.state.cooldown_seconds==0.0 && self.state.neural_evidence_seconds>0.0))
    }

    pub fn request_manual(&mut self) {
        self.pending_manual = true;
    }

    pub fn preparing(&self) -> bool {
        self.waiting_for_support
    }

    pub fn update(&mut self, input: GroomingInput) -> Result<GroomingCommand> {
        validate_input(input)?;
        if input.dt_seconds == 0.0 {
            return Ok(self.command());
        }
        self.total_seconds+=input.dt_seconds;
        self.state.cooldown_seconds=(self.state.cooldown_seconds-input.dt_seconds).max(0.0);
        self.accumulate_dirt(input);
        if input.neural_outputs_connected
            && input.grooming_probe_rate_hz >= self.parameters.neural_gate_rate_hz
        {
            self.state.neural_evidence_seconds = self.parameters.neural_evidence_hold_seconds;
        } else {
            self.state.neural_evidence_seconds =
                (self.state.neural_evidence_seconds - input.dt_seconds).max(0.0);
        }
        let safe = input.grounded
            && input.on_ground_surface
            && !input.collision_danger
            && !input.taste_active
            && input.taste_valence <= 0.0
            && input.feeding_extension <= 0.01;
        let autonomous_safe = safe && !input.hungry;
        let stable_support = autonomous_safe
            && input.contact_count >= GROOMING_MIN_SUPPORT_LEGS
            && input.horizontal_speed_mm_s <= 2.0;
        if stable_support {
            self.stable_support_loss_seconds = 0.0;
            self.state.stable_support_seconds += input.dt_seconds;
        } else if autonomous_safe && input.contact_count > 0 {
            self.stable_support_loss_seconds += input.dt_seconds;
            if self.stable_support_loss_seconds >= GROOMING_SUPPORT_LOSS_GRACE_SECONDS {
                self.state.stable_support_seconds = 0.0;
            }
        } else {
            self.stable_support_loss_seconds = 0.0;
            self.state.stable_support_seconds = 0.0;
        }
        if self.state.dirt >= self.parameters.start_threshold {
            if !self.autonomous_latched {
                self.state.opportunity_count+=1;
                self.state.last_opportunity_seconds=Some(self.total_seconds);
            }
            self.autonomous_latched = true;
        } else if self.state.dirt <= self.parameters.release_threshold {
            self.autonomous_latched = false;
        }

        if self.active() {
            self.waiting_for_support = false;
            let next_phase=motion_phase(self.elapsed_seconds+input.dt_seconds);
            if let Some(support)=input.support_legs {
                if input.contact_count<4 || ((0.15..=0.90).contains(&next_phase)&&support<4) {
                    self.abort();
                    return Ok(self.command());
                }
            }
            if !safe || (self.trigger == GroomingTrigger::Autonomous
                && (input.hungry || !input.neural_outputs_connected)) {
                self.abort();
                return Ok(self.command());
            }
            if input.contact_count < GROOMING_MIN_SUPPORT_LEGS {
                self.support_loss_elapsed_seconds += input.dt_seconds;
                if self.support_loss_elapsed_seconds >= GROOMING_SUPPORT_LOSS_GRACE_SECONDS {
                    self.abort();
                    return Ok(self.command());
                }
            } else {
                self.support_loss_elapsed_seconds = 0.0;
            }
            self.elapsed_seconds += input.dt_seconds;
            if self.elapsed_seconds + 1e-9 >= GROOMING_BOUT_DURATION_SECONDS {
                self.complete_bout();
            }
        } else if safe {
            if input.allow_fallback {
                self.fallback_elapsed_seconds += input.dt_seconds;
            } else {
                self.fallback_elapsed_seconds = 0.0;
            }
            let bout_requested = self.pending_manual
                || (input.allow_fallback
                    && self.fallback_elapsed_seconds + 1e-9 >= GROOMING_FALLBACK_INTERVAL_SECONDS)
                || (autonomous_safe
                    && self.autonomous_latched
                    && self.state.cooldown_seconds==0.0
                    && self.state.stable_support_seconds + 1e-9
                        >= self.parameters.stable_support_seconds
                    && self.state.neural_evidence_seconds > 0.0
                    && input.neural_outputs_connected);
            self.waiting_for_support = bout_requested;
            if bout_requested && !input.preparation_pending && input.contact_count >= GROOMING_MIN_SUPPORT_LEGS {
                let manual = self.pending_manual;
                self.pending_manual = false;
                let (mode, trigger) = if manual {
                    (GroomingMode::AntennaBilateral, GroomingTrigger::Manual)
                } else if autonomous_safe
                    && self.autonomous_latched
                    && self.state.neural_evidence_seconds > 0.0
                    && input.neural_outputs_connected
                {
                    (GroomingMode::AntennaBilateral, GroomingTrigger::Autonomous)
                } else {
                    let mode = if self.next_fallback_left {
                        GroomingMode::AntennaLeft
                    } else {
                        GroomingMode::AntennaRight
                    };
                    self.next_fallback_left = !self.next_fallback_left;
                    (mode, GroomingTrigger::Fallback)
                };
                self.start_bout(mode, trigger);
            }
        } else {
            self.fallback_elapsed_seconds = 0.0;
            self.waiting_for_support = false;
        }
        Ok(self.command())
    }

    fn accumulate_dirt(&mut self, input: GroomingInput) {
        let mut rate = self.urge_rate;
        if input.airborne_powered {
            rate += self.parameters.flight_dirt_rate_per_second;
        } else if input.grounded && input.horizontal_speed_mm_s > 0.5 {
            rate += self.parameters.walking_dirt_rate_per_second;
        }
        if input.environment_contact_count > 0 {
            rate += self.parameters.environment_dirt_rate_per_second
                * input.environment_contact_count.min(4) as f64;
        }
        self.state.dirt = (self.state.dirt + rate * input.dt_seconds).clamp(0.0, 1.0);
    }

    pub fn apply(
        &self,
        joint_controls: &mut [f64; GROOMING_CONTROL_COUNT],
        adhesion: &mut [f64; GROOMING_LEG_COUNT],
    ) {
        if !self.active() {
            return;
        }

        let active_front_legs = self.mode.active_front_legs();
        let phase=motion_phase(self.elapsed_seconds);
        for (leg_index, active) in active_front_legs.into_iter().enumerate() {
            if active {
                // Transfer load gradually as the forefeet lift, avoiding a
                // discontinuous adhesion impulse that unloads a support foot.
                adhesion[leg_index * 3] = if phase<0.20 {((0.20-phase)/0.05).clamp(0.0,1.0)}
                    else if phase>0.90 {((phase-0.90)/0.05).clamp(0.0,1.0)}else{0.0};
            }
        }
        for leg_index in [1, 2, 4, 5] {
            adhesion[leg_index] = 1.0;
        }

        // Move the middle feet forward before unloading the forelegs: the
        // ordinary six-foot stance leaves the four-foot support polygon aft of COM.
        // Finish repositioning before the foreleg adhesion is released at 0.15,
        // leaving 175 ms for the support feet to settle under their real contacts.
        let stance=(phase/0.08).clamp(0.0,1.0)*((1.0-phase)/0.10).clamp(0.0,1.0);
        joint_controls[JOINTS_PER_LEG+2]-=0.65*stance;
        joint_controls[4*JOINTS_PER_LEG+2]-=0.65*stance;
        // Offline inverse-kinematic fits to the unchanged real body, including
        // its spring/actuator equilibrium (tools/fit_grooming_keyframes.py).
        // Both tips reach in front of the face along a shared sagittal line.
        // Slide along that line, rather than sweeping the feet sideways apart.
        let rub_near=[-0.429847,-0.546259,-0.630746,-0.534366,0.625335,0.315598,0.393899];
        let rub_far=[-0.369316,-0.507093,-0.645774,-0.228465,0.618963,0.047549,0.107292];
        let lifted_pose=[-0.879945,-1.327075,-0.553451,-0.713990,0.733120,0.788068,1.574208];
        // Aim outside the actual eye mesh, not its body-frame origin inside the head.
        let head_pose=[-1.077356,-1.912542,-0.494815,-0.713990,0.761204,0.287024,2.107879];
        let smooth=|x:f64| {let t=x.clamp(0.0,1.0);t*t*(3.0-2.0*t)};
        let raised=smooth((phase-0.15)/0.10)*(1.0-smooth((phase-0.85)/0.10));
        let head=smooth((phase-0.42)/0.12);
        // Unload and lift with the proven folded pose before reaching forward;
        // dragging a straightened foreleg while adhesion remains active trips
        // the four-foot support guard.
        let reach=smooth((phase-0.25)/0.15);
        let brush = (TAU * BRUSH_FREQUENCY_HZ * self.elapsed_seconds).sin();
        for (front_index, active) in active_front_legs.into_iter().enumerate() {
            if !active {
                continue;
            }
            let leg_index = if front_index == 0 { 0 } else { 3 };
            let base = leg_index * JOINTS_PER_LEG;
            let direction=if front_index==0 {1.0}else{-1.0};
            let slide=0.5+0.5*direction*(TAU*5.0*self.elapsed_seconds).sin();
            // The right model joint axes are already mirrored. Equal joint
            // coordinates produce bilateral mirror poses; do not negate them again.
            for joint in 0..JOINTS_PER_LEG {
                let lifted=lifted_pose[joint]+match joint {2=>0.12*direction*brush,6=>0.20*direction*brush,_=>0.0};
                let extended=rub_near[joint]+slide*(rub_far[joint]-rub_near[joint]);
                let rub=lifted+reach*(extended-lifted);
                joint_controls[base+joint]+=raised*((1.0-head)*rub+head*head_pose[joint]);
            }
            // The established eye-brush pose and stroke remain unchanged.
            joint_controls[base+2]+=0.04*head*brush*raised;
            joint_controls[base+6]+=0.04*head*brush*raised;
            for control in &mut joint_controls[base..base + JOINTS_PER_LEG] {
                *control = control.clamp(-JOINT_CONTROL_LIMIT_RAD, JOINT_CONTROL_LIMIT_RAD);
            }
        }
        for control in joint_controls.iter_mut() {*control=control.clamp(-JOINT_CONTROL_LIMIT_RAD,JOINT_CONTROL_LIMIT_RAD);}
    }

    fn active(&self) -> bool {
        self.mode != GroomingMode::None
    }

    fn phase(&self) -> f64 {
        (self.elapsed_seconds / GROOMING_BOUT_DURATION_SECONDS).clamp(0.0, 1.0)
    }

    fn command(&self) -> GroomingCommand {
        let active = self.active();
        let active_front_legs = self.mode.active_front_legs();
        let active_front_count = active_front_legs
            .into_iter()
            .filter(|active| *active)
            .count();
        GroomingCommand {
            mode: self.mode,
            trigger: self.trigger,
            active,
            phase: if active { self.phase() } else { 0.0 },
            support_leg_count: if active {
                GROOMING_LEG_COUNT - active_front_count
            } else {
                0
            },
            dirt: self.state.dirt,
            stable_support_seconds: self.state.stable_support_seconds,
            neural_gate_active: self.state.neural_evidence_seconds > 0.0,
            completed_bouts: self.state.completed_bouts,
            interrupted_bouts: self.state.interrupted_bouts,
        }
    }

    fn start_bout(&mut self, mode: GroomingMode, trigger: GroomingTrigger) {
        self.mode = mode;
        self.trigger = trigger;
        self.elapsed_seconds = 0.0;
        self.support_loss_elapsed_seconds = 0.0;
        self.fallback_elapsed_seconds = 0.0;
        self.waiting_for_support = false;
        if trigger==GroomingTrigger::Autonomous {self.state.last_start_seconds=Some(self.total_seconds);}
    }

    fn complete_bout(&mut self) {
        if self.trigger == GroomingTrigger::Autonomous {
            self.state.dirt *= 1.0 - self.parameters.completed_cleaning_fraction;
            self.state.completed_bouts += 1;
            self.state.last_complete_seconds=Some(self.total_seconds);
            self.state.cooldown_seconds=5.0;
            self.choose_interval(false);
            if self.state.dirt <= self.parameters.release_threshold {
                self.autonomous_latched = false;
            }
        }
        self.finish_bout();
    }

    fn finish_bout(&mut self) {
        self.mode = GroomingMode::None;
        self.trigger = GroomingTrigger::None;
        self.elapsed_seconds = 0.0;
        self.support_loss_elapsed_seconds = 0.0;
        self.waiting_for_support = false;
    }

    fn abort(&mut self) {
        if self.trigger == GroomingTrigger::Autonomous {
            self.state.interrupted_bouts += 1;
            self.state.cooldown_seconds=5.0;
        }
        self.finish_bout();
        self.fallback_elapsed_seconds = 0.0;
    }

    fn choose_interval(&mut self,first:bool) {
        self.random_state=self.random_state.wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let unit=(self.random_state>>32) as u32 as f64/u32::MAX as f64;
        let interval=10.0+unit*if first {5.0}else{10.0};
        self.urge_rate=(self.parameters.start_threshold-self.state.dirt).max(0.0)/interval
            * (self.parameters.passive_dirt_rate_per_second/0.025);
    }
}

impl Default for GroomingController {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_input(input: GroomingInput) -> Result<()> {
    if !input.dt_seconds.is_finite()
        || input.dt_seconds < 0.0
        || !(-1.0..=1.0).contains(&input.taste_valence)
        || !input.taste_valence.is_finite()
        || !input.feeding_extension.is_finite()
        || !(0.0..=1.0).contains(&input.feeding_extension)
        || !input.horizontal_speed_mm_s.is_finite()
        || input.horizontal_speed_mm_s < 0.0
        || !input.grooming_probe_rate_hz.is_finite()
        || input.grooming_probe_rate_hz < 0.0
    {
        bail!("grooming input is invalid")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_opportunity_intervals_are_bounded_and_seeded() {
        fn schedule(seed:u64)->Vec<[f64;3]> {
            let mut controller=GroomingController::with_seed(GroomingParameters::default(),seed).unwrap();
            let mut events=Vec::new();
            for _ in 0..500_000 {
                let command=controller.update(autonomous_input(0.002)).unwrap();
                if command.completed_bouts as usize>events.len() {
                    let state=controller.state();
                    let event=[state.last_opportunity_seconds.unwrap(),state.last_start_seconds.unwrap(),state.last_complete_seconds.unwrap()];
                    let previous=events.last().map_or(0.0,|old:&[f64;3]|old[2]);
                    let interval=event[0]-previous;
                    let maximum=if events.is_empty(){15.002}else{20.002};
                    assert!((10.0..=maximum).contains(&interval),"seed {seed}: interval {interval}");
                    assert!((0.0..=2.0).contains(&(event[1]-event[0])));
                    assert!(((event[2]-event[1])-GROOMING_BOUT_DURATION_SECONDS).abs()<=0.0021);
                    assert_eq!(state.cooldown_seconds,5.0);
                    assert!(state.dirt<=controller.parameters.release_threshold);
                    events.push(event);
                    if events.len()==20 {break;}
                }
            }
            assert_eq!(events.len(),20);
            events
        }
        let mut previous=None;
        for seed in [11,13,17,19,23] {
            let events=schedule(seed);
            assert_eq!(events,schedule(seed));
            if let Some(old)=previous {assert_ne!(events,old);}
            let intervals=events.windows(2).map(|pair|pair[1][0]-pair[0][2]).collect::<Vec<_>>();
            assert!(intervals.iter().any(|t|(t-intervals[0]).abs()>0.01));
            eprintln!("seed {seed}: first opportunity {:.3}s, subsequent intervals {intervals:?}",events[0][0]);
            previous=Some(events);
        }
    }

    #[test]
    fn fixed_low_urge_and_delayed_opportunity_do_not_make_a_backlog() {
        let mut low=GroomingController::with_parameters(GroomingParameters {
            passive_dirt_rate_per_second:0.0,..Default::default()}).unwrap();
        for _ in 0..15_000 {assert!(!low.update(autonomous_input(0.002)).unwrap().active);}
        assert_eq!(low.state().opportunity_count,0);
        let mut delayed=GroomingController::new();
        for _ in 0..100_000 {
            assert!(!delayed.update(GroomingInput {hungry:true,grounded:false,..autonomous_input(0.002)}).unwrap().active);
        }
        assert_eq!(delayed.state().opportunity_count,1);
        for _ in 0..2350 {delayed.update(autonomous_input(0.002)).unwrap();}
        assert_eq!(delayed.state().completed_bouts,1);
        for _ in 0..2500 {assert!(!delayed.update(autonomous_input(0.002)).unwrap().active);}
        assert_eq!(delayed.state().completed_bouts,1);
    }

    #[test]
    fn physical_preparation_and_support_loss_cannot_receive_completion_credit() {
        let mut controller=GroomingController::new();controller.set_initial_dirt(0.9).unwrap();
        for _ in 0..500 {
            assert!(!controller.update(GroomingInput {preparation_pending:true,..autonomous_input(0.002)}).unwrap().active);
        }
        assert!(controller.update(autonomous_input(0.002)).unwrap().active);
        for _ in 0..200 {controller.update(GroomingInput {support_legs:Some(4),..autonomous_input(0.002)}).unwrap();}
        let before=controller.state().dirt;
        let result=controller.update(GroomingInput {support_legs:Some(3),..autonomous_input(0.002)}).unwrap();
        assert!(!result.active);assert_eq!(result.completed_bouts,0);
        assert_eq!(result.interrupted_bouts,1);assert!(result.dirt>=before);
    }

    #[test]
    fn disconnecting_autonomous_neural_gate_aborts_without_completion_credit() {
        let mut controller=GroomingController::new();controller.set_initial_dirt(0.9).unwrap();
        for _ in 0..300 {controller.update(autonomous_input(0.002)).unwrap();}
        assert!(controller.active());
        let before=controller.state().dirt;
        let command=controller.update(GroomingInput {neural_outputs_connected:false,..autonomous_input(0.002)}).unwrap();
        assert!(!command.active);assert_eq!(command.completed_bouts,0);
        assert_eq!(command.interrupted_bouts,1);assert!(command.dirt>=before);
    }

    fn input(dt_seconds: f64) -> GroomingInput {
        GroomingInput {
            dt_seconds,
            grounded: true,
            on_ground_surface: true,
            contact_count: 6,
            ..GroomingInput::default()
        }
    }

    fn autonomous_input(dt_seconds: f64) -> GroomingInput {
        GroomingInput {
            grooming_probe_rate_hz: 20.0,
            neural_outputs_connected: true,
            ..input(dt_seconds)
        }
    }

    #[test]
    fn manual_bout_is_grounded_only_and_keeps_four_support_legs() {
        let mut controller = GroomingController::new();
        controller.request_manual();
        let command = controller.update(input(0.01)).unwrap();
        assert_eq!(command.mode, GroomingMode::AntennaBilateral);
        assert_eq!(command.trigger, GroomingTrigger::Manual);
        assert_eq!(command.support_leg_count, 4);

        let mut controls = [0.0; GROOMING_CONTROL_COUNT];
        let mut adhesion = [0.0; GROOMING_LEG_COUNT];
        controller.update(input(0.5)).unwrap();
        controller.apply(&mut controls, &mut adhesion);
        assert_eq!(adhesion, [0.0, 1.0, 1.0, 0.0, 1.0, 1.0]);
        assert!(controls.iter().all(|control| control.is_finite()));
        assert!(
            controls.iter().all(
                |control| (-JOINT_CONTROL_LIMIT_RAD..=JOINT_CONTROL_LIMIT_RAD).contains(control)
            )
        );

        let airborne = GroomingInput {
            grounded: false,
            ..input(0.01)
        };
        assert!(!controller.update(airborne).unwrap().active);
    }

    #[test]
    fn taste_and_feeding_abort_a_bout() {
        let mut controller = GroomingController::new();
        controller.request_manual();
        assert!(controller.update(input(0.01)).unwrap().active);
        assert!(
            !controller
                .update(GroomingInput {
                    taste_active: true,
                    ..input(0.01)
                })
                .unwrap()
                .active
        );

        controller.request_manual();
        assert!(controller.update(input(0.01)).unwrap().active);
        assert!(
            !controller
                .update(GroomingInput {
                    feeding_extension: 0.5,
                    ..input(0.01)
                })
                .unwrap()
                .active
        );
    }

    #[test]
    fn unsafe_manual_request_waits_without_freezing_for_support() {
        let mut controller = GroomingController::new();
        controller.request_manual();
        let tasting = GroomingInput {
            taste_active: true,
            contact_count: 0,
            ..input(0.01)
        };
        assert!(!controller.update(tasting).unwrap().active);
        assert!(!controller.preparing());

        let unsupported = GroomingInput {
            contact_count: 2,
            ..input(0.01)
        };
        assert!(!controller.update(unsupported).unwrap().active);
        assert!(controller.preparing());

        let command = controller.update(input(0.01)).unwrap();
        assert!(command.active);
        assert_eq!(command.trigger, GroomingTrigger::Manual);
        assert!(!controller.preparing());
    }

    #[test]
    fn transient_support_loss_does_not_abort_a_bout() {
        let mut controller = GroomingController::new();
        controller.request_manual();
        assert!(controller.update(input(0.01)).unwrap().active);

        let unsupported = GroomingInput {
            contact_count: 3,
            ..input(0.01)
        };
        for _ in 0..4 {
            assert!(controller.update(unsupported).unwrap().active);
        }
        assert!(controller.update(input(0.01)).unwrap().active);
        for _ in 0..4 {
            assert!(controller.update(unsupported).unwrap().active);
        }
        assert!(!controller.update(unsupported).unwrap().active);
    }

    #[test]
    fn fallback_is_deterministic_and_alternates_sides() {
        let mut controller = GroomingController::new();
        let fallback_input = GroomingInput {
            allow_fallback: true,
            ..input(0.01)
        };
        for _ in 0..799 {
            let command = controller.update(fallback_input).unwrap();
            assert!(!command.active);
        }
        let mut command = controller.update(fallback_input).unwrap();
        assert_eq!(command.mode, GroomingMode::AntennaLeft);
        for _ in 0..(GROOMING_BOUT_DURATION_SECONDS/0.01).ceil() as usize {
            command = controller.update(fallback_input).unwrap();
        }
        assert!(!command.active);
        for _ in 0..800 {
            command = controller.update(fallback_input).unwrap();
        }
        assert_eq!(command.mode, GroomingMode::AntennaRight);
    }

    #[test]
    fn connected_brain_disables_idle_fallback_but_allows_manual_requests() {
        let mut controller = GroomingController::new();
        for _ in 0..801 {
            let command = controller.update(input(0.01)).unwrap();
            assert!(!command.active);
            assert_eq!(command.trigger, GroomingTrigger::None);
        }

        controller.request_manual();
        let command = controller.update(input(0.01)).unwrap();
        assert!(command.active);
        assert_eq!(command.trigger, GroomingTrigger::Manual);
    }

    #[test]
    fn paired_rub_keeps_mirrored_pose_but_counter_moves_the_feet() {
        let mut controller = GroomingController::new();
        controller.request_manual();
        controller.update(input(0.01)).unwrap();
        controller.update(input(0.65)).unwrap();
        let mut bilateral = [0.0; GROOMING_CONTROL_COUNT];
        let mut adhesion = [0.0; GROOMING_LEG_COUNT];
        controller.apply(&mut bilateral, &mut adhesion);
        assert!((bilateral[0] - bilateral[21]).abs() < 0.07);
        assert!((bilateral[6] - bilateral[27]).abs() > 0.05);
        assert!((bilateral[1] - bilateral[22]).abs() < 0.05);
        assert!(
            bilateral
                .iter()
                .all(|control| control.abs() <= JOINT_CONTROL_LIMIT_RAD)
        );
    }

    #[test]
    fn inserted_rub_preserves_support_transfer_and_provides_nearly_two_seconds() {
        assert!((motion_phase(0.375)-0.15).abs()<1e-12);
        assert!((motion_phase(0.625)-0.25).abs()<1e-12);
        for time in [1.05,1.5,2.0,2.55] {assert!((motion_phase(time)-0.42).abs()<1e-12);}
        assert!((2.55-0.625-1.925_f64).abs()<1e-12);
        assert!((motion_phase(2.85)-0.54).abs()<1e-12);
        assert!((motion_phase(3.875)-0.95).abs()<1e-12);
        assert_eq!(motion_phase(4.0),1.0);
    }

    #[test]
    fn joint_overlay_respects_the_model_ctrlrange_literal() {
        let mut controller = GroomingController::new();
        controller.request_manual();
        controller.update(input(0.01)).unwrap();
        controller.update(input(0.6)).unwrap();
        let mut controls = [JOINT_CONTROL_LIMIT_RAD; GROOMING_CONTROL_COUNT];
        let mut adhesion = [0.0; GROOMING_LEG_COUNT];
        controller.apply(&mut controls, &mut adhesion);
        assert!(
            controls.iter().all(
                |control| (-JOINT_CONTROL_LIMIT_RAD..=JOINT_CONTROL_LIMIT_RAD).contains(control)
            )
        );
        assert!(
            controls
                .iter()
                .any(|control| (*control - JOINT_CONTROL_LIMIT_RAD).abs() < 1e-12)
        );
    }

    #[test]
    fn custom_initial_urge_reset_replays_opportunities() {
        let mut controller=GroomingController::with_seed(GroomingParameters::default(),23).unwrap();
        controller.set_initial_dirt(0.2).unwrap();
        let initial=controller.state();
        let first=(0..10_000).map(|_|controller.update(autonomous_input(0.002)).unwrap()).collect::<Vec<_>>();
        controller.reset();
        assert_eq!(controller.state(),initial);
        for expected in first {assert_eq!(controller.update(autonomous_input(0.002)).unwrap(),expected);}
    }

    #[test]
    fn dirt_pause_reset_bounds_hysteresis_and_state_serialization() {
        let parameters = GroomingParameters::default();
        let mut controller = GroomingController::with_parameters(parameters).unwrap();
        let initial = controller.state();
        controller.update(autonomous_input(0.0)).unwrap();
        assert_eq!(controller.state(), initial);
        for _ in 0..20_000 {
            controller
                .update(GroomingInput {
                    airborne_powered: true,
                    grounded: false,
                    ..autonomous_input(0.01)
                })
                .unwrap();
        }
        assert_eq!(controller.state().dirt, 1.0);
        let encoded = serde_json::to_string(&controller.state()).unwrap();
        let decoded: GroomingState = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, controller.state());
        controller.reset();
        assert_eq!(controller.state().dirt, parameters.initial_dirt);

        let mut a = GroomingController::with_parameters(parameters).unwrap();
        let mut b = GroomingController::with_parameters(parameters).unwrap();
        for _ in 0..100 {
            assert_eq!(
                a.update(autonomous_input(0.01)).unwrap(),
                b.update(autonomous_input(0.01)).unwrap()
            );
        }
    }

    #[test]
    fn autonomous_grooming_requires_satiety_support_and_neural_gate() {
        for blocked in ["hungry", "support", "probe", "outputs"] {
            let mut controller = GroomingController::new();
            controller.set_initial_dirt(1.0).unwrap();
            for _ in 0..3_000 {
                let mut sample = autonomous_input(0.01);
                match blocked {
                    "hungry" => sample.hungry = true,
                    "support" => sample.contact_count = 3,
                    "probe" => sample.grooming_probe_rate_hz = 0.0,
                    "outputs" => sample.neural_outputs_connected = false,
                    _ => unreachable!(),
                }
                assert!(!controller.update(sample).unwrap().active);
            }
        }

        let mut controller = GroomingController::new();
        controller.set_initial_dirt(1.0).unwrap();
        for _ in 0..49 {
            assert!(!controller.update(autonomous_input(0.01)).unwrap().active);
        }
        let command = controller.update(autonomous_input(0.01)).unwrap();
        assert!(command.active);
        assert_eq!(command.trigger, GroomingTrigger::Autonomous);
        assert!(command.stable_support_seconds >= 0.5);
    }

    #[test]
    fn only_completed_autonomous_bout_receives_full_cleaning_credit() {
        let mut interrupted = GroomingController::new();
        interrupted.set_initial_dirt(0.5).unwrap();
        for _ in 0..50 {
            interrupted.update(autonomous_input(0.01)).unwrap();
        }
        let before_abort = interrupted.state().dirt;
        for _ in 0..5 {
            interrupted
                .update(GroomingInput {
                    contact_count: 0,
                    ..autonomous_input(0.01)
                })
                .unwrap();
        }
        assert_eq!(interrupted.state().interrupted_bouts, 1);
        assert!(interrupted.state().dirt >= before_abort);

        let mut completed = GroomingController::new();
        completed.set_initial_dirt(0.5).unwrap();
        let initial = completed.state().dirt;
        for _ in 0..500 {
            completed.update(autonomous_input(0.01)).unwrap();
        }
        assert_eq!(completed.state().completed_bouts, 1);
        assert!(completed.state().dirt <= initial * 0.70);
    }

    #[test]
    fn combined_bout_has_distinct_rubbing_reaching_and_recovery_controls() {
        let mut controller = GroomingController::new();
        controller.request_manual();
        controller.update(input(0.01)).unwrap();
        let controls_at = |controller: &GroomingController| {
            let mut controls = [0.0; GROOMING_CONTROL_COUNT];
            let mut adhesion = [0.0; GROOMING_LEG_COUNT];
            controller.apply(&mut controls, &mut adhesion);
            controls
        };
        controller.update(input(GROOMING_BOUT_DURATION_SECONDS*0.28)).unwrap();
        let rubbing = controls_at(&controller);
        controller.update(input(GROOMING_BOUT_DURATION_SECONDS*0.48)).unwrap();
        let reaching = controls_at(&controller);
        controller.update(input(GROOMING_BOUT_DURATION_SECONDS*0.225)).unwrap();
        let recovery = controls_at(&controller);
        // Both fitted poses lift the forefeet; eye brushing reaches higher and
        // laterally outward rather than merely increasing the old yaw overlay.
        assert!((0.0..0.5).contains(&rubbing[6]));
        assert!(reaching[6]>rubbing[6]);
        assert!(reaching[1]<rubbing[1]);
        assert!(recovery[..7].iter().chain(&recovery[21..28]).all(|value| value.abs()<1e-9));
        controller.update(input(GROOMING_BOUT_DURATION_SECONDS*0.015)).unwrap();
        assert!(controls_at(&controller).iter().all(|value|value.abs()<1e-9));
    }
}
