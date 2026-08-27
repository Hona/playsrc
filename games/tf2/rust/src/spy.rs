use crate::{PlayerClass, PlayerTeam};

pub const CLOAK_MAXIMUM: f32 = 100.0;
pub const CLOAK_CONSUME_RATE: f32 = 10.0;
pub const CLOAK_REGEN_RATE: f32 = 3.3;
pub const CLOAK_MINIMUM_TO_ACTIVATE: f32 = 8.0;
pub const CLOAK_FADE_IN_SECONDS: f32 = 1.0;
pub const CLOAK_FADE_OUT_SECONDS: f32 = 2.0;
pub const CLOAK_ATTACK_LOCK_SECONDS: f32 = 2.0;
pub const DISGUISE_SECONDS: f32 = 2.0;
pub const QUICK_DISGUISE_SECONDS: f32 = 0.5;
pub const DISGUISE_WEAR_OFF_SECONDS: f32 = 0.5;
pub const KNIFE_DAMAGE: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CloakRenderState {
    pub local_factor: f32,
    pub world_factor: f32,
    pub raw_factor: f32,
    pub player_tint: [f32; 3],
}

pub fn cloak_render_state(
    invisibility: f32,
    blink: bool,
    empty_motion_cloak: bool,
    team: PlayerTeam,
    enemy: bool,
    deathcam_target: bool,
) -> CloakRenderState {
    CloakRenderState {
        local_factor: if blink || empty_motion_cloak {
            0.3
        } else if invisibility < 0.01 {
            0.0
        } else {
            0.22 + invisibility * (0.5 - 0.22)
        },
        world_factor: if !enemy || deathcam_target {
            invisibility.min(0.95)
        } else {
            invisibility
        },
        raw_factor: invisibility,
        player_tint: if team == PlayerTeam::Red {
            [1.0, 0.5, 0.4]
        } else {
            [0.4, 0.5, 1.0]
        },
    }
}

