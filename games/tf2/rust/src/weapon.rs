use super::Weapon;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReloadPhase {
    Ready = 0,
    Start = 1,
    Insert = 2,
    Finish = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeaponActivity {
    Draw,
    PrimaryAttack,
    ReloadStart,
    ReloadLoop,
    ReloadFinish,
    Idle,
    SecondaryAttack,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivityEvent {
    pub tick: u64,
    pub weapon: Weapon,
    pub activity: WeaponActivity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AmmoEvent {
    pub tick: u64,
    pub weapon: Weapon,
    pub clip: u16,
    pub reserve: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponProfile {
    pub maximum_clip: u16,
    pub maximum_reserve: u16,
    pub fire_delay: f32,
    pub reload_start: f32,
    pub reload_round: f32,
    pub maximum_charge: Option<f32>,
    pub center_fire_projectile: bool,
    pub flip_viewmodel: bool,
}

impl WeaponProfile {
    pub const fn configured(weapon: Weapon) -> Self {
        match weapon {
            Weapon::RocketLauncher => Self {
                maximum_clip: 4,
                maximum_reserve: 20,
                fire_delay: 0.8,
                reload_start: 0.5,
                reload_round: 0.833_333_3,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Original => Self {
                maximum_clip: 4,
                maximum_reserve: 20,
                fire_delay: 0.8,
                reload_start: 0.1,
                reload_round: 0.83,
                maximum_charge: None,
                center_fire_projectile: true,
                flip_viewmodel: false,
            },
            Weapon::StickybombLauncher => Self {
                maximum_clip: 8,
                maximum_reserve: 24,
                fire_delay: 0.6,
                reload_start: 0.333_333_34,
                reload_round: 0.666_666_7,
                maximum_charge: Some(4.0),
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Minigun => Self {
                maximum_clip: 0,
                maximum_reserve: 200,
                fire_delay: 0.1,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Scattergun | Weapon::Shotgun | Weapon::HeavyShotgun => Self {
                maximum_clip: 6,
                maximum_reserve: 32,
                fire_delay: 0.625,
                reload_start: 0.1,
                reload_round: 0.5,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Pistol => Self {
                maximum_clip: 12,
                maximum_reserve: 36,
                fire_delay: 0.15,
                reload_start: 0.5,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Bat | Weapon::Shovel => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: if matches!(weapon, Weapon::Shovel) {
                    0.8
                } else {
                    0.5
                },
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Fists => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: 0.8,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::SniperRifle => Self {
                maximum_clip: 0,
                maximum_reserve: 25,
                fire_delay: 1.5,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Smg => Self {
                maximum_clip: 25,
                maximum_reserve: 75,
                fire_delay: 0.1,
                reload_start: 1.233_333_3,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Kukri => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: 0.8,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Flamethrower => Self {
                maximum_clip: 0,
                maximum_reserve: 200,
                fire_delay: 0.02,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::FireAxe => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: 0.8,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum MinigunState {
    #[default]
    Idle = 0,
    Starting = 1,
    Firing = 2,
    Spinning = 3,
    DryFire = 4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRuntime {
    pub weapon: Weapon,
    pub clip: u16,
    pub reserve: u16,
    pub reload: ReloadPhase,
    pub next_primary_tick: u64,
    pub reload_due_tick: Option<u64>,
    pub charge_begin_tick: Option<u64>,
    pub first_primary_tick: u64,

    pub minigun_state: MinigunState,
    pub spin_begin_tick: Option<u64>,
    pub firing_begin_tick: Option<u64>,
    pub idle_due_tick: Option<u64>,
    pub smack_due_tick: Option<u64>,
    pub charged_damage: f32,
    pub next_secondary_tick: u64,
    pub unzoom_due_tick: Option<u64>,
    pub rezoom_due_tick: Option<u64>,
    pub rezoom_after_shot: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrimaryResult {
    None,
    ChargeStarted,
    Fired { charge_seconds: f32 },
}

impl WeaponRuntime {
    pub fn full(weapon: Weapon) -> Self {
        let profile = WeaponProfile::configured(weapon);
        Self {
            weapon,
            clip: profile.maximum_clip,
            reserve: profile.maximum_reserve,
            reload: ReloadPhase::Ready,
            next_primary_tick: 0,
            reload_due_tick: None,
            charge_begin_tick: None,
            first_primary_tick: 0,

            minigun_state: MinigunState::Idle,
            spin_begin_tick: None,
            firing_begin_tick: None,
            idle_due_tick: None,
            smack_due_tick: None,
            charged_damage: 0.0,
            next_secondary_tick: 0,
            unzoom_due_tick: None,
            rezoom_due_tick: None,
            rezoom_after_shot: false,
        }
    }

    pub fn profile(self) -> WeaponProfile {
        WeaponProfile::configured(self.weapon)
    }

    pub fn sniper_damage(self) -> Option<f32> {
        (self.weapon == Weapon::SniperRifle).then_some(self.charged_damage.max(50.0))
    }

    pub fn sniper_headshot_is_critical(
        self,
        tick: u64,
        tick_interval: f32,
        zoomed: bool,
        headshot: bool,
        crit_boosted: bool,
    ) -> bool {
        self.weapon == Weapon::SniperRifle
            && (crit_boosted
                || headshot
                    && zoomed
                    && self.charge_begin_tick.is_some_and(|start| {
                        tick.saturating_sub(start) as f32 * tick_interval >= 0.2
                    }))
    }

    pub fn regenerate(&mut self, tick: u64, tick_interval: f32) {
        self.refill();
        self.first_primary_tick = self
            .first_primary_tick
            .max(tick.saturating_add(delay_ticks(1.0, tick_interval)));
    }

    pub fn deploy(&mut self, tick: u64, tick_interval: f32) {
        self.abort_reload();
        self.charge_begin_tick = None;
        self.next_primary_tick = self
            .next_primary_tick
            .max(tick.saturating_add(delay_ticks(0.5, tick_interval)));
        self.first_primary_tick = self.next_primary_tick;
    }

    pub fn refill(&mut self) {
        let profile = self.profile();
        self.clip = profile.maximum_clip;
        self.reserve = profile.maximum_reserve;
        self.abort_reload();
        self.charge_begin_tick = None;

        self.minigun_state = MinigunState::Idle;
        self.spin_begin_tick = None;
        self.firing_begin_tick = None;
        self.idle_due_tick = None;
        self.smack_due_tick = None;
        self.charged_damage = 0.0;
        self.next_secondary_tick = 0;
        self.unzoom_due_tick = None;
        self.rezoom_due_tick = None;
        self.rezoom_after_shot = false;
    }

    pub fn reset_for_spawn(&mut self) {
        self.refill();
    }

    pub fn abort_reload(&mut self) {
        self.reload = ReloadPhase::Ready;
        self.reload_due_tick = None;
    }

    pub fn start_reload(
        &mut self,
        tick: u64,
        tick_interval: f32,
        activities: &mut Vec<ActivityEvent>,
    ) -> bool {
        let profile = self.profile();
        if self.reload != ReloadPhase::Ready
            || self.reserve == 0
            || self.clip >= profile.maximum_clip
            || tick < self.next_primary_tick
        {
            return false;
        }
        self.reload = ReloadPhase::Start;
        self.reload_due_tick =
            Some(tick.saturating_add(delay_ticks(profile.reload_start, tick_interval)));
        activities.push(ActivityEvent {
            tick,
            weapon: self.weapon,
            activity: WeaponActivity::ReloadStart,
        });
        true
    }

    pub fn advance_reload(
        &mut self,
        tick: u64,
        tick_interval: f32,
        activities: &mut Vec<ActivityEvent>,
        ammo: &mut Vec<AmmoEvent>,
    ) {
        let Some(due) = self.reload_due_tick else {
            return;
        };
        if tick < due {
            return;
        }
        let profile = self.profile();
        match self.reload {
            ReloadPhase::Start => {
                if matches!(self.weapon, Weapon::Pistol | Weapon::Smg) {
                    let inserted = (profile.maximum_clip - self.clip).min(self.reserve);
                    self.clip += inserted;
                    self.reserve -= inserted;
                    ammo.push(AmmoEvent {
                        tick,
                        weapon: self.weapon,
                        clip: self.clip,
                        reserve: self.reserve,
                    });
                    self.reload = ReloadPhase::Ready;
                    self.reload_due_tick = None;
                } else {
                    self.reload = ReloadPhase::Insert;
                    self.reload_due_tick =
                        Some(tick.saturating_add(delay_ticks(profile.reload_round, tick_interval)));
                    activities.push(ActivityEvent {
                        tick,
                        weapon: self.weapon,
                        activity: WeaponActivity::ReloadLoop,
                    });
                }
            }
            ReloadPhase::Insert => {
                if self.reserve > 0 && self.clip < profile.maximum_clip {
                    self.clip += 1;
                    self.reserve -= 1;
                    ammo.push(AmmoEvent {
                        tick,
                        weapon: self.weapon,
                        clip: self.clip,
                        reserve: self.reserve,
                    });
                }
                if self.reserve == 0 || self.clip == profile.maximum_clip {
                    self.reload = ReloadPhase::Finish;
                    self.reload_due_tick = Some(tick);
                } else {
                    self.reload_due_tick =
                        Some(tick.saturating_add(delay_ticks(profile.reload_round, tick_interval)));
                    activities.push(ActivityEvent {
                        tick,
                        weapon: self.weapon,
                        activity: WeaponActivity::ReloadLoop,
                    });
                }
            }
            ReloadPhase::Finish => {
                self.reload = ReloadPhase::Ready;
                self.reload_due_tick = None;
                activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: WeaponActivity::ReloadFinish,
                });
            }
            ReloadPhase::Ready => self.reload_due_tick = None,
        }
    }

    pub fn minigun_penalties(self, tick: u64, tick_interval: f32) -> (f32, f32) {
        let prefire = self.spin_begin_tick.map_or(0.0, |begin| {
            elapsed_seconds(begin, tick, tick_interval) - 0.75
        });
        let firing = self
            .firing_begin_tick
            .map_or(0.0, |begin| elapsed_seconds(begin, tick, tick_interval));
        let duration = prefire.max(firing);
        let damage = 0.5 + ((duration - 0.2) / 0.8).clamp(0.0, 1.0) * 0.5;
        let spread = 1.5 - duration.clamp(0.0, 1.0) * 0.5;
        (damage, spread)
    }

    pub fn primary(
        &mut self,
        tick: u64,
        tick_interval: f32,
        held: bool,
        released: bool,
        activities: &mut Vec<ActivityEvent>,
    ) -> PrimaryResult {
        self.attack(tick, tick_interval, held, false, released, activities)
    }

    pub fn attack(
        &mut self,
        tick: u64,
        tick_interval: f32,
        held: bool,
        secondary: bool,
        released: bool,
        activities: &mut Vec<ActivityEvent>,
    ) -> PrimaryResult {
        if self.weapon == Weapon::Minigun {
            return self.minigun_attack(tick, tick_interval, held, secondary, activities);
        }
        if self.weapon == Weapon::Fists {
            if (held || secondary) && tick >= self.next_primary_tick {
                self.next_primary_tick = tick.saturating_add(delay_ticks(0.8, tick_interval));
                self.smack_due_tick = Some(tick.saturating_add(delay_ticks(0.2, tick_interval)));
                activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: WeaponActivity::PrimaryAttack,
                });
                return PrimaryResult::Fired {
                    charge_seconds: if secondary { 1.0 } else { 0.0 },
                };
            }
            return PrimaryResult::None;
        }
        let profile = self.profile();
        if let Some(maximum_charge) = profile.maximum_charge {
            if let Some(begin) = self.charge_begin_tick {
                let charge = elapsed_seconds(begin, tick, tick_interval).min(maximum_charge);
                if (released && self.clip > 0) || charge >= maximum_charge {
                    return self.commit_shot(tick, tick_interval, charge, activities);
                }
                return PrimaryResult::None;
            }
            if held && self.clip > 0 && tick >= self.next_primary_tick {
                self.charge_begin_tick = Some(tick);
                self.abort_reload();
                return PrimaryResult::ChargeStarted;
            }
            return PrimaryResult::None;
        }
        let available = match self.weapon {
            Weapon::SniperRifle => self.reserve > 0,
            Weapon::Bat | Weapon::Shovel | Weapon::Kukri | Weapon::FireAxe => true,
            _ => self.clip > 0,
        };
        if held && available && tick >= self.next_primary_tick {
            return self.commit_shot(tick, tick_interval, self.charged_damage, activities);
        }
        PrimaryResult::None
    }

    fn minigun_attack(
        &mut self,
        tick: u64,
        tick_interval: f32,
        primary: bool,
        secondary: bool,
        activities: &mut Vec<ActivityEvent>,
    ) -> PrimaryResult {
        if !primary && !secondary {
            if self.minigun_state != MinigunState::Idle
                && self.idle_due_tick.is_none_or(|due| tick >= due)
            {
                self.minigun_state = MinigunState::Idle;
                self.spin_begin_tick = None;
                self.firing_begin_tick = None;
                self.idle_due_tick = Some(tick.saturating_add(delay_ticks(2.0, tick_interval)));
                activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: WeaponActivity::Idle,
                });
            }
            return PrimaryResult::None;
        }
        match self.minigun_state {
            MinigunState::Idle => {
                if tick < self.next_primary_tick {
                    return PrimaryResult::None;
                }
                self.minigun_state = MinigunState::Starting;
                self.spin_begin_tick = Some(tick);
                self.firing_begin_tick = None;
                self.next_primary_tick = tick.saturating_add(delay_ticks(0.75, tick_interval));
                self.idle_due_tick = Some(self.next_primary_tick);
                activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: WeaponActivity::Draw,
                });
            }
            MinigunState::Starting if tick >= self.next_primary_tick => {
                self.minigun_state = if primary {
                    MinigunState::Firing
                } else {
                    MinigunState::Spinning
                };
                self.next_primary_tick = tick.saturating_add(delay_ticks(0.1, tick_interval));
                self.idle_due_tick = Some(self.next_primary_tick);
            }
            MinigunState::Firing if !primary => {
                self.minigun_state = MinigunState::Spinning;
                self.firing_begin_tick = None;
                self.next_primary_tick = tick.saturating_add(delay_ticks(0.1, tick_interval));
                self.idle_due_tick = Some(self.next_primary_tick);
            }
            MinigunState::Spinning if primary && tick >= self.next_primary_tick => {
                self.minigun_state = if self.reserve > 0 {
                    MinigunState::Firing
                } else {
                    MinigunState::DryFire
                };
            }
            MinigunState::DryFire => {
                if !primary && secondary {
                    self.minigun_state = MinigunState::Spinning;
                } else if self.reserve > 0 {
                    self.minigun_state = MinigunState::Firing;
                }
            }
            MinigunState::Firing if primary && tick >= self.next_primary_tick => {
                if self.reserve == 0 {
                    self.minigun_state = MinigunState::DryFire;
                    return PrimaryResult::None;
                }
                self.firing_begin_tick.get_or_insert(tick);
                self.reserve -= 1;
                self.next_primary_tick = tick.saturating_add(delay_ticks(0.1, tick_interval));
                self.idle_due_tick = Some(tick.saturating_add(delay_ticks(0.2, tick_interval)));
                activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: WeaponActivity::PrimaryAttack,
                });
                return PrimaryResult::Fired {
                    charge_seconds: 0.0,
                };
            }
            _ => {}
        }
        PrimaryResult::None
    }

    fn commit_shot(
        &mut self,
        tick: u64,
        tick_interval: f32,
        charge_seconds: f32,
        activities: &mut Vec<ActivityEvent>,
    ) -> PrimaryResult {
        match self.weapon {
            Weapon::SniperRifle => self.reserve -= 1,
            Weapon::Bat | Weapon::Shovel | Weapon::Kukri | Weapon::FireAxe => {}
            _ => self.clip -= 1,
        }
        self.abort_reload();
        if self.weapon != Weapon::SniperRifle {
            self.charge_begin_tick = None;
        }
        self.next_primary_tick =
            tick.saturating_add(delay_ticks(self.profile().fire_delay, tick_interval));
        activities.push(ActivityEvent {
            tick,
            weapon: self.weapon,
            activity: WeaponActivity::PrimaryAttack,
        });
        PrimaryResult::Fired { charge_seconds }
    }
}

pub fn delay_ticks(seconds: f32, tick_interval: f32) -> u64 {
    (seconds / tick_interval).ceil() as u64
}

fn elapsed_seconds(begin: u64, tick: u64, tick_interval: f32) -> f32 {
    tick.saturating_sub(begin) as f32 * tick_interval
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavy_stock_profiles_and_spin_penalties_match_configured_source_scripts() {
        let minigun = WeaponProfile::configured(Weapon::Minigun);
        assert_eq!((minigun.maximum_clip, minigun.maximum_reserve), (0, 200));
        assert_eq!(minigun.fire_delay, 0.1);
        let shotgun = WeaponProfile::configured(Weapon::HeavyShotgun);
        assert_eq!((shotgun.maximum_clip, shotgun.maximum_reserve), (6, 32));
        assert_eq!(
            (
                shotgun.fire_delay,
                shotgun.reload_start,
                shotgun.reload_round
            ),
            (0.625, 0.1, 0.5)
        );
        assert_eq!(WeaponProfile::configured(Weapon::Fists).fire_delay, 0.8);

        let mut runtime = WeaponRuntime::full(Weapon::Minigun);
        let mut activities = Vec::new();
        assert_eq!(
            runtime.attack(0, 0.01, false, true, false, &mut activities),
            PrimaryResult::None
        );
        assert_eq!(runtime.minigun_state, MinigunState::Starting);
        assert_eq!(runtime.next_primary_tick, 75);
        runtime.attack(75, 0.01, false, true, false, &mut activities);
        assert_eq!(runtime.minigun_state, MinigunState::Spinning);
        runtime.attack(85, 0.01, true, false, false, &mut activities);
        assert_eq!(runtime.minigun_state, MinigunState::Firing);
        assert_eq!(
            runtime.attack(86, 0.01, true, false, false, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 0.0
            }
        );
        assert_eq!(runtime.reserve, 199);
        let (damage, spread) = runtime.minigun_penalties(86, 0.01);
        assert_eq!(damage, 0.5);
        assert!((spread - 1.445).abs() < 0.0001);
        runtime.attack(106, 0.01, false, false, false, &mut activities);
        assert_eq!(runtime.minigun_state, MinigunState::Idle);
    }

    #[test]
    fn fists_schedule_the_exact_delayed_smack_without_consuming_ammo() {
        let mut runtime = WeaponRuntime::full(Weapon::Fists);
        let mut activities = Vec::new();
        assert_eq!(
            runtime.attack(10, 0.01, true, false, false, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 0.0
            }
        );
        assert_eq!(runtime.smack_due_tick, Some(30));
        assert_eq!(runtime.next_primary_tick, 90);
        assert_eq!((runtime.clip, runtime.reserve), (0, 0));
        assert_eq!(
            runtime.attack(90, 0.01, false, true, false, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 1.0
            }
        );
    }

    #[test]
    fn configured_profiles_match_fixed_content() {
        assert_eq!(
            WeaponProfile::configured(Weapon::RocketLauncher),
            WeaponProfile {
                maximum_clip: 4,
                maximum_reserve: 20,
                fire_delay: 0.8,
                reload_start: 0.5,
                reload_round: 0.833_333_3,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            }
        );
        assert_eq!(
            WeaponProfile::configured(Weapon::StickybombLauncher),
            WeaponProfile {
                maximum_clip: 8,
                maximum_reserve: 24,
                fire_delay: 0.6,
                reload_start: 0.333_333_34,
                reload_round: 0.666_666_7,
                maximum_charge: Some(4.0),
                center_fire_projectile: false,
                flip_viewmodel: false,
            }
        );
    }

    #[test]
    fn scout_stock_weapons_preserve_script_cadence_and_class_ammo_ledgers() {
        let scattergun = WeaponProfile::configured(Weapon::Scattergun);
        assert_eq!(
            (scattergun.maximum_clip, scattergun.maximum_reserve),
            (6, 32)
        );
        assert_eq!(
            (
                scattergun.fire_delay,
                scattergun.reload_start,
                scattergun.reload_round
            ),
            (0.625, 0.1, 0.5)
        );

        let pistol = WeaponProfile::configured(Weapon::Pistol);
        assert_eq!((pistol.maximum_clip, pistol.maximum_reserve), (12, 36));
        assert_eq!((pistol.fire_delay, pistol.reload_start), (0.15, 0.5));

        let bat = WeaponProfile::configured(Weapon::Bat);
        assert_eq!((bat.maximum_clip, bat.maximum_reserve), (0, 0));
        assert_eq!(bat.fire_delay, 0.5);
    }

    #[test]
    fn soldier_stock_weapons_preserve_script_cadence_and_secondary_ammo() {
        let shotgun = WeaponProfile::configured(Weapon::HeavyShotgun);
        assert_eq!((shotgun.maximum_clip, shotgun.maximum_reserve), (6, 32));
        assert_eq!(
            (
                shotgun.fire_delay,
                shotgun.reload_start,
                shotgun.reload_round
            ),
            (0.625, 0.1, 0.5),
        );

        let mut shovel = WeaponRuntime::full(Weapon::Shovel);
        assert_eq!(
            (shovel.clip, shovel.reserve, shovel.profile().fire_delay),
            (0, 0, 0.8)
        );
        let mut activities = Vec::new();
        assert!(matches!(
            shovel.primary(0, 0.01, true, false, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 0.0
            }
        ));
        assert_eq!(
            (shovel.clip, shovel.reserve, shovel.next_primary_tick),
            (0, 0, 80)
        );
        assert!(!shovel.start_reload(80, 0.01, &mut activities));
    }

    #[test]
    fn pistol_reloads_the_complete_available_magazine_atomically() {
        let mut pistol = WeaponRuntime::full(Weapon::Pistol);
        pistol.clip = 3;
        pistol.reserve = 7;
        let mut activities = Vec::new();
        let mut ammo = Vec::new();
        assert!(pistol.start_reload(10, 0.01, &mut activities));
        pistol.advance_reload(59, 0.01, &mut activities, &mut ammo);
        assert_eq!((pistol.clip, pistol.reserve), (3, 7));
        pistol.advance_reload(60, 0.01, &mut activities, &mut ammo);
        assert_eq!((pistol.clip, pistol.reserve), (10, 0));
        assert_eq!(ammo.len(), 1);
        assert_eq!(pistol.reload, ReloadPhase::Ready);
        assert!(
            !activities
                .iter()
                .any(|event| event.activity == WeaponActivity::ReloadFinish)
        );
    }

    #[test]
    fn bat_swings_without_consuming_or_producing_ammunition() {
        let mut bat = WeaponRuntime::full(Weapon::Bat);
        let mut activities = Vec::new();
        assert_eq!(
            bat.primary(0, 0.01, true, false, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 0.0
            }
        );
        assert_eq!((bat.clip, bat.reserve, bat.next_primary_tick), (0, 0, 50));
        assert_eq!(
            bat.primary(49, 0.01, true, false, &mut activities),
            PrimaryResult::None
        );
        assert_eq!(
            bat.primary(50, 0.01, true, false, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 0.0
            }
        );
        assert!(!bat.start_reload(100, 0.01, &mut activities));
    }

    #[test]
    fn single_round_reload_preserves_all_four_phases_and_interrupts() {
        let mut weapon = WeaponRuntime::full(Weapon::RocketLauncher);
        weapon.clip = 2;
        let mut activities = Vec::new();
        let mut ammo = Vec::new();
        assert!(weapon.start_reload(0, 0.01, &mut activities));
        assert_eq!(weapon.reload, ReloadPhase::Start);
        assert_eq!(weapon.reload_due_tick, Some(50));

        weapon.advance_reload(49, 0.01, &mut activities, &mut ammo);
        assert_eq!(weapon.reload, ReloadPhase::Start);
        weapon.advance_reload(50, 0.01, &mut activities, &mut ammo);
        assert_eq!(weapon.reload, ReloadPhase::Insert);
        assert_eq!(weapon.reload_due_tick, Some(134));
        weapon.advance_reload(134, 0.01, &mut activities, &mut ammo);
        assert_eq!((weapon.clip, weapon.reserve), (3, 19));
        assert_eq!(weapon.reload, ReloadPhase::Insert);
        assert_eq!(ammo.len(), 1);

        assert!(matches!(
            weapon.primary(135, 0.01, true, false, &mut activities),
            PrimaryResult::Fired { .. }
        ));
        assert_eq!(weapon.reload, ReloadPhase::Ready);
        assert_eq!((weapon.clip, weapon.reserve), (2, 19));
    }

    #[test]
    fn sticky_charge_launches_on_release_and_force_releases_at_four_seconds() {
        let mut activities = Vec::new();
        let mut weapon = WeaponRuntime::full(Weapon::StickybombLauncher);
        assert_eq!(
            weapon.primary(0, 0.01, true, false, &mut activities),
            PrimaryResult::ChargeStarted
        );
        assert_eq!(weapon.clip, 8);
        assert_eq!(
            weapon.primary(125, 0.01, false, true, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 1.25
            }
        );
        assert_eq!(weapon.clip, 7);

        weapon.next_primary_tick = 0;
        assert_eq!(
            weapon.primary(200, 0.01, true, false, &mut activities),
            PrimaryResult::ChargeStarted
        );
        assert_eq!(
            weapon.primary(600, 0.01, true, false, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 4.0
            }
        );
        assert_eq!(weapon.clip, 6);
    }

    #[test]
    fn regeneration_refills_without_turning_the_honorbound_lock_into_a_fire_delay() {
        let mut activities = Vec::new();
        let mut weapon = WeaponRuntime::full(Weapon::RocketLauncher);
        weapon.clip = 0;
        weapon.reserve = 2;
        weapon.next_primary_tick = 40;
        weapon.regenerate(10, 0.01);
        assert_eq!((weapon.clip, weapon.reserve), (4, 20));
        assert_eq!(weapon.first_primary_tick, 110);
        assert!(matches!(
            weapon.primary(40, 0.01, true, false, &mut activities),
            PrimaryResult::Fired { .. }
        ));
    }

    #[test]
    fn deployment_applies_the_stock_switch_delay_without_shortening_a_later_deadline() {
        let mut weapon = WeaponRuntime::full(Weapon::RocketLauncher);
        weapon.deploy(10, 0.01);
        assert_eq!(weapon.next_primary_tick, 60);
        assert_eq!(weapon.first_primary_tick, 60);
        weapon.next_primary_tick = 100;
        weapon.deploy(20, 0.01);
        assert_eq!(weapon.next_primary_tick, 100);
        assert_eq!(weapon.first_primary_tick, 100);
    }
}
