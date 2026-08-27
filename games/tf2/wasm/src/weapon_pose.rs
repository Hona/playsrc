use super::ModelPoseRequest;
use playsrc_studio_model::{PresentationModel, ViewModelPhase};
use playsrc_tf2::{class::PlayerClass, equipment, weapon_presentation};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, Default)]
pub(super) struct AnimationState {
    source: Option<(u32, u64, String)>,
    idle_started: Option<f32>,
    barrel: BarrelState,
}

#[derive(Clone, Debug, Default)]
struct BarrelState {
    definition: Option<u32>,
    velocity: f32,
    target: f32,
    angle: f32,
}

impl BarrelState {
    fn advance(&mut self, definition: u32, activity: &str, frame_time: f32) -> f32 {
        if self.definition != Some(definition) { *self = Self { definition: Some(definition), ..Self::default() }; }
        match activity {
            "ACT_VM_PRIMARYATTACK" | "ACT_MP_ATTACK_STAND_PRIMARYFIRE" | "ACT_MP_ATTACK_STAND_PREFIRE" => self.target = 20.0,
            "ACT_VM_DRAW" | "ACT_MP_ATTACK_STAND_POSTFIRE" => self.target = 0.0,
            _ => {},
        }
        if self.velocity < self.target { self.velocity = (self.velocity + 0.1).min(self.target); }
        else if self.velocity > self.target { self.velocity = (self.velocity - 0.1).max(self.target); }
        self.angle += self.velocity * frame_time;
        self.angle
    }
}

pub(super) fn bone_rotations(request: &ModelPoseRequest, model: &PresentationModel) -> Vec<(usize, [playsrc_studio_model::Float32; 4])> {
    let Some(angle) = request.barrel_angle else { return Vec::new(); };
    let Some(bone) = model.bones.iter().position(|bone| bone.name.eq_ignore_ascii_case(b"barrel")) else { return Vec::new(); };
    vec![(bone, [0.0, 0.0, (angle * 0.5).sin(), (angle * 0.5).cos()].map(|value| playsrc_studio_model::Float32(value.to_bits())))]
}

impl AnimationState {
    fn idle_start(
        &mut self,
        definition: u32,
        start_tick: u64,
        activity: &str,
        now: f32,
        finished: bool,
        allow: bool,
    ) -> Option<f32> {
        if self.source.as_ref().is_none_or(|(item, tick, source)| {
            *item != definition || *tick != start_tick || source != activity
        }) {
            self.source = Some((definition, start_tick, activity.to_owned()));
            self.idle_started = None;
        }
        if finished && allow && self.idle_started.is_none() {
            self.idle_started = Some(now);
        }
        self.idle_started
    }
}

fn sequence(model: &PresentationModel, activity: &str) -> Option<usize> {
    playsrc_studio_model::sequences_for_activity_name(model, activity.as_bytes())
        .first()
        .copied()
        .or_else(|| {
            model
                .sequences
                .iter()
                .find(|sequence| sequence.label.eq_ignore_ascii_case(activity.as_bytes()))
                .map(|sequence| sequence.index)
        })
}

