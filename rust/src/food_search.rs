//! Engineered local chemotaxis and active height sampling, driven by CNS ORN readout.
//! Resource positions and resource identities deliberately do not enter this API.
use serde::Serialize;
use std::collections::VecDeque;
use crate::search_progress::{ProgressMonitor,SearchObservation};

#[derive(Clone,Copy)]
pub struct FoodSearchInput {
    pub time:f64,
    pub dt:f64,
    pub position:[f64;3],
    pub forward:[f64;2],
    pub concentration_ppm:f64,
    pub eligible:bool,
    pub grounded:bool,
    pub up_clearance_mm:f64,
    pub down_clearance_mm:f64,
    pub bounds:[f64;2],
}

#[derive(Clone,Copy,Debug,Default,Serialize)]
pub struct FoodSearchCommand {
    pub vertical_sampling:bool,
    pub allow_takeoff:bool,
    pub height_target_mm:Option<f64>,
    pub steering:Option<f64>,
    pub turning_back:bool,
    pub escaping_overhang:bool,
    pub recovery_active:bool,
    pub recovery_failed:bool,
    pub recovery_count:u64,
    pub retreating:bool,
    pub retreat_direction_xy:Option<[f64;2]>,
}

struct VerticalProbe {
    target:f64,
    best_height:f64,
    best_concentration:f64,
    reference:f64,
    direction:f64,
    reversals:u32,
    settled:f64,
    step:f64,
    clearance_bonus:f64,
}

#[derive(Default)]
pub struct LocalFoodSearch {
    monitor:ProgressMonitor,
    weak_seconds:f64,
    concentration:f64,
    probe:Option<VerticalProbe>,
    sampled_height:Option<f64>,
    best:Option<([f64;3],f64)>,
    return_to_best:bool,
    blocked_seconds:f64,
    escape:Option<([f64;3],[f64;2])>,
    acquisition_settled:f64,
    breadcrumbs:VecDeque<[f64;3]>,
    recovery_wait_seconds:f64,
    retreat:Option<([f64;3],[f64;2])>,
    sampling_anchor:Option<[f64;3]>,
    sampling_stationary_seconds:f64,
}

impl LocalFoodSearch {
    pub fn reset(&mut self) { *self=Self::default(); }

