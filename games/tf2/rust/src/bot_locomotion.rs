//! Source NextBot movement request/anchor timers and default player buttons.
use std::collections::VecDeque;

pub(super) fn update_ticks(interval:f32)->u64{(0.5+0.1/interval) as u64}

#[derive(Clone,Debug)]
struct Scheduled {identity:u32,last_tick:i64,flagged:bool,alive:bool,registered:bool}
#[derive(Clone,Debug,Default)]
pub(super) struct Scheduler {queue:VecDeque<Scheduled>,period:i64,spent_seconds:f64}
impl Scheduler {
    pub fn prepare(&mut self,tick:u64,interval:f32,roster:impl Iterator<Item=(u32,bool)>){
        self.period=update_ticks(interval) as i64;self.spent_seconds=0.0;
        for entry in &mut self.queue{entry.registered=false;}
        for (identity,alive) in roster{
            if let Some(entry)=self.queue.iter_mut().find(|entry|entry.identity==identity){entry.alive=alive;entry.registered=true;}
            else{self.queue.push_front(Scheduled{identity,last_tick:-999,flagged:false,alive,registered:true});}
        }
        self.queue.retain(|entry|entry.registered);
        if self.period<1{return;}
        let mut count=(self.queue.iter().filter(|entry|entry.alive).count() as u64).div_ceil(self.period as u64);
        for entry in &mut self.queue{
            if count==0{break;}
            if entry.flagged{continue;}
            if tick as i64-entry.last_tick<self.period{break;}
            if entry.alive{entry.flagged=true;count-=1;}
        }
    }
    pub fn begin(&mut self,identity:u32,tick:u64)->bool{
        let index=self.queue.iter().position(|entry|entry.identity==identity).expect("registered bot");
        let entry=&mut self.queue[index];
        let flagged=std::mem::replace(&mut entry.flagged,false);
        // ShouldUpdate samples the frame sum only for flagged bots; max-slide
        // admission of an unflagged bot uses its local zero sum as in the SDK.
        let spent=if flagged{(self.spent_seconds*1000.0) as f32}else{0.0};
        if self.period>=1&&!(flagged&&spent<15.0)&&!(tick as i64-entry.last_tick-self.period>=2&&spent<30.0){return false;}
        let mut entry=self.queue.remove(index).unwrap();entry.last_tick=tick as i64;self.queue.push_back(entry);true
    }
    pub fn finish(&mut self,seconds:f64)->bool{if !seconds.is_finite()||seconds<0.0{return false;}self.spent_seconds+=seconds;true}
    pub fn reset(&mut self,identity:u32){if let Some(entry)=self.queue.iter_mut().find(|entry|entry.identity==identity){entry.last_tick=-999;entry.flagged=false;}}
    #[cfg(test)]
    pub fn last_update(&self,identity:u32)->i64{self.queue.iter().find(|entry|entry.identity==identity).unwrap().last_tick}
}
#[derive(Clone,Debug,Default)]
pub(super) struct Monitor {
    request:Option<f32>,anchor:[f32;3],since:Option<f32>,stuck:bool,repeat_at:f32,
}
impl Monitor {
    pub fn request_move(&mut self,now:f32){self.request=Some(now);}
    pub fn clear(&mut self,now:f32,feet:[f32;3]){self.stuck=false;self.anchor=feet;self.since=Some(now);}
    pub fn update(&mut self,now:f32,feet:[f32;3],desired_speed:f32)->bool {
        if self.request.is_none_or(|request|now-request>0.25){self.anchor=feet;self.since=Some(now);return false;}
        if feet.iter().zip(self.anchor).map(|(a,b)|(a-b)*(a-b)).sum::<f32>()>100.0*100.0{self.clear(now,feet);return false;}
        if self.stuck {
            if now>self.repeat_at{self.repeat_at=now+1.0;return true;}
        }else if self.since.is_none_or(|since|now-since>100.0/(0.1*desired_speed+0.1)){self.stuck=true;return true;}
        false
    }
}

