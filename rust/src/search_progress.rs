//! Local history only: no resource IDs, source positions, or environmental oracle.
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug)]
pub struct SearchObservation {
    pub time: f64,
    pub position: [f64; 3],
    pub concentration_ppm: f64,
    pub searching: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SearchProgress {
    pub stalled: bool,
    pub recovering: bool,
    pub recovery_failed: bool,
    pub recovery_count: u64,
}

#[derive(Default)]
pub struct ProgressMonitor {
    history: VecDeque<SearchObservation>,
    recovery: Option<SearchObservation>,
    count: u64,
}

fn distance(a: [f64;3], b: [f64;3]) -> f64 {
    a.into_iter().zip(b).map(|(x,y)|(x-y).powi(2)).sum::<f64>().sqrt()
}

impl ProgressMonitor {
    pub fn reset(&mut self) { *self=Self::default(); }

    pub fn update(&mut self, input: SearchObservation) -> SearchProgress {
        if !input.searching || self.history.back().is_some_and(|old| input.time<old.time) {
            self.history.clear();
            self.recovery=None;
            return SearchProgress {recovery_count:self.count,..Default::default()};
        }
        self.history.push_back(input);
        while self.history.front().is_some_and(|old| input.time-old.time>3.002001) {
            self.history.pop_front();
        }
        let mut result=SearchProgress {recovery_count:self.count,..Default::default()};
        if let Some(origin)=self.recovery {
            let improved=input.concentration_ppm-origin.concentration_ppm > (origin.concentration_ppm*0.05).max(0.25);
            if distance(input.position,origin.position)>5.0 || improved {
                self.recovery=None;
                self.history.clear();
            } else {
                result.recovering=true;
                result.recovery_failed=input.time-origin.time>5.0;
                return result;
            }
        }
        let Some(first)=self.history.front() else {return result;};
        if input.time-first.time<3.0-1e-8 {return result;}
        let mean=|start:f64,end:f64| {
            let mut sum=0.0;let mut n=0;
            for point in &self.history {
                if point.time>=start&&point.time<=end {sum+=point.concentration_ppm;n+=1;}
            }
            sum/(n.max(1) as f64)
        };
        let before=mean(first.time,first.time+0.5);
        let after=mean(input.time-0.5,input.time);
        let improved=after-before>(before*0.05).max(0.25);
        let displacement=distance(input.position,first.position);
        let mut path=0.0;
        let mut previous=first.position;
        for point in &self.history {path+=distance(point.position,previous);previous=point.position;}
        let repetitive=displacement<=5.0 || (path>5.0&&displacement/path<0.75);
        if !improved&&repetitive {
            self.count+=1;
            self.recovery=Some(input);
            result=SearchProgress {stalled:true,recovering:true,recovery_count:self.count,..Default::default()};
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_circle_reversal_and_stationary_fixtures_trigger_at_three_seconds() {
        for kind in 0..3 {
            let mut monitor=ProgressMonitor::default();
            let mut trigger=None;
            for i in 0..=1600 {
                let time=i as f64*0.002;
                let phase=time*std::f64::consts::TAU/1.5;
                let position=match kind {0=>[4.0*phase.cos(),4.0*phase.sin(),2.0],1=>[8.0*phase.sin(),0.0,2.0],_=>[0.0,0.0,2.0]};
                let state=monitor.update(SearchObservation {time,position,concentration_ppm:2.0,searching:true});
                if state.stalled {trigger.get_or_insert(time);}
            }
            assert!((trigger.unwrap()-3.0).abs()<=0.002001,"kind {kind}: {trigger:?}");
        }
    }
    #[test]
    fn real_approach_and_rest_are_not_stalls_and_recovery_is_scored() {
        for searching in [true,false] {
            let mut monitor=ProgressMonitor::default();
            for i in 0..5000 {
                let time=i as f64*0.002;
                assert!(!monitor.update(SearchObservation {time,position:[time,0.0,2.0],concentration_ppm:2.0+time,searching}).stalled);
            }
        }
        let mut monitor=ProgressMonitor::default();
        for i in 0..=1500 {monitor.update(SearchObservation {time:i as f64*0.002,position:[0.0;3],concentration_ppm:2.0,searching:true});}
        assert!(monitor.update(SearchObservation {time:8.002,position:[0.0;3],concentration_ppm:2.0,searching:true}).recovery_failed);
        let recovered=monitor.update(SearchObservation {time:8.004,position:[6.0,0.0,0.0],concentration_ppm:2.0,searching:true});
        assert!(!recovered.recovering);
        assert_eq!(recovered.recovery_count,1);
    }
}