pub(super) fn prepare(
    request: &ModelPoseRequest,
    models: &BTreeMap<String, Arc<super::RetainedPresentationModel>>,
    states: &mut BTreeMap<u32, AnimationState>,
    gameplay: Option<&playsrc_tf2::Snapshot>,
) -> Result<Option<ModelPoseRequest>, ()> {
    let Some(definition) = request.item_definition else {
        return Ok(None);
    };
    let item = equipment::schema().definition(definition).ok_or(())?;
    let class = PlayerClass::ALL
        .into_iter()
        .find(|class| {
            let data = class.data();
            request.model == data.hand_model
                || request.model == data.model
                || request.model == data.hwm_model
        })
        .or_else(|| (item.usable_by.len() == 1).then(|| *item.usable_by.first().unwrap()))
        .ok_or(())?;
    let mut resolved = request.clone();
    if item.item_class == "tf_weapon_minigun" && request.activity == "ACT_MP_ATTACK_STAND_PREFIRE"
        && !request.preparation && !request.model_panel && !request.world_item {
        let gameplay = gameplay.ok_or(())?;
        let weapon = if request.actor_identity <= 1 {
            gameplay.loadout.iter().find(|weapon| weapon.weapon == playsrc_tf2::Weapon::Minigun)
        } else {
            gameplay.bots.iter().find(|bot| bot.identity == request.actor_identity).and_then(|bot| bot.weapon.as_ref())
        }.ok_or(())?;
        resolved.elapsed *= weapon.prefire_playback_rate;
        resolved.previous_elapsed *= weapon.prefire_playback_rate;
    }
    if item.item_class == "tf_weapon_minigun" && !request.preparation && !request.model_panel {
        let mut transient;
        let state = if request.attachments_only {
            transient = states.get(&request.identity).cloned().unwrap_or_default();
            &mut transient
        } else { states.entry(request.identity).or_default() };
        resolved.barrel_angle = Some(state.barrel.advance(definition, &request.activity, request.frame_time));
    }
    if request.world_item || request.model_panel || request.class_selection {
        resolved.activity =
            weapon_presentation::world_activity(definition, class, &request.activity).ok_or(())?;
        return Ok(Some(resolved));
    }
    let model = models.get(&request.model).ok_or(())?;
    resolved.activity =
        weapon_presentation::viewmodel_activity(definition, class, &request.activity).ok_or(())?;
    if request.activity == "ACT_VM_SWINGHARD" && sequence(model, &resolved.activity).is_none() {
        resolved.activity =
            weapon_presentation::viewmodel_activity(definition, class, "ACT_VM_HITCENTER")
                .ok_or(())?;
    }
    let selected = sequence(model, &resolved.activity).ok_or(())?;
    let parameters = vec![playsrc_studio_model::Float32(0); model.pose_parameters.len()];
    let timing =
        playsrc_studio_model::sequence_timing(model, selected, &parameters).map_err(|_| ())?;
    let duration = f32::from_bits(timing.duration_seconds.0);
    let finished = request.phase != Some(ViewModelPhase::Idle)
        && !timing.looping
        && resolved.elapsed >= duration;
    let mut transient = AnimationState::default();
    let state = if request.preparation || request.attachments_only {
        &mut transient
    } else {
        states.entry(request.identity).or_default()
    };
    let idle = state.idle_start(
        definition,
        request.activity_start_tick.ok_or(())?,
        &request.activity,
        request.current_time,
        finished,
        request.allow_idle_transition,
    );
    if let Some(started) = idle {
        resolved.activity =
            weapon_presentation::viewmodel_activity(definition, class, "ACT_VM_IDLE").ok_or(())?;
        if sequence(model, &resolved.activity).is_none() {
            return Err(());
        }
        resolved.elapsed = (request.current_time - started).max(0.0);
        resolved.previous_elapsed = (resolved.elapsed - request.frame_time).max(0.0);
        if resolved.phase.is_some() {
            resolved.phase = Some(ViewModelPhase::Idle);
        }
    }
    Ok(Some(resolved))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minigun_barrel_uses_source_per_call_acceleration_and_frame_time() {
        let mut barrel = BarrelState::default();
        assert!((barrel.advance(15, "ACT_MP_ATTACK_STAND_PREFIRE", 0.015) - 0.0015).abs() < 0.000001);
        assert!((barrel.advance(15, "ACT_VM_SECONDARYATTACK", 0.03) - 0.0075).abs() < 0.000001);
        assert!((barrel.advance(15, "ACT_MP_ATTACK_STAND_POSTFIRE", 0.015) - 0.009).abs() < 0.000001);
        assert!((barrel.advance(15, "ACT_VM_IDLE", 0.015) - 0.009).abs() < 0.000001);
        assert_eq!(barrel.velocity, 0.0);
        assert_eq!(barrel.advance(424, "ACT_VM_DRAW", 0.015), 0.0);
    }

    fn request_bytes() -> (Vec<u8>, usize) {
        let mut bytes = b"PMRQ".to_vec();
        for value in [12_u32, 1, 7, 1] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&[0; 24]);
        bytes.extend_from_slice(&42_u64.to_le_bytes());
        bytes.extend_from_slice(&[6, 0, 0, 12]);
        bytes.extend_from_slice(&18_u32.to_le_bytes());
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        bytes.extend_from_slice(&[0; 28]);
        for text in [
            "models/player/soldier.mdl",
            "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl",
            "ACT_MP_STAND_IDLE",
        ] {
            bytes.extend_from_slice(&(text.len() as u32).to_le_bytes());
            bytes.extend_from_slice(text.as_bytes());
        }
        for value in [0.0_f32, 0.0, 1.0, 0.015, 0.0, 4.0 / 3.0, 32768.0] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&[0; 8]);
        let phase = bytes.len();
        bytes.extend_from_slice(&[255, 0, 1, 0]);
        bytes.extend_from_slice(&i32::MIN.to_le_bytes());
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(&[0; 52]);
        (bytes, phase)
    }

    #[test]
    fn pose12_retains_cosmetic_flags_and_uses_explicit_definition_and_clock() {
        let (mut bytes, phase) = request_bytes();
        let requests = super::super::decode_model_requests(&bytes).unwrap();
        let request = &requests[0];
        assert_eq!(request.actor_identity, 1);
        assert!(request.cloak.is_none());
        assert!(
            request.model_panel && request.world_item && request.preparation && request.hud_model
        );
        assert_eq!(request.item_definition, Some(18));
        assert_eq!(request.activity_start_tick, None);
        bytes[52] = 1;
        bytes[55] = 0;
        bytes[phase] = 0;
        assert!(super::super::decode_model_requests(&bytes).is_err());
        bytes[60..68].copy_from_slice(&7_u64.to_le_bytes());
        bytes[phase + 3] = 1;
        let requests = super::super::decode_model_requests(&bytes).unwrap();
        assert_eq!(requests[0].activity_start_tick, Some(7));
        assert!(requests[0].allow_idle_transition);
        bytes[56..60].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
        assert!(super::super::decode_model_requests(&bytes).is_err());
    }

    #[test]
    fn exact_start_tick_distinguishes_coalesced_repeated_attacks_and_idle_is_retained() {
        let mut clock = AnimationState::default();
        assert_eq!(
            clock.idle_start(18, 5, "ACT_VM_PRIMARYATTACK", 1.0, false, true),
            None
        );
        assert_eq!(
            clock.idle_start(18, 5, "ACT_VM_PRIMARYATTACK", 2.0, true, false),
            None
        );
        assert_eq!(
            clock.idle_start(18, 5, "ACT_VM_PRIMARYATTACK", 2.1, true, true),
            Some(2.1)
        );
        assert_eq!(
            clock.idle_start(18, 5, "ACT_VM_PRIMARYATTACK", 2.2, true, false),
            Some(2.1)
        );
        assert_eq!(
            clock.idle_start(18, 100, "ACT_VM_PRIMARYATTACK", 3.0, false, true),
            None
        );
        assert_eq!(
            clock.idle_start(18, 100, "ACT_VM_IDLE", 3.1, false, true),
            None
        );
    }
}
