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

fn apply_prefire_rate(request: &ModelPoseRequest, resolved: &mut ModelPoseRequest) -> Result<(), &'static str> {
    // The viewmodel rate belongs to the publication that produced this pose.
    // A later observe may already have replaced the owner's class/loadout.
    let rate = request.prefire_playback_rate.filter(|rate| rate.is_finite() && *rate > 0.0)
        .ok_or("minigun-prefire-rate")?;
    resolved.elapsed *= rate;
    resolved.previous_elapsed *= rate;
    Ok(())
}

pub(super) fn prepare(
    request: &ModelPoseRequest,
    models: &BTreeMap<String, Arc<super::RetainedPresentationModel>>,
    states: &mut BTreeMap<u32, AnimationState>,
    gameplay: Option<&playsrc_tf2::Snapshot>,
) -> Result<Option<ModelPoseRequest>, &'static str> {
    let actor = gameplay.and_then(|snapshot| snapshot.bots.iter().find(|bot| bot.identity == request.actor_identity));
    let automatic = actor.filter(|bot| request.item_definition.is_none() && !request.model_panel && !request.class_selection
        && bot.lifecycle == playsrc_tf2::PlayerLifecycle::Active && request.model == bot.class.data().model);
    let Some(definition) = request.item_definition.or_else(|| automatic.and_then(|bot| bot.weapon_definition)) else {
        return Ok(None);
    };
    let item = equipment::schema().definition(definition).ok_or("weapon-definition")?;
    let class = PlayerClass::ALL
        .into_iter()
        .find(|class| {
            let data = class.data();
            request.model == data.hand_model
                || request.model == data.model
                || request.model == data.hwm_model
        })
        .or_else(|| (item.usable_by.len() == 1).then(|| *item.usable_by.first().unwrap()))
        .ok_or("weapon-model-class")?;
    let mut resolved = request.clone();
    if let Some(bot) = automatic {
        let presentation = equipment::presentation(definition).ok_or("world-weapon-presentation")?;
        resolved.item_definition = Some(definition);
        resolved.item = presentation.model_for_class(bot.class).filter(|model| !model.is_empty()).map(str::to_owned);
        resolved.item_bodygroups = if let Some(item) = &resolved.item { vec![0; models.get(item).ok_or("world-weapon-model")?.body_parts.len()] } else { Vec::new() };
        resolved.world_item = true;
        resolved.activity = if request.activity.starts_with("ACT_MP_RUN_") { "ACT_MP_RUN" } else { "ACT_MP_STAND_IDLE" }.to_owned();
    }
    if item.item_class == "tf_weapon_minigun" && request.activity == "ACT_MP_ATTACK_STAND_PREFIRE"
        && !request.preparation && !request.model_panel && !resolved.world_item {
        apply_prefire_rate(request, &mut resolved)?;
    }
    if item.item_class == "tf_weapon_minigun" && !request.preparation && !request.model_panel {
        let mut transient;
        let state = if request.attachments_only {
            transient = states.get(&request.identity).cloned().unwrap_or_default();
            &mut transient
        } else { states.entry(request.identity).or_default() };
        let activity = if let Some(bot) = automatic {
            if bot.weapon.is_some_and(|weapon| weapon.minigun_state != playsrc_tf2::weapon::MinigunState::Idle) { "ACT_MP_ATTACK_STAND_PREFIRE" } else { "ACT_MP_ATTACK_STAND_POSTFIRE" }
        } else { &request.activity };
        resolved.barrel_angle = Some(state.barrel.advance(definition, activity, request.frame_time));
    }
    if resolved.world_item || request.model_panel || request.class_selection {
        resolved.activity =
            weapon_presentation::world_activity(definition, class, &resolved.activity).ok_or("world-weapon-activity")?;
        return Ok(Some(resolved));
    }
    let model = models.get(&request.model).ok_or("viewmodel-model")?;
    resolved.activity =
        weapon_presentation::viewmodel_activity(definition, class, &request.activity).ok_or("viewmodel-activity")?;
    if request.activity == "ACT_VM_SWINGHARD" && sequence(model, &resolved.activity).is_none() {
        resolved.activity =
            weapon_presentation::viewmodel_activity(definition, class, "ACT_VM_HITCENTER")
                .ok_or("viewmodel-melee-activity")?;
    }
    let selected = sequence(model, &resolved.activity).ok_or("viewmodel-sequence")?;
    let parameters = vec![playsrc_studio_model::Float32(0); model.pose_parameters.len()];
    let timing =
        playsrc_studio_model::sequence_timing(model, selected, &parameters).map_err(|_| "viewmodel-sequence-timing")?;
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
        request.activity_start_tick.ok_or("viewmodel-activity-clock")?,
        &request.activity,
        request.current_time,
        finished,
        request.allow_idle_transition,
    );
    if let Some(started) = idle {
        resolved.activity =
            weapon_presentation::viewmodel_activity(definition, class, "ACT_VM_IDLE").ok_or("viewmodel-idle-activity")?;
        if sequence(model, &resolved.activity).is_none() {
            return Err("viewmodel-idle-sequence");
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
        for value in [13_u32, 1, 7, 1] {
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
        bytes.extend_from_slice(&0_f32.to_le_bytes());
        (bytes, phase)
    }

    #[test]
    fn pose13_retains_cosmetic_flags_and_uses_explicit_definition_and_clock() {
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
    fn model_failure_identifies_the_rejected_request_and_weapon_owner() {
        let (bytes, _) = request_bytes();
        let mut request = super::super::decode_model_requests(&bytes).unwrap().remove(0);
        request.model = PlayerClass::Heavy.data().hand_model.to_owned();
        request.item_definition = Some(15);
        request.activity = "ACT_MP_ATTACK_STAND_PREFIRE".into();
        request.preparation = false;
        request.model_panel = false;
        request.world_item = false;
        let models = BTreeMap::new();
        let mut states = BTreeMap::new();
        assert_eq!(prepare(&request, &models, &mut states, None).unwrap_err(), "minigun-prefire-rate");
        let metadata = BTreeMap::new();
        let mut particles = super::super::wearable::ParticleStates::default();
        let mut world = super::super::ModelPoseWorld {
            metadata: &metadata, lighting: None, visibility: None, collision: None,
            snapshot: None, gameplay: None, cubemaps: &[], particle_inputs: None,
            wearable_particles: &mut particles,
        };
        let error = super::super::encode_model_poses(&models, &BTreeMap::new(), &mut BTreeMap::new(),
            &mut BTreeMap::new(), &mut states, &[request.clone()], &mut world, Vec::new()).unwrap_err();
        assert!(error.starts_with("model pose minigun-prefire-rate: request=0 identity=7 actor=1 sample_tick=42 "), "{error}");
        assert!(error.contains("definition=Some(15) activity=\"ACT_MP_ATTACK_STAND_PREFIRE\""), "{error}");
        assert!(error.ends_with("authority_tick=None authority_class=None"), "{error}");
        // Preparation has no live weapon owner: it must reach the independent
        // model-resource check rather than report a missing gameplay snapshot.
        request.preparation = true;
        assert_eq!(prepare(&request, &models, &mut states, None).unwrap_err(), "viewmodel-model");
        assert!(states.is_empty());
    }

    #[test]
    fn delayed_prefire_uses_the_pose_publication_rate_without_a_live_weapon_owner() {
        let (mut bytes, _) = request_bytes();
        let end = bytes.len();
        bytes[end - 4..].copy_from_slice(&1.5_f32.to_le_bytes());
        let mut request = super::super::decode_model_requests(&bytes).unwrap().remove(0);
        assert_eq!(request.prefire_playback_rate, Some(1.5));
        request.model = PlayerClass::Heavy.data().hand_model.to_owned();
        request.item_definition = Some(15);
        request.activity = "ACT_MP_ATTACK_STAND_PREFIRE".into();
        request.preparation = false;
        request.model_panel = false;
        request.world_item = false;
        request.previous_elapsed = 0.25;
        request.elapsed = 0.5;
        let mut resolved = request.clone();
        apply_prefire_rate(&request, &mut resolved).unwrap();
        assert_eq!((resolved.previous_elapsed, resolved.elapsed), (0.375, 0.75));
        // No latest gameplay snapshot is needed, including after its Minigun
        // owner was replaced. Only the intentionally absent model rejects this.
        assert_eq!(prepare(&request, &BTreeMap::new(), &mut BTreeMap::new(), None).unwrap_err(), "viewmodel-model");
        for invalid in [-1.0_f32, f32::INFINITY, f32::NAN] {
            bytes[end - 4..].copy_from_slice(&invalid.to_le_bytes());
            assert!(super::super::decode_model_requests(&bytes).is_err());
        }
        bytes[4..8].copy_from_slice(&12_u32.to_le_bytes());
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
