use super::Weapon;
use crate::class::AmmoType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponSource {
    pub owner: u32,
    pub definition_index: u32,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttributeTarget {
    Weapon,
    Player,
}

/// Live local-game inputs to the SDK's clip, ammo, firing, and reload hooks.
/// Resolve again when these inputs or equipment providers change. Existing
/// attack/reload deadlines are not retroactively rescaled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfileContext {
    pub ammo: Option<AmmoType>,
    pub gun: bool,
    pub blast_impact: bool,
    pub decapitations: i32,
    pub kill_combo: i32,
    pub health_fraction: f32,
    pub blast_jumping: bool,
    pub healer_count: usize,
    pub reload_speed_scale: f32,
}

impl Default for ProfileContext {
    fn default() -> Self {
        Self {
            ammo: None,
            gun: false,
            blast_impact: false,
            decapitations: 0,
            kill_combo: 0,
            health_fraction: 1.0,
            blast_jumping: false,
            healer_count: 0,
            reload_speed_scale: 1.0,
        }
    }
}

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
    MeleeCritical,
    MeleePrimary,
    FistLeft,
    FistRight,
    Prefire,
    Postfire,
    Pullback,
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
    /// Apply the normal local TF2 weapon hooks to an unmodified script profile.
    /// The closure must query the equipped AttributeGraph, not a second table
    /// of item statistics. Integer hooks use Source's nearest-even conversion.
    pub fn with_attributes(
        mut self,
        context: ProfileContext,
        mut query: impl FnMut(AttributeTarget, &str, f32) -> f32,
    ) -> Self {
        use AttributeTarget::{Player, Weapon};
        if self.maximum_clip != 0 {
            let clip = query(Weapon, "mult_clipsize", self.maximum_clip as f32).round_ties_even();
            self.maximum_clip = if context.blast_impact {
                let atomic =
                    query(Weapon, "mult_clipsize_upgrade_atomic", 0.0).round_ties_even() as i32;
                let on_kill =
                    query(Weapon, "clipsize_increase_on_kill", 0.0).round_ties_even() as i32;
                let earned = if on_kill != 0 {
                    context.decapitations.min(on_kill)
                } else {
                    0
                };
                (clip + (atomic + earned) as f32) as u16
            } else {
                query(Weapon, "mult_clipsize_upgrade", clip).round_ties_even() as u16
            };
        }
        if let Some(ammo) = context.ammo {
            let hook = match ammo {
                AmmoType::Primary => "mult_maxammo_primary",
                AmmoType::Secondary => "mult_maxammo_secondary",
                AmmoType::Metal => "mult_maxammo_metal",
                AmmoType::Grenades1 => "mult_maxammo_grenades1",
                AmmoType::Grenades2 => "",
            };
            if !hook.is_empty() {
                self.maximum_reserve =
                    query(Player, hook, self.maximum_reserve as f32).round_ties_even() as u16;
            }
        }

        let delay_multiplier = query(Weapon, "mult_postfiredelay", 1.0)
            - query(Weapon, "kill_combo_fire_rate_boost", 0.0) * context.kill_combo as f32;
        self.fire_delay *= delay_multiplier;
        if context.gun {
            self.fire_delay = query(Player, "hwn_mult_postfiredelay", self.fire_delay);
            let reduced_health = query(Weapon, "mult_postfiredelay_with_reduced_health", 1.0);
            if reduced_health != 1.0 {
                let fraction = ((context.health_fraction - 0.2) / (0.9 - 0.2)).clamp(0.0, 1.0);
                self.fire_delay *= reduced_health + (1.0 - reduced_health) * fraction;
            }
            self.fire_delay = query(
                Weapon,
                if context.blast_jumping {
                    "rocketjump_attackrate_bonus"
                } else {
                    "mul_nonrocketjump_attackrate"
                },
                self.fire_delay,
            );
            if self.maximum_clip == 0 {
                self.fire_delay = query(Weapon, "fast_reload", self.fire_delay);
            }
        }
        for time in [&mut self.reload_start, &mut self.reload_round] {
            // A zero profile duration denotes no such reload phase. Do not
            // create a new phase for clipless or magazine-reloaded weapons.
            if *time == 0.0 {
                continue;
            }
            for hook in ["mult_reload_time", "mult_reload_time_hidden", "fast_reload"] {
                *time = query(Weapon, hook, *time);
            }
            *time = query(Player, "hwn_mult_reload_time", *time);
            *time *= context.reload_speed_scale;
            if context.healer_count == 1 {
                *time = query(Player, "mult_reload_time_while_healed", *time);
            }
            *time = time.max(0.00001);
        }
        self
    }

    pub const fn configured(weapon: Weapon) -> Self {
        match weapon {
            Weapon::HandgunScoutPrimary => Self {
                maximum_clip: 4, maximum_reserve: 32, fire_delay: 0.35,
                // Configured ACT_SECONDARY_VM_RELOAD_2 is 37 frames / 30fps;
                // DefaultReload uses duration - 0.2 before reload-time attributes.
                reload_start: 1.0, reload_round: 0.0, maximum_charge: None,
                center_fire_projectile: false, flip_viewmodel: false,
            },
            Weapon::RocketLauncher | Weapon::DirectHit | Weapon::BlackBox
            | Weapon::LibertyLauncher | Weapon::RocketJumper | Weapon::AirStrike => Self {
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
            Weapon::FlareGun | Weapon::Detonator | Weapon::ScorchShot | Weapon::Manmelter => Self {
                maximum_clip: if matches!(weapon, Weapon::Manmelter) { 20 } else { 0 },
                maximum_reserve: 32,
                fire_delay: 2.0,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::GrenadeLauncher => Self {
                maximum_clip: 4,
                maximum_reserve: 16,
                fire_delay: 0.6,
                // Authored g_reload_start: 19 frames at 30 FPS. Reload loops
                // deliberately use the script duration in CTFWeaponBase.
                reload_start: 18.0 / 30.0,
                reload_round: 0.6,
                maximum_charge: None,
                center_fire_projectile: false,
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
            Weapon::EngineerShotgun => Self {
                maximum_clip: 6,
                maximum_reserve: 32,
                fire_delay: 0.625,
                reload_start: 0.333_333_34,
                reload_round: 0.5,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Pistol | Weapon::EngineerPistol => Self {
                maximum_clip: 12,
                maximum_reserve: if matches!(weapon, Weapon::EngineerPistol) {
                    200
                } else {
                    36
                },
                fire_delay: 0.15,
                reload_start: if matches!(weapon, Weapon::EngineerPistol) {
                    1.033_333_3
                } else {
                    0.5
                },
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Bat | Weapon::Shovel | Weapon::Bottle | Weapon::Wrench => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: if matches!(weapon, Weapon::Shovel | Weapon::Bottle | Weapon::Wrench) {
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
            Weapon::Fists | Weapon::BuildPda | Weapon::DestroyPda | Weapon::Toolbox => Self {
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
            Weapon::SyringeGun => Self {
                maximum_clip: crate::medic::SYRINGE_CLIP,
                maximum_reserve: crate::medic::SYRINGE_RESERVE,
                fire_delay: crate::medic::SYRINGE_FIRE_DELAY,
                reload_start: crate::medic::SYRINGE_RELOAD_SECONDS,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::MediGun => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: 0.5,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Bonesaw => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: crate::medic::BONESAW_FIRE_DELAY,
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

            Weapon::Revolver => Self {
                maximum_clip: 6,
                maximum_reserve: 24,
                fire_delay: 0.5,
                reload_start: 1.133_333_3,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Knife => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: 0.8,
                reload_start: 0.0,
                reload_round: 0.0,
                maximum_charge: None,
                center_fire_projectile: false,
                flip_viewmodel: false,
            },
            Weapon::Sapper | Weapon::DisguiseKit | Weapon::InvisibilityWatch => Self {
                maximum_clip: 0,
                maximum_reserve: 0,
                fire_delay: 0.0,
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

pub fn minigun_aiming_transition(previous: MinigunState, current: MinigunState) -> Option<bool> {
    if current == MinigunState::Starting && previous == MinigunState::Idle { Some(true) }
    else if current == MinigunState::Idle && previous != MinigunState::Idle { Some(false) }
    else { None }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRuntime {
    pub hitscan: crate::hitscan::State,
    pub deploy_multiplier: f32,
    pub spinup_seconds: f32,
    pub postfire_until: Option<f32>,
    pub discard_chambered_on_reload: bool,
    pub generation: u64,
    pub critical: crate::critical::WeaponState,
    pub last_flare_deny_time: f32,
    pub last_extinguish_time: f32,
    pub resolved_profile: WeaponProfile,
    pub weapon: Weapon,
    pub clip: u16,
    pub reserve: u16,
    pub reload: ReloadPhase,
    pub next_primary_tick: u64,
    pub reload_due_tick: Option<u64>,
    reload_prior_next_primary_tick:u64,
    pub charge_begin_tick: Option<u64>,
    pub first_primary_tick: u64,

    pub minigun_state: MinigunState,
    pub spin_begin_tick: Option<u64>,
    pub firing_begin_tick: Option<u64>,
    pub idle_due_tick: Option<u64>,
    pub smack_due_tick: Option<u64>,
    pub push_due_time: Option<f32>,
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
    pub fn prefire_playback_rate(&self) -> f32 {
        if self.weapon == Weapon::Minigun { 0.75 / self.spinup_seconds.max(0.00001) } else { 1.0 }
    }

    pub fn full_with_attributes(weapon: Weapon, context: ProfileContext,
        mut query: impl FnMut(AttributeTarget, &str, f32) -> f32) -> Self {
        let mut base = WeaponProfile::configured(weapon);
        let discard = weapon == Weapon::Scattergun && query(AttributeTarget::Weapon, "set_scattergun_no_reload_single", 0.0) == 1.0;
        if discard {
            // c_scout_arms ACT_ITEM2_VM_RELOAD has 50 frames at 30 fps;
            // DefaultReload finishes 0.2 seconds before that sequence ends.
            base.reload_start = 49.0 / 30.0 - 0.2;
            base.reload_round = 0.0;
        }
        let profile = base.with_attributes(context, &mut query);
        let mut runtime = Self::full_with_profile(weapon, profile);
        runtime.discard_chambered_on_reload = discard;
        runtime.spinup_seconds = query(AttributeTarget::Weapon, "mult_minigun_spinup_time", 0.75);
        runtime
    }

    pub fn full(weapon: Weapon) -> Self {
        Self::full_with_profile(weapon, WeaponProfile::configured(weapon))
    }

    pub fn full_with_profile(weapon: Weapon, profile: WeaponProfile) -> Self {
        Self {
            generation: 0,
            critical: crate::critical::WeaponState::default(),
            hitscan: crate::hitscan::State::default(),
            deploy_multiplier: 1.0,
            spinup_seconds: 0.75,
            postfire_until: None,
            discard_chambered_on_reload: false,
            last_flare_deny_time: 0.0,
            last_extinguish_time: 0.0,
            resolved_profile: profile,
            weapon,
            clip: profile.maximum_clip,
            reserve: profile.maximum_reserve,
            reload: ReloadPhase::Ready,
            next_primary_tick: 0,
            reload_due_tick: None,
            reload_prior_next_primary_tick:0,
            charge_begin_tick: None,
            first_primary_tick: 0,

            minigun_state: MinigunState::Idle,
            spin_begin_tick: None,
            firing_begin_tick: None,
            idle_due_tick: None,
            smack_due_tick: None,
            push_due_time: None,
            charged_damage: 0.0,
            next_secondary_tick: 0,
            unzoom_due_tick: None,
            rezoom_due_tick: None,
            rezoom_after_shot: false,
        }
    }

    pub fn profile(self) -> WeaponProfile {
        self.resolved_profile
    }

    pub fn charge_progress(self, tick: u64, interval: f32) -> Option<f32> {
        let maximum = self.profile().maximum_charge?;
        if maximum == 0.0 { return None; }
        let begin = self.charge_begin_tick.map_or(0.0, |tick| tick as f32 * interval);
        Some(if begin > 0.0 {
            ((tick as f32 * interval - begin).max(0.0) / maximum).min(1.0)
        } else { 0.0 })
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
        self.hitscan.consecutive_shots = 0;
        self.charge_begin_tick = None;
        self.next_primary_tick = self
            .next_primary_tick
            .max(tick.saturating_add(delay_ticks(0.5 * self.deploy_multiplier, tick_interval)));
        self.first_primary_tick = self.next_primary_tick;
        self.next_secondary_tick = self.next_secondary_tick.max(tick.saturating_add(delay_ticks(0.5 * self.deploy_multiplier, tick_interval)));
    }

    pub fn refill(&mut self) {
        let profile = self.profile();
        self.clip = profile.maximum_clip;
        self.reserve = profile.maximum_reserve;
        self.abort_reload();
        self.charge_begin_tick = None;

        self.minigun_state = MinigunState::Idle;
        self.postfire_until = None;
        self.spin_begin_tick = None;
        self.firing_begin_tick = None;
        self.idle_due_tick = None;
        self.smack_due_tick = None;
        self.push_due_time = None;
        self.charged_damage = 0.0;
        self.next_secondary_tick = 0;
        self.unzoom_due_tick = None;
        self.rezoom_due_tick = None;
        self.rezoom_after_shot = false;
    }

    pub fn reset_for_spawn(&mut self) {
        self.refill();
        self.hitscan = crate::hitscan::State::default();
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
            || self.charge_begin_tick.is_some()
            || self.reserve == 0
            || self.clip >= profile.maximum_clip
            || tick < self.next_primary_tick
        {
            return false;
        }
        self.reload = ReloadPhase::Start;
        self.reload_due_tick = Some(source_deadline_tick(tick,profile.reload_start,tick_interval));
        self.reload_prior_next_primary_tick=self.next_primary_tick;
        self.next_primary_tick=self.next_primary_tick.max(self.reload_due_tick.unwrap());
        self.hitscan.idle_tick = self.reload_due_tick.unwrap();
        activities.push(ActivityEvent {
            tick,
            weapon: self.weapon,
            activity: if profile.reload_round == 0.0 || self.weapon == Weapon::SyringeGun || self.discard_chambered_on_reload {
                WeaponActivity::ReloadLoop
            } else {
                WeaponActivity::ReloadStart
            },
        });
        true
    }

    // Portions adapted from Valve's Source SDK 2013. Copyright Valve Corporation,
    // All rights reserved. See LICENSE.source-sdk-2013 and thirdpartylegalnotices.txt.
    pub fn reload_frame(&mut self,tick:u64,tick_interval:f32,requested:bool,activities:&mut Vec<ActivityEvent>,ammo:&mut Vec<AmmoEvent>) {
        if requested||(self.profile().reload_round>0.0&&self.clip==0) {
            if self.reload==ReloadPhase::Ready {self.start_reload(tick,tick_interval,activities);}
            else {self.advance_reload(tick,tick_interval,activities,ammo);}
        }
        if self.reload!=ReloadPhase::Ready {self.advance_reload(tick,tick_interval,activities,ammo);}
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
        if tick < due || tick < self.next_primary_tick {
            return;
        }
        let profile = self.profile();
        if self.weapon == Weapon::SyringeGun && self.reload == ReloadPhase::Start {
            let transferred = self.reserve.min(profile.maximum_clip - self.clip);
            self.clip += transferred;
            self.reserve -= transferred;
            self.reload = ReloadPhase::Ready;
            self.reload_due_tick = None;
            ammo.push(AmmoEvent {
                tick,
                weapon: self.weapon,
                clip: self.clip,
                reserve: self.reserve,
            });
            return;
        }
        match self.reload {
            ReloadPhase::Start => {
                if profile.reload_round == 0.0 {
                    let inserted = (profile.maximum_clip - self.clip).min(self.reserve);
                    self.clip += inserted;
                    self.reserve = self.reserve.saturating_sub(if self.discard_chambered_on_reload { profile.maximum_clip } else { inserted });
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
                    self.reload_due_tick = Some(source_deadline_tick(tick,profile.reload_round,tick_interval));
                    self.reload_prior_next_primary_tick=self.next_primary_tick;
                    self.next_primary_tick=self.next_primary_tick.max(self.reload_due_tick.unwrap());
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
                    self.reload=ReloadPhase::Start;
                    self.reload_due_tick=Some(tick);
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
        self.hitscan.idle_tick = self.reload_due_tick.unwrap_or(tick);
    }

    pub fn minigun_penalties(self, tick: u64, tick_interval: f32) -> (f32, f32) {
        let prefire = self.spin_begin_tick.map_or(0.0, |begin| {
            elapsed_seconds(begin, tick, tick_interval) - self.spinup_seconds
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
        if held && self.clip>0 && self.reload!=ReloadPhase::Ready
            && self.reload_due_tick.is_some_and(|due|tick<due)
            && self.profile().reload_round>0.0 {
            self.next_primary_tick=tick.max(self.reload_prior_next_primary_tick);
            self.abort_reload();
            return PrimaryResult::None;
        }
        if !held && !secondary { self.hitscan.idle(tick); }
        if self.weapon == Weapon::HandgunScoutPrimary && secondary && tick >= self.next_secondary_tick {
            self.next_primary_tick = source_deadline_tick(tick, 0.6, tick_interval);
            self.next_secondary_tick = source_deadline_tick(tick, 1.5, tick_interval);
            self.push_due_time = Some(tick as f32 * tick_interval + 0.2);
            activities.push(ActivityEvent { tick, weapon: self.weapon, activity: WeaponActivity::SecondaryAttack });
        }
        if self.weapon == Weapon::Minigun {
            return self.minigun_attack(tick, tick_interval, held, secondary, activities);
        }
        if self.weapon == Weapon::Fists {
            if (held || secondary) && tick >= self.next_primary_tick {
                self.next_primary_tick =
                    tick.saturating_add(delay_ticks(self.profile().fire_delay, tick_interval));
                self.smack_due_tick = Some(tick.saturating_add(delay_ticks(0.2, tick_interval)));
                activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: if secondary { WeaponActivity::FistRight } else { WeaponActivity::FistLeft },
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
                let charge = (tick as f32 * tick_interval - begin as f32 * tick_interval).min(maximum_charge);
                if (released && self.clip > 0) || charge >= maximum_charge {
                    return self.commit_shot(tick, tick_interval, charge, activities);
                }
                return PrimaryResult::None;
            }
            if held && self.clip > 0 && tick >= self.next_primary_tick {
                self.charge_begin_tick = Some(tick);
                self.abort_reload();
                if self.weapon == Weapon::StickybombLauncher {
                    activities.push(ActivityEvent { tick, weapon: self.weapon, activity: WeaponActivity::Pullback });
                }
                return PrimaryResult::ChargeStarted;
            }
            return PrimaryResult::None;
        }
        let available = match self.weapon {
            Weapon::SniperRifle | Weapon::FlareGun | Weapon::Detonator | Weapon::ScorchShot => self.reserve > 0,
            Weapon::Manmelter => true,
            Weapon::Bat
            | Weapon::Shovel
            | Weapon::Kukri
            | Weapon::Wrench
            | Weapon::FireAxe
            | Weapon::Bottle
            | Weapon::Knife
            | Weapon::Bonesaw => true,
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
                self.postfire_until = Some(tick as f32 * tick_interval + 40.0 / 30.0);
                self.spin_begin_tick = None;
                self.firing_begin_tick = None;
                self.idle_due_tick = Some(tick.saturating_add(delay_ticks(2.0, tick_interval)));
                activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: WeaponActivity::Postfire,
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
                self.postfire_until = None;
                self.spin_begin_tick = Some(tick);
                self.firing_begin_tick = None;
                self.next_primary_tick = source_deadline_tick(tick, self.spinup_seconds, tick_interval);
                self.idle_due_tick = Some(self.next_primary_tick);
                activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: WeaponActivity::Prefire,
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
            MinigunState::Spinning => {
                self.firing_begin_tick = None;
                if primary && tick >= self.next_primary_tick {
                    self.minigun_state = if self.reserve > 0 { MinigunState::Firing } else { MinigunState::DryFire };
                }
                activities.push(ActivityEvent { tick, weapon: self.weapon, activity: WeaponActivity::SecondaryAttack });
            }
            MinigunState::DryFire => {
                self.firing_begin_tick = None;
                self.spin_begin_tick = None;
                if !primary && secondary {
                    self.minigun_state = MinigunState::Spinning;
                } else if self.reserve > 0 {
                    self.minigun_state = MinigunState::Firing;
                }
                activities.push(ActivityEvent { tick, weapon: self.weapon, activity: WeaponActivity::SecondaryAttack });
            }
            MinigunState::Firing if primary && tick >= self.next_primary_tick => {
                if self.reserve == 0 {
                    self.minigun_state = MinigunState::DryFire;
                    return PrimaryResult::None;
                }
                let began_firing = self.firing_begin_tick.is_none();
                self.firing_begin_tick.get_or_insert(tick);
                self.reserve -= 1;
                self.next_primary_tick =
                    tick.saturating_add(delay_ticks(self.profile().fire_delay, tick_interval));
                self.idle_due_tick = Some(tick.saturating_add(delay_ticks(0.2, tick_interval)));
                if began_firing { activities.push(ActivityEvent {
                    tick,
                    weapon: self.weapon,
                    activity: WeaponActivity::PrimaryAttack,
                }); }
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
            Weapon::SniperRifle | Weapon::FlareGun | Weapon::Detonator | Weapon::ScorchShot => self.reserve -= 1,
            Weapon::Manmelter => {},
            Weapon::Bat
            | Weapon::Shovel
            | Weapon::Kukri
            | Weapon::Wrench
            | Weapon::FireAxe
            | Weapon::Bottle
            | Weapon::Knife
            | Weapon::Bonesaw => {}
            _ => self.clip -= 1,
        }
        self.abort_reload();
        if self.weapon != Weapon::SniperRifle {
            self.charge_begin_tick = None;
        }
        self.next_primary_tick = source_deadline_tick(tick, self.profile().fire_delay, tick_interval);
        activities.push(ActivityEvent {
            tick,
            weapon: self.weapon,
            activity: if matches!(self.weapon, Weapon::Bat | Weapon::Shovel | Weapon::Kukri | Weapon::Wrench | Weapon::FireAxe | Weapon::Bottle | Weapon::Knife | Weapon::Bonesaw) {
                WeaponActivity::MeleePrimary
            } else { WeaponActivity::PrimaryAttack },
        });
        PrimaryResult::Fired { charge_seconds }
    }
}

pub fn delay_ticks(seconds: f32, tick_interval: f32) -> u64 {
    (seconds / tick_interval).ceil() as u64
}

pub fn source_deadline_tick(tick: u64, seconds: f32, tick_interval: f32) -> u64 {
    let deadline = tick as f32 * tick_interval + seconds;
    let mut due = tick.saturating_add((seconds / tick_interval).floor() as u64);
    while due as f32 * tick_interval < deadline {
        due = due.saturating_add(1);
    }
    due
}

fn elapsed_seconds(begin: u64, tick: u64, tick_interval: f32) -> f32 {
    tick.saturating_sub(begin) as f32 * tick_interval
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minigun_prefire_rate_uses_the_authored_spin_time_and_source_lower_bound() {
        let mut weapon = WeaponRuntime::full(Weapon::Minigun);
        assert_eq!(weapon.prefire_playback_rate(), 1.0);
        weapon.spinup_seconds = 0.5;
        assert_eq!(weapon.prefire_playback_rate(), 1.5);
        weapon.spinup_seconds = 0.0;
        assert_eq!(weapon.prefire_playback_rate(), 0.75 / 0.00001);
        assert_eq!(WeaponRuntime::full(Weapon::RocketLauncher).prefire_playback_rate(), 1.0);
    }

    #[test]
    fn shortstop_shove_has_independent_cooldown_and_does_not_consume_ammo() {
        let mut weapon = WeaponRuntime::full(Weapon::HandgunScoutPrimary);
        let mut activities = Vec::new();
        assert_eq!(weapon.attack(100, 0.015, true, true, false, &mut activities), PrimaryResult::None);
        assert_eq!((weapon.next_primary_tick, weapon.next_secondary_tick, weapon.push_due_time), (140, 200, Some(1.7)));
        assert_eq!(weapon.clip, 4);
        assert_eq!(activities[0].activity, WeaponActivity::SecondaryAttack);
        assert_eq!(weapon.attack(139, 0.015, true, true, false, &mut activities), PrimaryResult::None);
        assert!(matches!(weapon.attack(140, 0.015, true, true, false, &mut activities), PrimaryResult::Fired { .. }));
        assert_eq!(weapon.clip, 3);
        assert_eq!(weapon.next_secondary_tick, 200);
    }

    #[test]
    fn minigun_prefire_and_postfire_keep_the_looped_primary_action_clock() {
        let mut weapon = WeaponRuntime::full(Weapon::Minigun);
        weapon.spinup_seconds = 0.6;
        let mut activities = Vec::new();
        weapon.attack(100, 0.015, true, false, false, &mut activities);
        assert_eq!(weapon.next_primary_tick, 140);
        assert_eq!(activities[0].activity, WeaponActivity::Prefire);
        for tick in 101..200 { weapon.attack(tick, 0.015, true, false, false, &mut activities); }
        assert_eq!(activities.iter().filter(|event| event.activity == WeaponActivity::PrimaryAttack).count(), 1);
        assert!(weapon.reserve < 199);
        for tick in 200..220 { weapon.attack(tick, 0.015, false, false, false, &mut activities); }
        assert_eq!(weapon.minigun_state, MinigunState::Idle);
        assert_eq!(activities.last().unwrap().activity, WeaponActivity::Postfire);
    }

    fn rocket_context() -> ProfileContext {
        ProfileContext {
            ammo: Some(AmmoType::Primary),
            gun: true,
            blast_impact: true,
            ..ProfileContext::default()
        }
    }

    #[test]
    fn profile_hooks_preserve_stock_values_and_do_not_invent_reload_phases() {
        for weapon in [
            Weapon::RocketLauncher,
            Weapon::Original,
            Weapon::StickybombLauncher,
            Weapon::Scattergun,
            Weapon::Pistol,
            Weapon::Bat,
            Weapon::Shotgun,
            Weapon::Shovel,
            Weapon::Minigun,
            Weapon::HeavyShotgun,
            Weapon::Fists,
            Weapon::SniperRifle,
            Weapon::Smg,
            Weapon::Kukri,
            Weapon::Flamethrower,
            Weapon::FireAxe,
            Weapon::SyringeGun,
            Weapon::MediGun,
            Weapon::Bonesaw,
            Weapon::EngineerShotgun,
            Weapon::EngineerPistol,
            Weapon::Wrench,
            Weapon::Revolver,
            Weapon::Knife,
        ] {
            let script = WeaponProfile::configured(weapon);
            assert_eq!(
                script.with_attributes(rocket_context(), |_, _, input| input),
                script
            );
        }
    }

    #[test]
    fn clip_and_reserve_hooks_use_distinct_providers_and_source_integer_conversion() {
        let script = WeaponProfile::configured(Weapon::RocketLauncher);
        for (clip_multiplier, expected) in [(0.75, 3), (1.25, 5), (0.625, 2), (0.875, 4)] {
            let profile = script.with_attributes(rocket_context(), |target, hook, input| {
                match (target, hook) {
                    (AttributeTarget::Weapon, "mult_clipsize") => input * clip_multiplier,
                    (AttributeTarget::Player, "mult_maxammo_primary") => input * 3.0,
                    _ => input,
                }
            });
            assert_eq!(profile.maximum_clip, expected);
            assert_eq!(profile.maximum_reserve, 60);
            let runtime = WeaponRuntime::full_with_profile(Weapon::RocketLauncher, profile);
            assert_eq!((runtime.clip, runtime.reserve), (expected, 60));
            assert_eq!(runtime.profile(), profile);
        }
    }

    #[test]
    fn airstrike_clip_growth_is_capped_and_does_not_grant_rounds_or_retime_pending_shots() {
        let script = WeaponProfile::configured(Weapon::RocketLauncher);
        let mut runtime = WeaponRuntime::full(Weapon::RocketLauncher);
        runtime.clip = 1;
        runtime.reserve = 7;
        runtime.next_primary_tick = 100;
        runtime.reload_due_tick = Some(120);
        for (kills, expected_clip) in [(0, 4), (1, 5), (4, 8), (8, 8)] {
            runtime.resolved_profile = script.with_attributes(
                ProfileContext {
                    decapitations: kills,
                    blast_jumping: true,
                    ..rocket_context()
                },
                |target, hook, input| match (target, hook) {
                    (AttributeTarget::Weapon, "clipsize_increase_on_kill") => 4.0,
                    (AttributeTarget::Weapon, "rocketjump_attackrate_bonus") => input * 0.4,
                    _ => input,
                },
            );
            assert_eq!(runtime.profile().maximum_clip, expected_clip);
            assert_eq!(runtime.profile().fire_delay, 0.8 * 0.4);
            assert_eq!((runtime.clip, runtime.reserve), (1, 7));
            assert_eq!(runtime.next_primary_tick, 100);
            assert_eq!(runtime.reload_due_tick, Some(120));
        }
        runtime.refill();
        assert_eq!((runtime.clip, runtime.reserve), (8, 20));
    }

    #[test]
    fn reload_hooks_preserve_order_and_only_one_healer_enables_the_conditional_hook() {
        let script = WeaponProfile::configured(Weapon::RocketLauncher);
        for healers in [0, 1, 2] {
            let mut calls = Vec::new();
            let profile = script.with_attributes(
                ProfileContext {
                    healer_count: healers,
                    ..rocket_context()
                },
                |target, hook, input| {
                    if hook.contains("reload") {
                        calls.push((target, hook.to_owned()));
                    }
                    match hook {
                        "mult_reload_time" => input * 0.5,
                        "mult_reload_time_hidden" => input + 0.1,
                        "fast_reload" => input * 0.8,
                        "hwn_mult_reload_time" => input * 0.9,
                        "mult_reload_time_while_healed" => input * 0.6,
                        _ => input,
                    }
                },
            );
            let extra = if healers == 1 { 0.6 } else { 1.0 };
            assert_eq!(
                profile.reload_start,
                ((0.5 * 0.5 + 0.1) * 0.8) * 0.9 * extra
            );
            assert_eq!(
                profile.reload_round,
                ((script.reload_round * 0.5 + 0.1) * 0.8) * 0.9 * extra
            );
            let mut phase = vec![
                (AttributeTarget::Weapon, "mult_reload_time".to_owned()),
                (
                    AttributeTarget::Weapon,
                    "mult_reload_time_hidden".to_owned(),
                ),
                (AttributeTarget::Weapon, "fast_reload".to_owned()),
                (AttributeTarget::Player, "hwn_mult_reload_time".to_owned()),
            ];
            if healers == 1 {
                phase.push((
                    AttributeTarget::Player,
                    "mult_reload_time_while_healed".to_owned(),
                ));
            }
            assert_eq!(calls, [phase.clone(), phase].concat());
        }
        let profile = script.with_attributes(rocket_context(), |_, hook, input| {
            if hook == "mult_reload_time" {
                -input
            } else {
                input
            }
        });
        assert_eq!(profile.reload_start, 0.00001);
        assert_eq!(profile.reload_round, 0.00001);
    }

    #[test]
    fn resolved_profile_drives_real_ammo_consumption_refire_and_reload_deadlines() {
        let profile = WeaponProfile::configured(Weapon::RocketLauncher).with_attributes(
            rocket_context(),
            |_, hook, input| match hook {
                "mult_clipsize" => input * 0.75,
                "mult_postfiredelay" | "mult_reload_time" => input * 0.5,
                _ => input,
            },
        );
        let mut runtime = WeaponRuntime::full_with_profile(Weapon::RocketLauncher, profile);
        let mut activities = Vec::new();
        assert!(matches!(
            runtime.primary(0, 0.015, true, false, &mut activities),
            PrimaryResult::Fired { .. }
        ));
        assert_eq!(runtime.clip, 2);
        assert_eq!(runtime.next_primary_tick, delay_ticks(0.4, 0.015));
        let fire_due=runtime.next_primary_tick;
        assert!(!runtime.start_reload(runtime.next_primary_tick - 1, 0.015, &mut activities));
        assert!(runtime.start_reload(runtime.next_primary_tick, 0.015, &mut activities));
        assert_eq!(
            runtime.reload_due_tick,
            Some(fire_due + delay_ticks(0.25, 0.015))
        );
        assert_eq!(Some(runtime.next_primary_tick),runtime.reload_due_tick);
        let due = runtime.reload_due_tick.unwrap();
        runtime.advance_reload(due, 0.015, &mut activities, &mut Vec::new());
        assert_eq!(
            runtime.reload_due_tick,
            Some(due + delay_ticks(profile.reload_round, 0.015))
        );
    }

    #[test]
    fn sticky_charge_uses_float_curtimes_for_release_and_auto_fire() {
        let interval = 0.015_f32;
        for begin in [1_u64, 41, 50_000, 1_000_000, 5_000_000] {
            for held_ticks in [1, 13, 67, 133] {
                let mut runtime = WeaponRuntime::full(Weapon::StickybombLauncher);
                let mut activities = Vec::new();
                assert_eq!(runtime.primary(begin, interval, true, false, &mut activities), PrimaryResult::ChargeStarted);
                let tick = begin + held_ticks;
                let PrimaryResult::Fired { charge_seconds } = runtime.primary(tick, interval, false, true, &mut activities) else { panic!("released sticky did not fire") };
                let expected = tick as f32 * interval - begin as f32 * interval;
                assert_eq!(charge_seconds.to_bits(), expected.to_bits(), "begin={begin}, held={held_ticks}");
                assert_eq!(runtime.clip, 7);
                assert_eq!(runtime.charge_begin_tick, None);
            }
            let mut runtime = WeaponRuntime::full(Weapon::StickybombLauncher);
            let mut activities = Vec::new();
            assert_eq!(runtime.primary(begin, interval, true, false, &mut activities), PrimaryResult::ChargeStarted);
            let due = (begin + 1..begin + 300).find(|tick| *tick as f32 * interval - begin as f32 * interval >= 4.0).unwrap();
            for tick in begin + 1..due {
                assert_eq!(runtime.primary(tick, interval, true, false, &mut activities), PrimaryResult::None);
            }
            assert_eq!(runtime.primary(due, interval, true, false, &mut activities), PrimaryResult::Fired { charge_seconds: 4.0 });
            assert_eq!(runtime.clip, 7);
        }
    }

    #[test]
    fn sticky_hud_charge_preserves_float_time_progress_and_idle_sentinel() {
        let mut weapon = WeaponRuntime::full(Weapon::StickybombLauncher);
        assert_eq!(weapon.charge_progress(200, 0.015), Some(0.0));
        weapon.charge_begin_tick = Some(0);
        assert_eq!(weapon.charge_progress(200, 0.015), Some(0.0));
        for begin in [41_u64, 50_000, 5_000_000] {
            weapon.charge_begin_tick = Some(begin);
            for tick in [begin - 1, begin, begin + 1, begin + 133, begin + 267, begin + 400] {
                let expected = ((tick as f32 * 0.015_f32 - begin as f32 * 0.015_f32).max(0.0) / 4.0).min(1.0);
                assert_eq!(weapon.charge_progress(tick, 0.015).unwrap().to_bits(), expected.to_bits());
            }
        }
        assert_eq!(WeaponRuntime::full(Weapon::GrenadeLauncher).charge_progress(200, 0.015), None);
        assert_eq!(WeaponRuntime::full(Weapon::Bottle).charge_progress(200, 0.015), None);
    }

    #[test]
    fn demoman_stock_profiles_preserve_configured_scripts_and_held_primary_cadence() {
        let grenade = WeaponProfile::configured(Weapon::GrenadeLauncher);
        assert_eq!((grenade.maximum_clip, grenade.maximum_reserve), (4, 16));
        assert_eq!(
            (
                grenade.fire_delay,
                grenade.reload_start,
                grenade.reload_round
            ),
            (0.6, 0.6, 0.6)
        );
        let bottle = WeaponProfile::configured(Weapon::Bottle);
        assert_eq!(
            (
                bottle.maximum_clip,
                bottle.maximum_reserve,
                bottle.fire_delay
            ),
            (0, 0, 0.8)
        );

        let mut grenade = WeaponRuntime::full(Weapon::GrenadeLauncher);
        let mut activities = Vec::new();
        assert_eq!(
            grenade.primary(10, 0.015, true, false, &mut activities),
            PrimaryResult::Fired {charge_seconds:0.0},
        );
        assert_eq!((grenade.clip, grenade.reserve,grenade.next_primary_tick), (3, 16,50));
        assert_eq!(activities.len(),1);
        assert_eq!(grenade.primary(49,0.015,true,false,&mut activities),PrimaryResult::None);
        assert_eq!(grenade.primary(50,0.015,true,false,&mut activities),PrimaryResult::Fired {charge_seconds:0.0});
        activities.clear();

        let mut bottle = WeaponRuntime::full(Weapon::Bottle);
        assert_eq!(
            bottle.primary(10, 0.015, true, false, &mut activities),
            PrimaryResult::Fired {
                charge_seconds: 0.0
            },
        );
        assert_eq!(
            (bottle.clip, bottle.reserve, bottle.next_primary_tick),
            (0, 0, 64)
        );
        assert_eq!(activities[0].activity, WeaponActivity::MeleePrimary);
    }

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
            (0, 0, 81)
        );
        assert!(!shovel.start_reload(80, 0.01, &mut activities));
    }

    #[test]
    fn engineer_stock_profiles_use_engineer_ammo_and_authored_animation_durations() {
        let shotgun = WeaponProfile::configured(Weapon::EngineerShotgun);
        assert_eq!((shotgun.maximum_clip, shotgun.maximum_reserve), (6, 32));
        assert_eq!(
            (
                shotgun.fire_delay,
                shotgun.reload_start,
                shotgun.reload_round
            ),
            (0.625, 0.333_333_34, 0.5),
        );
        let pistol = WeaponProfile::configured(Weapon::EngineerPistol);
        assert_eq!((pistol.maximum_clip, pistol.maximum_reserve), (12, 200));
        assert_eq!(
            (pistol.fire_delay, pistol.reload_start),
            (0.15, 1.033_333_3)
        );
        let wrench = WeaponProfile::configured(Weapon::Wrench);
        assert_eq!(
            (
                wrench.maximum_clip,
                wrench.maximum_reserve,
                wrench.fire_delay
            ),
            (0, 0, 0.8)
        );
        assert_eq!(source_deadline_tick(34, 0.15, 0.015), 44);
        assert_eq!(source_deadline_tick(45, 0.15, 0.015), 56);
    }

    #[test]
    fn engineer_pistol_reloads_the_whole_magazine_at_the_authored_animation_deadline() {
        let mut pistol = WeaponRuntime::full(Weapon::EngineerPistol);
        pistol.clip = 3;
        pistol.reserve = 5;
        let mut activities = Vec::new();
        let mut ammo = Vec::new();
        assert!(pistol.start_reload(10, 0.015, &mut activities));
        assert_eq!(pistol.reload_due_tick, Some(79));
        pistol.advance_reload(78, 0.015, &mut activities, &mut ammo);
        assert_eq!((pistol.clip, pistol.reserve), (3, 5));
        pistol.advance_reload(79, 0.015, &mut activities, &mut ammo);
        assert_eq!((pistol.clip, pistol.reserve), (8, 0));
        assert_eq!(pistol.reload, ReloadPhase::Ready);
        assert_eq!(ammo.len(), 1);
    }

    #[test]
    fn pistol_reloads_the_complete_available_magazine_atomically() {
        let mut pistol = WeaponRuntime::full(Weapon::Pistol);
        pistol.clip = 3;
        pistol.reserve = 7;
        let mut activities = Vec::new();
        let mut ammo = Vec::new();
        assert!(pistol.start_reload(10, 0.01, &mut activities));
        assert_eq!(pistol.reload_due_tick,Some(61));
        pistol.advance_reload(59, 0.01, &mut activities, &mut ammo);
        assert_eq!((pistol.clip, pistol.reserve), (3, 7));
        pistol.advance_reload(60, 0.01, &mut activities, &mut ammo);
        assert_eq!((pistol.clip,pistol.reserve),(3,7));
        pistol.advance_reload(61,0.01,&mut activities,&mut ammo);
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
        assert_eq!(weapon.reload, ReloadPhase::Start);
        weapon.advance_reload(134, 0.01, &mut activities, &mut ammo);
        assert_eq!(weapon.reload, ReloadPhase::Insert);
        assert_eq!(ammo.len(), 1);

        assert_eq!(weapon.primary(135,0.01,true,false,&mut activities),PrimaryResult::None);
        assert_eq!(weapon.reload,ReloadPhase::Ready);
        assert!(matches!(
            weapon.primary(136, 0.01, true, false, &mut activities),
            PrimaryResult::Fired { .. }
        ));
        assert_eq!(weapon.reload, ReloadPhase::Ready);
        assert_eq!((weapon.clip, weapon.reserve), (2, 19));
    }

    #[test]
    fn requested_reload_and_post_frame_are_separate_source_mode_calls() {
        for requested in [false,true] {
            let mut weapon=WeaponRuntime::full(Weapon::RocketLauncher);weapon.clip=3;weapon.reserve=1;
            let mut activities=Vec::new();let mut ammo=Vec::new();
            assert!(weapon.start_reload(0,0.01,&mut activities));
            weapon.reload_frame(50,0.01,requested,&mut activities,&mut ammo);
            assert_eq!(weapon.reload,ReloadPhase::Insert);
            weapon.reload_frame(134,0.01,requested,&mut activities,&mut ammo);
            assert_eq!((weapon.clip,weapon.reserve),(4,0));
            if requested {
                assert_eq!(weapon.reload,ReloadPhase::Ready);
                assert_eq!(activities.last().unwrap().tick,134);
            }else{
                assert_eq!(weapon.reload,ReloadPhase::Finish);
                weapon.reload_frame(135,0.01,false,&mut activities,&mut ammo);
                assert_eq!(weapon.reload,ReloadPhase::Ready);
                assert_eq!(activities.last().unwrap().tick,135);
            }
            assert_eq!(activities.last().unwrap().activity,WeaponActivity::ReloadFinish);
            assert_eq!(ammo.len(),1);
        }
    }

    #[test]
    fn syringe_reloads_its_complete_clip_at_the_authored_animation_deadline() {
        let mut weapon = WeaponRuntime::full(Weapon::SyringeGun);
        weapon.clip = 3;
        weapon.reserve = 20;
        let mut activities = Vec::new();
        let mut ammo = Vec::new();
        assert!(weapon.start_reload(10, 0.01, &mut activities));
        assert_eq!(weapon.reload_due_tick, Some(140));
        assert_eq!(activities[0].activity, WeaponActivity::ReloadLoop);
        weapon.advance_reload(139, 0.01, &mut activities, &mut ammo);
        assert_eq!((weapon.clip, weapon.reserve), (3, 20));
        weapon.advance_reload(140, 0.01, &mut activities, &mut ammo);
        assert_eq!((weapon.clip, weapon.reserve), (23, 0));
        assert_eq!(weapon.reload, ReloadPhase::Ready);
        assert_eq!(ammo.len(), 1);
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