// nb_player_move_direct=0. Left has priority if both side buttons are pressed.
pub(super) fn approach(relative:f32,moving:bool,stuck_left:Option<bool>,speed:f32)->(f32,f32){
    let ahead=if moving{relative.cos()}else{0.0};
    let side=if moving{-relative.sin()}else{0.0};
    let forward=if ahead>0.25{speed}else if ahead< -0.25{-speed}else{0.0};
    let side=if side<= -0.25||stuck_left==Some(true){-speed}else if side>=0.25||stuck_left==Some(false){speed}else{0.0};
    (forward,side)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn manager_schedules_oldest_bots_and_preserves_two_tick_max_slide(){
        let mut scheduler=Scheduler::default();
        let mut admitted=Vec::new();
        for tick in 0..10{
            scheduler.prepare(tick,0.015,(2..17).map(|identity|(identity,true)));
            admitted.push((2..17).filter(|identity|scheduler.begin(*identity,tick)).collect::<Vec<_>>());
        }
        assert_eq!(admitted[0],(2..17).collect::<Vec<_>>());
        assert!(admitted[1..7].iter().all(Vec::is_empty));
        assert_eq!(admitted[7],[2,3,4]);assert_eq!(admitted[8],[5,6,7]);assert_eq!(admitted[9],(8..17).collect::<Vec<_>>());
    }
    #[test]
    fn flagged_bots_observe_the_fifteen_millisecond_budget_and_respawn_resets_admission(){
        let mut scheduler=Scheduler::default();
        scheduler.prepare(0,0.015,(2..5).map(|identity|(identity,true)));
        for id in 2..5{assert!(scheduler.begin(id,0));}
        scheduler.prepare(7,0.015,(2..5).map(|identity|(identity,true)));
        scheduler.finish(0.015);assert!(!scheduler.begin(2,7));assert!(!scheduler.begin(3,7));
        scheduler.reset(3);assert!(scheduler.begin(3,7));
        scheduler.prepare(8,0.015,[(2,false),(3,true)].into_iter());
        assert_eq!(scheduler.queue.len(),2);
        assert!(!scheduler.begin(3,8));
    }
    #[test]
    fn source_update_period_rounds_to_ticks_without_changing_movement_cadence(){
        assert_eq!(update_ticks(0.015),7);assert_eq!(update_ticks(0.02),5);assert_eq!(update_ticks(0.03),3);
    }
    #[test]
    fn default_button_mode_preserves_main_action_side_input_and_left_priority(){
        assert_eq!(approach(0.0,true,None,400.0),(400.0,0.0));
        assert_eq!(approach(0.0,true,Some(true),400.0),(400.0,-400.0));
        assert_eq!(approach(0.0,true,Some(false),400.0),(400.0,400.0));
        assert_eq!(approach(std::f32::consts::FRAC_PI_4,true,Some(false),400.0),(400.0,-400.0));
        assert_eq!(approach(-std::f32::consts::FRAC_PI_2,true,None,400.0),(0.0,400.0));
    }
    #[test]
    fn movement_request_idle_window_and_escape_radius_use_strict_source_bounds(){
        let mut monitor=Monitor::default();assert!(!monitor.update(0.0,[0.0;3],300.0));
        let escape=100.0/(0.1*300.0+0.1);monitor.request_move(escape);
        assert!(!monitor.update(escape,[100.0,0.0,0.0],300.0));
        assert!(monitor.update(escape+0.001,[100.0,0.0,0.0],300.0));
        monitor.clear(escape+0.001,[100.0,0.0,0.0]);
        assert!(!monitor.update(escape+0.002,[100.0,0.0,0.0],300.0));
        assert!(!monitor.update(10.0,[100.0,0.0,0.0],300.0));monitor.request_move(10.0);
        assert!(!monitor.update(10.01,[100.0,0.0,0.0],300.0));
    }
}