pub fn player_hulls_touch(
    left: [f32; 3],
    left_hull: playsrc_collision::Hull,
    right: [f32; 3],
    right_hull: playsrc_collision::Hull,
) -> bool {
    (0..3).all(|axis| {
        left[axis] + left_hull.maxs[axis] >= right[axis] + right_hull.mins[axis]
            && right[axis] + right_hull.maxs[axis] >= left[axis] + left_hull.mins[axis]
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Disguise {
    pub class: PlayerClass,
    pub team: PlayerTeam,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpyState {
    pub cloak_meter: f32,
    pub invisibility: f32,
    pub cloaked: bool,
    pub disguise: Option<Disguise>,
    pub desired_disguise: Option<Disguise>,
    pub disguise_complete_time: f32,
    pub invisibility_complete_time: f32,
    pub no_attack_until: f32,
    pub next_stealth_time: f32,
    pub disguise_wear_off_until: f32,
    pub last_stealth_expose_time: Option<f32>,
}

impl Default for SpyState {
    fn default() -> Self {
        Self {
            cloak_meter: CLOAK_MAXIMUM,
            invisibility: 0.0,
            cloaked: false,
            disguise: None,
            desired_disguise: None,
            disguise_complete_time: 0.0,
            invisibility_complete_time: 0.0,
            no_attack_until: 0.0,
            next_stealth_time: 0.0,
            disguise_wear_off_until: 0.0,
            last_stealth_expose_time: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpyEvent {
    Cloaked,
    Uncloaked,
    Disguising,
    Disguised,
    DisguiseRemoved,
}

impl SpyState {
    pub fn note_damage(&mut self, now: f32, damage: i32, bleeding: bool) {
        if damage > 5 || bleeding {
            self.expose(now);
        }
    }
    pub fn expose(&mut self, now: f32) {
        if self.cloaked {
            self.last_stealth_expose_time = Some(now);
        }
    }

    pub fn blink(self, now: f32) -> bool {
        self.last_stealth_expose_time
            .is_some_and(|time| now - time <= 0.3)
    }

    pub fn reveal_invisibility(&mut self, now: f32, urine: bool) {
        if self.blink(now) || urine {
            self.invisibility *= 0.85;
        }
    }
    pub fn toggle_cloak(&mut self, now: f32) -> Option<SpyEvent> {
        if now < self.next_stealth_time {
            return None;
        }
        if self.cloaked {
            self.decloak(now);
            self.next_stealth_time = now + 0.1;
            return Some(SpyEvent::Uncloaked);
        }
        if self.cloak_meter > CLOAK_MINIMUM_TO_ACTIVATE {
            self.cloaked = true;
            self.invisibility_complete_time = now + CLOAK_FADE_IN_SECONDS;
            self.next_stealth_time = now + 0.5;
            Some(SpyEvent::Cloaked)
        } else {
            self.next_stealth_time = now + 0.1;
            None
        }
    }

    pub fn request_disguise(
        &mut self,
        real_team: PlayerTeam,
        desired: Disguise,
        now: f32,
    ) -> Option<SpyEvent> {
        if !desired.team.is_gameplay() {
            return None;
        }
        if desired.team == real_team && desired.class == PlayerClass::Spy {
            return self.remove_disguise(now);
        }
        if self.disguise == Some(desired) || self.desired_disguise == Some(desired) {
            return None;
        }
        self.desired_disguise = Some(desired);
        self.disguise_complete_time = now
            + if self.disguise.is_some() {
                QUICK_DISGUISE_SECONDS
            } else {
                DISGUISE_SECONDS
            };
        Some(SpyEvent::Disguising)
    }

    pub fn remove_disguise(&mut self, now: f32) -> Option<SpyEvent> {
        if self.disguise.is_none() && self.desired_disguise.is_none() {
            return None;
        }
        self.disguise = None;
        self.desired_disguise = None;
        self.disguise_complete_time = 0.0;
        self.disguise_wear_off_until = now + DISGUISE_WEAR_OFF_SECONDS;
        Some(SpyEvent::DisguiseRemoved)
    }

    pub fn can_attack(self, now: f32) -> bool {
        !self.cloaked && now >= self.no_attack_until
    }

    pub fn advance(&mut self, now: f32, interval: f32) -> [Option<SpyEvent>; 2] {
        let mut events = [None, None];
        if self.cloaked {
            self.cloak_meter -= interval * CLOAK_CONSUME_RATE;
            if self.cloak_meter <= 0.0 {
                self.cloak_meter = 0.0;
                self.decloak(now);
                events[0] = Some(SpyEvent::Uncloaked);
            }
        } else {
            self.cloak_meter = (self.cloak_meter + interval * CLOAK_REGEN_RATE).min(CLOAK_MAXIMUM);
        }
        self.invisibility = if self.invisibility_complete_time > now {
            if self.cloaked {
                1.0 - (self.invisibility_complete_time - now)
            } else {
                (self.invisibility_complete_time - now) * 0.5
            }
        } else if self.cloaked {
            1.0
        } else {
            0.0
        }
        .clamp(0.0, 1.0);
        if self.desired_disguise.is_some()
            && self.disguise_complete_time > 0.0
            && now > self.disguise_complete_time
        {
            self.disguise = self.desired_disguise.take();
            self.disguise_complete_time = 0.0;
            events[1] = Some(SpyEvent::Disguised);
        }
        events
    }

    fn decloak(&mut self, now: f32) {
        self.cloaked = false;
        self.invisibility_complete_time = now + CLOAK_FADE_OUT_SECONDS;
        self.no_attack_until = now + CLOAK_ATTACK_LOCK_SECONDS;
    }
}

pub fn can_backstab(
    owner_center: [f32; 3],
    owner_forward: [f32; 3],
    target_center: [f32; 3],
    target_forward: [f32; 3],
) -> bool {
    let to_target = normalize_planar([
        target_center[0] - owner_center[0],
        target_center[1] - owner_center[1],
        0.0,
    ]);
    let owner = normalize_planar(owner_forward);
    let target = normalize_planar(target_forward);
    dot(to_target, target) > 0.0 && dot(to_target, owner) > 0.5 && dot(target, owner) > -0.3
}

pub fn backstab_damage(target_health: i32) -> f32 {
    (target_health * 2) as f32
}

fn normalize_planar(value: [f32; 3]) -> [f32; 3] {
    let length = value[0].hypot(value[1]);
    if length == 0.0 {
        [0.0; 3]
    } else {
        [value[0] / length, value[1] / length, 0.0]
    }
}

fn dot(left: [f32; 3], right: [f32; 3]) -> f32 {
    left[0] * right[0] + left[1] * right[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloak_proxy_thresholds_observers_and_blink_are_not_alpha() {
        let render = |fraction, blink, enemy| {
            cloak_render_state(fraction, blink, false, PlayerTeam::Red, enemy, false)
        };
        assert_eq!(render(0.009, false, true).local_factor, 0.0);
        assert_eq!(render(0.01, false, true).local_factor, 0.2228);
        assert_eq!(render(1.0, false, true).local_factor, 0.5);
        assert_eq!(render(1.0, false, true).world_factor, 1.0);
        assert_eq!(render(1.0, false, false).world_factor, 0.95);
        assert_eq!(render(0.85, true, true).local_factor, 0.3);
        assert_eq!(render(0.85, true, true).world_factor, 0.85);
        let mut spy = SpyState::default();
        spy.toggle_cloak(0.0);
        spy.expose(2.0);
        spy.advance(2.25, 0.015);
        spy.reveal_invisibility(2.25, false);
        assert_eq!(spy.invisibility, 0.85);
        spy.advance(2.31, 0.015);
        spy.reveal_invisibility(2.31, false);
        assert_eq!(spy.invisibility, 1.0);
        spy.last_stealth_expose_time = None;
        spy.note_damage(3.0, 5, false);
        assert!(!spy.blink(3.0));
        spy.note_damage(3.0, 6, false);
        assert!(spy.blink(3.0));
        spy.last_stealth_expose_time = Some(0.0);
        assert!(spy.blink(0.3));
        assert!(!spy.blink(0.300001));
        spy.last_stealth_expose_time = None;
        spy.note_damage(4.0, 1, true);
        assert!(spy.blink(4.0));
    }

    #[test]
    fn backstab_requires_all_three_strict_planar_source_thresholds() {
        assert!(can_backstab(
            [0.0; 3],
            [1.0, 0.0, 90.0],
            [32.0, 0.0, -200.0],
            [1.0, 0.0, -70.0],
        ));
        assert!(!can_backstab(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [32.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ));
        assert!(!can_backstab(
            [0.0; 3],
            [0.5, 0.866_025_4, 0.0],
            [32.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        ));
        assert!(!can_backstab(
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0; 3],
            [1.0, 0.0, 0.0],
        ));
        assert_eq!(backstab_damage(125), 250.0);
        assert_eq!(backstab_damage(300), 600.0);
    }

    #[test]
    fn stock_cloak_preserves_rates_fades_threshold_and_attack_lock() {
        let mut state = SpyState::default();
        assert_eq!(state.toggle_cloak(2.0), Some(SpyEvent::Cloaked));
        assert_eq!(state.advance(2.5, 0.5), [None, None]);
        assert_eq!(state.cloak_meter, 95.0);
        assert_eq!(state.invisibility, 0.5);
        assert!(!state.can_attack(2.5));
        assert_eq!(state.toggle_cloak(3.0), Some(SpyEvent::Uncloaked));
        assert_eq!(state.advance(4.0, 1.0), [None, None]);
        assert_eq!(state.invisibility, 0.5);
        assert!(!state.can_attack(4.999));
        assert!(state.can_attack(5.0));
        assert_eq!(state.cloak_meter, 98.3);
        state.cloak_meter = 8.0;
        assert_eq!(state.toggle_cloak(6.0), None);
        state.cloak_meter = 8.001;
        assert_eq!(state.toggle_cloak(6.1), Some(SpyEvent::Cloaked));
    }

    #[test]
    fn empty_cloak_forces_decloak_without_immediate_regeneration() {
        let mut state = SpyState {
            cloak_meter: 0.1,
            cloaked: true,
            ..SpyState::default()
        };
        assert_eq!(state.advance(4.0, 0.015)[0], Some(SpyEvent::Uncloaked));
        assert_eq!(state.cloak_meter, 0.0);
        assert_eq!(state.no_attack_until, 6.0);
        state.advance(4.015, 0.015);
        assert!((state.cloak_meter - 0.0495).abs() < 0.000_001);
    }

    #[test]
    fn disguise_uses_full_then_quick_timing_and_self_selection_removes_it() {
        let mut state = SpyState::default();
        let soldier = Disguise {
            class: PlayerClass::Soldier,
            team: PlayerTeam::Blue,
        };
        assert_eq!(
            state.request_disguise(PlayerTeam::Red, soldier, 1.0),
            Some(SpyEvent::Disguising)
        );
        assert_eq!(state.request_disguise(PlayerTeam::Red, soldier, 2.0), None);
        assert_eq!(state.advance(3.0, 0.015)[1], None);
        assert_eq!(state.advance(3.015, 0.015)[1], Some(SpyEvent::Disguised));
        assert_eq!(state.disguise, Some(soldier));
        let medic = Disguise {
            class: PlayerClass::Medic,
            team: PlayerTeam::Blue,
        };
        assert_eq!(
            state.request_disguise(PlayerTeam::Red, medic, 4.0),
            Some(SpyEvent::Disguising)
        );
        assert_eq!(state.disguise_complete_time, 4.5);
        assert_eq!(state.advance(4.51, 0.015)[1], Some(SpyEvent::Disguised));
        assert_eq!(
            state.request_disguise(
                PlayerTeam::Red,
                Disguise {
                    class: PlayerClass::Spy,
                    team: PlayerTeam::Red,
                },
                5.0,
            ),
            Some(SpyEvent::DisguiseRemoved)
        );
        assert_eq!(state.disguise_wear_off_until, 5.5);
    }
}