    pub fn update(&mut self,input:FoodSearchInput)->FoodSearchCommand {
        if !input.eligible {
            self.reset();
            return FoodSearchCommand::default();
        }
        self.concentration+=(1.0-(-input.dt/0.15).exp())*(input.concentration_ppm.min(100.0)-self.concentration);
        let progress=self.monitor.update(SearchObservation {
            time:input.time,position:input.position,concentration_ppm:self.concentration,searching:true,
        });
        let mut command=FoodSearchCommand {
            recovery_active:progress.recovering,recovery_failed:progress.recovery_failed,
            recovery_count:progress.recovery_count,..Default::default()
        };
        if self.breadcrumbs.back().is_none_or(|p|(input.position[0]-p[0]).hypot(input.position[1]-p[1])>=1.0) {
            self.breadcrumbs.push_back(input.position);
            if self.breadcrumbs.len()>256 {self.breadcrumbs.pop_front();}
        }
        self.recovery_wait_seconds=if progress.recovering {self.recovery_wait_seconds+input.dt}else{0.0};
        let sampling=self.probe.is_some() || (self.sampled_height.is_some()&&self.acquisition_settled<0.5);
        if sampling && !input.grounded {
            let moved=self.sampling_anchor.map_or(f64::INFINITY,|p|
                ((input.position[0]-p[0]).powi(2)+(input.position[1]-p[1]).powi(2)+(input.position[2]-p[2]).powi(2)).sqrt());
            if moved>1.0 {self.sampling_anchor=Some(input.position);self.sampling_stationary_seconds=0.0;}
            else {self.sampling_stationary_seconds+=input.dt;}
        } else {self.sampling_anchor=None;self.sampling_stationary_seconds=0.0;}
        // Sensory fluctuations cannot prove that an obstructed height command
        // was reached. Retrace after three seconds without physical progress.
        let blocked_probe=self.sampling_stationary_seconds>=3.0;
        if self.retreat.is_none() && !input.grounded && (self.recovery_wait_seconds>=2.0 || blocked_probe) {
            // Retrace a physically visited position, never a resource coordinate.
            // A turn-in-place can be impossible when the body touches a planter.
            let target=self.breadcrumbs.iter().rev().find(|p|
                (input.position[0]-p[0]).hypot(input.position[1]-p[1])>=8.0)
                .map(|p|[p[0],p[1]])
                .unwrap_or([input.position[0]-8.0*input.forward[0],input.position[1]-8.0*input.forward[1]]);
            self.retreat=Some((input.position,target));
            self.probe=None;self.sampled_height=None;self.return_to_best=false;
        }
        if let Some((origin,target))=self.retreat {
            let delta=[target[0]-input.position[0],target[1]-input.position[1]];
            let remaining=delta[0].hypot(delta[1]);
            let moved=(input.position[0]-origin[0]).hypot(input.position[1]-origin[1]);
            if remaining<2.0&&moved>5.0 {
                self.retreat=None;self.best=None;self.recovery_wait_seconds=0.0;
            } else {
                command.recovery_active=true;
                command.retreating=true;command.allow_takeoff=true;
                command.height_target_mm=Some(origin[2]);
                command.retreat_direction_xy=Some([delta[0]/remaining.max(1e-6),delta[1]/remaining.max(1e-6)]);
                return command;
            }
        }
        if self.best.is_none_or(|(_,c)| self.concentration>c) {
            self.best=Some((input.position,self.concentration));
        }
        if input.grounded&&self.concentration<3.0&&self.sampled_height.is_none() {
            self.weak_seconds+=input.dt;
        } else {self.weak_seconds=0.0;}
        let acquiring=self.sampled_height.is_some()&&self.acquisition_settled<0.5;
        if self.probe.is_none()&&!acquiring&&(progress.stalled||self.weak_seconds>=0.6)&&input.up_clearance_mm>12.0 {
            let refining=!input.grounded&&self.sampled_height.is_some();
            let direction=if refining {-1.0}else{1.0};
            let step=if refining {4.0}else{8.0};
            self.probe=Some(VerticalProbe {
                target:(input.position[2]+direction*step).clamp(input.bounds[0],input.bounds[1]),
                best_height:input.position[2],best_concentration:self.concentration,
                reference:self.concentration,direction,reversals:0,settled:0.0,
                step,clearance_bonus:if refining {0.0}else{6.0},
            });
            self.sampled_height=None;
            self.return_to_best=false;
        }
        let mut completed=None;
        if let Some(probe)=self.probe.as_mut() {
            let support_floor=(input.position[2]-input.down_clearance_mm+5.0).clamp(input.bounds[0],input.bounds[1]);
            probe.target=probe.target.max(support_floor);
            command.allow_takeoff=true;
            command.vertical_sampling=true;
            command.height_target_mm=Some(probe.target);
            if !input.grounded&&(input.position[2]-probe.target).abs()<2.0 {
                probe.settled+=input.dt;
            } else {probe.settled=0.0;}
            if probe.settled>=0.35 {
                if self.concentration>probe.best_concentration {
                    probe.best_concentration=self.concentration;
                    probe.best_height=input.position[2];
                }
                let better=self.concentration-probe.reference>(probe.reference*0.05).max(0.02);
                if !better {
                    probe.reversals+=1;
                    probe.direction=-probe.direction;
                }
                if probe.reversals>=2 {
                    completed=Some((probe.best_height+probe.clearance_bonus).clamp(input.bounds[0],input.bounds[1]));
                } else {
                    probe.reference=self.concentration;
                    let next=(probe.target+probe.step*probe.direction).clamp(support_floor,input.bounds[1]);
                    if (next-probe.target).abs()<1.0 {probe.reversals=2;completed=Some(probe.best_height+probe.clearance_bonus);}
                    probe.target=next;
                    probe.settled=0.0;
                }
            }
        }
        if let Some(height)=completed {
            self.probe=None;
            self.sampled_height=Some(height);
            self.acquisition_settled=0.0;
            self.best=Some((input.position,self.concentration));
            command.vertical_sampling=false;
            command.height_target_mm=Some(height);
        } else if self.probe.is_none() {command.height_target_mm=self.sampled_height;}

        // Finish reaching the selected sensory height before translating or starting
        // another probe. A completed scan can end far below its selected best sample.
        if self.probe.is_none()&&self.acquisition_settled<0.5 {
            if let Some(height)=self.sampled_height {
                if (input.position[2]-height).abs()<2.0 {self.acquisition_settled+=input.dt;}
                else {self.acquisition_settled=0.0;}
                command.vertical_sampling=true;
                command.allow_takeoff=true;
            }
        }

        if self.probe.is_none() {
            if let Some((position, concentration))=self.best {
                let delta=[position[0]-input.position[0],position[1]-input.position[1]];
                let distance=delta[0].hypot(delta[1]);
                if concentration>self.concentration*1.15+0.1&&distance>2.0 {self.return_to_best=true;}
                if self.return_to_best&&distance<=2.0 {self.return_to_best=false;self.best=Some((input.position,self.concentration));}
                if self.return_to_best||progress.recovering {
                    let dot=input.forward[0]*delta[0]+input.forward[1]*delta[1];
                    let cross=input.forward[0]*delta[1]-input.forward[1]*delta[0];
                    let angle=cross.atan2(dot);
                    command.steering=Some(angle.clamp(-0.7,0.7));
                    command.turning_back=angle.abs()>1.0;
                }
            }
        }
        if self.probe.as_ref().is_some_and(|probe|probe.target>input.position[2]+3.0)
            && input.up_clearance_mm<=55.0 {
            self.blocked_seconds+=input.dt;
        } else {self.blocked_seconds=0.0;}
        if self.blocked_seconds>=0.5&&self.escape.is_none() {
            self.escape=Some((input.position,[-input.forward[0],-input.forward[1]]));
        }
        if let Some((origin,direction))=self.escape {
            let distance=(input.position[0]-origin[0]).hypot(input.position[1]-origin[1]);
            if distance>5.0&&input.up_clearance_mm>55.0 {
                self.escape=None;
                self.blocked_seconds=0.0;
            } else {
                let angle=(input.forward[0]*direction[1]-input.forward[1]*direction[0])
                    .atan2(input.forward[0]*direction[0]+input.forward[1]*direction[1]);
                command.vertical_sampling=false;
                command.escaping_overhang=true;
                command.allow_takeoff=true;
                command.height_target_mm=Some(origin[2]);
                command.steering=Some(angle.clamp(-0.7,0.7));
                command.turning_back=angle.abs()>1.0;
            }
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stalled_airborne_search_retraces_real_history_without_faking_recovery() {
        let mut search=LocalFoodSearch::default();
        let mut command=FoodSearchCommand::default();
        for i in 0..4500 {
            let t=i as f64*0.002;
            command=search.update(FoodSearchInput {time:t,dt:0.002,
                position:[(t*10.0).min(20.0),0.0,20.0],forward:[1.0,0.0],
                concentration_ppm:2.0,eligible:true,grounded:false,
                up_clearance_mm:100.0,down_clearance_mm:20.0,bounds:[5.0,110.0]});
        }
        assert!(command.retreating);assert!(command.retreat_direction_xy.unwrap()[0] < -0.9);
        // Issuing a retreat command is not proof that the body actually moved.
        for i in 4500..6500 {
            command=search.update(FoodSearchInput {time:i as f64*0.002,dt:0.002,
                position:[20.0,0.0,20.0],forward:[1.0,0.0],concentration_ppm:2.0,
                eligible:true,grounded:false,up_clearance_mm:100.0,down_clearance_mm:20.0,bounds:[5.0,110.0]});
        }
        assert!(command.recovery_failed);
        command=search.update(FoodSearchInput {time:13.0,dt:0.002,position:[12.0,0.0,20.0],
            forward:[1.0,0.0],concentration_ppm:2.0,eligible:true,grounded:false,
            up_clearance_mm:100.0,down_clearance_mm:20.0,bounds:[5.0,110.0]});
        assert!(!command.retreating);assert!(!command.recovery_failed);
    }

    #[test]
    fn descending_probe_cannot_request_a_height_inside_the_support_surface() {
        let mut search=LocalFoodSearch::default();
        search.probe=Some(VerticalProbe {target:28.0,best_height:35.0,best_concentration:2.0,
            reference:1.0,direction:-1.0,reversals:0,settled:0.0,step:4.0,clearance_bonus:0.0});
        let mut finished=false;
        for i in 0..600 {
            let command=search.update(FoodSearchInput {time:i as f64*0.002,dt:0.002,
                position:[0.0,0.0,35.0],forward:[1.0,0.0],concentration_ppm:3.0,
                eligible:true,grounded:false,up_clearance_mm:100.0,down_clearance_mm:5.0,bounds:[5.0,110.0]});
            assert!(command.height_target_mm.unwrap()>=35.0);
            finished|=!command.vertical_sampling;
        }
        assert!(finished,"support boundary must end the downward probe");
    }
    #[test]
    fn selected_height_must_be_reached_before_forward_search_or_resampling() {
        let mut search=LocalFoodSearch::default();
        search.sampled_height=Some(40.0);
        let mut input=FoodSearchInput {time:0.0,dt:0.002,position:[0.0,0.0,20.0],forward:[1.0,0.0],concentration_ppm:2.0,eligible:true,grounded:false,up_clearance_mm:100.0,down_clearance_mm:20.0,bounds:[5.0,110.0]};
        for i in 0..2000 {
            input.time=i as f64*input.dt;
            input.position[2]=20.0+i as f64*0.006;
            let command=search.update(input);
            assert!(command.vertical_sampling);
            assert_eq!(command.height_target_mm,Some(40.0));
        }
        input.position[2]=40.0;
        for _ in 0..260 {input.time+=input.dt;search.update(input);}
        assert!(!search.update(input).vertical_sampling);
    }
    #[test]
    fn sensory_fluctuations_cannot_hide_a_physically_blocked_height_probe() {
        let mut search=LocalFoodSearch::default();
        search.sampled_height=Some(40.0);
        let mut started=None;
        for i in 0..1600 {
            let t=i as f64*0.002;
            let command=search.update(FoodSearchInput {time:t,dt:0.002,position:[0.0,0.0,20.0],
                forward:[1.0,0.0],concentration_ppm:2.0+0.5*(t*4.0).sin(),eligible:true,grounded:false,
                up_clearance_mm:100.0,down_clearance_mm:20.0,bounds:[5.0,110.0]});
            if command.retreating {assert!(!command.vertical_sampling);started=Some(t);break;}
        }
        assert!((3.0..=3.01).contains(&started.unwrap()));
    }

    #[test]
    fn weak_odor_samples_height_but_neural_ineligibility_cannot_move() {
        let mut search=LocalFoodSearch::default();
        let mut input=FoodSearchInput {time:0.0,dt:0.002,position:[0.0,0.0,2.0],forward:[1.0,0.0],concentration_ppm:0.5,eligible:true,grounded:true,up_clearance_mm:100.0,down_clearance_mm:2.0,bounds:[5.0,110.0]};
        let mut command=FoodSearchCommand::default();
        for i in 0..400 {input.time=i as f64*input.dt;command=search.update(input);}
        assert!(command.vertical_sampling&&command.allow_takeoff);
        input.eligible=false;
        let disabled=search.update(input);
        assert!(!disabled.vertical_sampling&&!disabled.allow_takeoff);
        assert!(disabled.height_target_mm.is_none()&&disabled.steering.is_none());
    }
    #[test]
    fn vertical_search_brackets_a_local_sensory_peak_without_source_coordinates() {
        let mut search=LocalFoodSearch::default();
        let mut height=2.0_f64;
        let mut airborne=false;
        let mut target=None;
        for i in 0..5000 {
            let command=search.update(FoodSearchInput {
                time:i as f64*0.002,dt:0.002,position:[0.0,0.0,height],forward:[1.0,0.0],
                concentration_ppm:10.0*(-((height-34.0)/15.0).powi(2)).exp(),
                eligible:true,grounded:!airborne,up_clearance_mm:100.0,down_clearance_mm:height,bounds:[5.0,110.0],
            });
            airborne|=command.allow_takeoff;
            if let Some(request)=command.height_target_mm {
                height+=(request-height).clamp(-0.09,0.09);
                if !command.vertical_sampling {target=Some(request);break;}
            }
        }
        assert!((target.expect("scan never completed")-37.0).abs()<5.0,"{target:?}");
    }
}
