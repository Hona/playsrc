use playsrc_studio_model::{Matrix3x4, PresentationModel, SampledPose};
use playsrc_tf2::equipment::EquippedItem;
use std::collections::{BTreeMap, BTreeSet};
use playsrc_particle::{ParticleWorld, ParticleMaterial, ControlPoint, Event, EventCommand, AdvanceRequest};

#[derive(Clone)]
struct State {
    model: String,
    actor: u32,
    skin: usize,
    world: Option<ParticleWorld>,
    identity: u32,
    event: u64,
    position: [f32; 3],
}

#[derive(Clone, Default)]
pub struct ParticleStates {
    states: BTreeMap<(u8, u32, u32), State>,
    serial: u32,
    pub pending: Vec<Event>,
    pub controls: BTreeMap<u32, ControlPoint>,
    event_serial: u64,
}

pub struct ParticleInputs<'a> {
    pub template: &'a ParticleWorld,
    pub materials: &'a BTreeMap<String, ParticleMaterial>,
    pub identities: &'a [String],
}

impl ParticleStates {
    pub fn retain_gameplay(&mut self, snapshot: Option<&playsrc_tf2::Snapshot>, time: f32) {
        let remove = self.states.iter().filter(|(key, state)| key.0 != 1 && !snapshot.is_some_and(|snapshot| {
            if state.actor == playsrc_tf2::PLAYER_IDENTITY {
                snapshot.health > 0.0 && snapshot.equipped_items.iter().any(|item| item.item_id == key.2)
            } else { snapshot.bots.iter().any(|bot| bot.identity == state.actor && bot.lifecycle == playsrc_tf2::PlayerLifecycle::Active && bot.equipped_items.iter().any(|item| item.item_id == key.2)) }
        })).map(|(key, state)| (*key, state.identity)).collect::<Vec<_>>();
        for (key, identity) in remove { self.states.remove(&key); if key.0 == 0 { self.queue(time, EventCommand::Destroy { effect_identity: identity }); } }
    }

    pub fn retain(&mut self, requests: &[super::ModelPoseRequest]) {
        if requests.is_empty() { self.states.retain(|key, _| key.0 != 1); return; }
        let requests = requests.iter().filter(|r| !r.preparation).collect::<Vec<_>>();
        let owners = requests.iter().map(|r| (scope(r), r.identity)).collect::<BTreeSet<_>>();
        let keys = requests.iter().flat_map(|r| r.equipped_items.iter().map(move |i| (scope(r), r.identity, i.item_id))).collect::<BTreeSet<_>>();
        let time = requests.iter().find(|r| !r.model_panel).map_or(0.0, |r| r.current_time);
        let removed = self.states.iter().filter(|(key, _)| owners.contains(&(key.0, key.1)) && !keys.contains(key)).map(|(key, s)| (*key, s.identity)).collect::<Vec<_>>();
        for (key, identity) in removed {
            self.states.remove(&key);
            if key.0 == 0 { self.queue(time, EventCommand::Destroy { effect_identity: identity }); }
        }
    }

    fn queue(&mut self, time: f32, command: EventCommand) {
        if let EventCommand::SetControlPoint { effect_identity, control_point } = command {
            self.controls.insert(effect_identity, control_point);
            return;
        }
        if let EventCommand::Destroy { effect_identity } = &command { self.controls.remove(effect_identity); }
        self.event_serial += 1;
        self.pending.push(Event { identity: 0x6000_0000_0000_0000 + self.event_serial, timestamp_seconds: time, source_order: 0, command });
    }

    pub fn sample(&mut self, inputs: &ParticleInputs<'_>, request: &super::ModelPoseRequest, item: &EquippedItem, model: &PresentationModel, pose: &SampledPose) -> Result<Vec<u8>, ()> {
        if effect(item)? == 0 { return Ok(Vec::new()); }
        if !request.model_panel && request.actor_identity == 0 { return Err(()); }
        let lighting = request.lighting.ok_or(())?;
        let local = control_point(model, pose, request.model_panel)?.0.map(|v| f32::from_bits(v.0));
        let transform = entity_transform(lighting.origin, lighting.angles, local);
        let position = [transform[3], transform[7], transform[11]];
        let key = (scope(request), request.identity, item.item_id);
        if self.states.get(&key).is_some_and(|s| s.model != model.identity || s.skin != request.skin || s.world.as_ref().is_some_and(|w| w.time() > request.current_time) || request.model_panel_reset) {
            let state = self.states.remove(&key).ok_or(())?;
            if !request.model_panel { self.queue(request.current_time, EventCommand::Destroy { effect_identity: state.identity }); }
        }
        if !self.states.contains_key(&key) {
            self.serial = self.serial.checked_add(1).ok_or(())?;
            self.states.insert(key, State { model: model.identity.clone(), actor: request.actor_identity, skin: request.skin,
                world: request.model_panel.then(|| inputs.template.independent()), identity: 0x6000_0000_u32.checked_add(self.serial).ok_or(())?, event: 0, position });
        }
        let state = self.states.get_mut(&key).ok_or(())?;
        let cp = ControlPoint { index: 0, position, previous_position: state.position,
            orientation: quaternion(transform), velocity: [0.0; 3], radius: 0.0, density: 0.0, duration: 0.0, parent: None, object_identity: (!request.model_panel).then_some(request.actor_identity) };
        let command = if state.event == 0 { EventCommand::Create { effect_identity: state.identity, definition: "superrare_burning1".into(), seed: u64::from(state.identity), owner_identity: (!request.model_panel).then_some(request.actor_identity), control_points: vec![cp] } }
            else { EventCommand::SetControlPoint { effect_identity: state.identity, control_point: cp } };
        let from = state.world.as_ref().map_or_else(|| inputs.template.time(), ParticleWorld::time);
        let timestamp_seconds = if state.event == 0 { request.current_time } else { from };
        state.event += 1;
        state.position = position;
        if let Some(world) = state.world.as_mut() {
            let (events, controls) = match command {
                EventCommand::SetControlPoint { effect_identity, control_point } => (Vec::new(), vec![(effect_identity, control_point)]),
                command => (vec![Event { identity: state.event, timestamp_seconds, source_order: 0, command }], Vec::new()),
            };
            world.transact_render_output(&events, &controls,
                AdvanceRequest { from_seconds: from, to_seconds: request.current_time, maximum_step_seconds: 0.05, camera_position: lighting.camera },
                &mut NoQueries, inputs.materials, inputs.identities, 1024 * 1024).map_err(|_| ())
        } else { self.queue(timestamp_seconds, command); Ok(Vec::new()) }
    }
}

struct NoQueries;
fn scope(request: &super::ModelPoseRequest) -> u8 { if request.hud_model { 2 } else { u8::from(request.model_panel) } }
impl playsrc_particle::CollisionQuery for NoQueries {
    fn trace_batch(&mut self, requests: &[playsrc_particle::TraceRequest]) -> Result<Vec<playsrc_particle::CollisionResult>, playsrc_particle::Error> {
        if !requests.is_empty() { return Err(playsrc_particle::Error { code: playsrc_particle::ErrorCode::MissingQuery, source: "model-panel".into(), offset: 0, detail: "particle collision input is unavailable in this model panel".into() }); }
        Ok(Vec::new())
    }
}

fn entity_transform(origin: [f32; 3], angles: [f32; 3], local: [f32; 12]) -> [f32; 12] {
    let (sp, cp) = angles[0].to_radians().sin_cos();
    let (sy, cy) = angles[1].to_radians().sin_cos();
    let (sr, cr) = angles[2].to_radians().sin_cos();
    let m = [[cp*cy, sr*sp*cy-cr*sy, cr*sp*cy+sr*sy], [cp*sy, sr*sp*sy+cr*cy, cr*sp*sy-sr*cy], [-sp, sr*cp, cr*cp]];
    std::array::from_fn(|i| { let row=i/4; let col=i%4; m[row][0]*local[col]+m[row][1]*local[4+col]+m[row][2]*local[8+col]+if col==3 {origin[row]} else {0.0} })
}

fn quaternion(m: [f32; 12]) -> [f32; 4] {
    let mut q = [0.0; 4];
    let trace = m[0]+m[5]+m[10];
    if trace > 0.0 { let s=(trace+1.0).sqrt(); q[3]=s*0.5; let s=0.5/s; q[0]=(m[9]-m[6])*s; q[1]=(m[2]-m[8])*s; q[2]=(m[4]-m[1])*s; }
    else { let i=if m[0]>m[5] && m[0]>m[10] {0} else if m[5]>m[10] {1} else {2}; let j=(i+1)%3; let k=(j+1)%3; let s=(m[i*4+i]-m[j*4+j]-m[k*4+k]+1.0).sqrt(); q[i]=s*0.5; let s=0.5/s; q[j]=(m[j*4+i]+m[i*4+j])*s; q[k]=(m[k*4+i]+m[i*4+k])*s; q[3]=(m[k*4+j]-m[j*4+k])*s; }
    q
}

pub fn model(item: &EquippedItem, player_model: &str) -> Result<Option<&'static str>, ()> {
    if item.definition_index != 378 { return Ok(None); }
    if item.style != 0 { return Err(()); }
    match player_model {
        "models/player/soldier.mdl" => Ok(Some("models/player/items/soldier/soldier_officer.mdl")),
        "models/player/medic.mdl" => Ok(Some("models/player/items/medic/medic_officer.mdl")),
        "models/player/heavy.mdl" => Ok(Some("models/player/items/heavy/heavy_officer.mdl")),
        _ => Err(()),
    }
}

pub fn effect(item: &EquippedItem) -> Result<u32, ()> {
    match item.attributes.iter().find(|attribute| attribute.definition == 134) {
        None => Ok(0),
        Some(attribute) if attribute.value == 13.0 => Ok(13),
        _ => Err(()),
    }
}

pub fn owner_lighting_origin(player_model: &str, origin: [f32; 3]) -> Result<[f32; 3], ()> {
    let class = match player_model {
        "models/player/soldier.mdl" => playsrc_tf2::PlayerClass::Soldier,
        "models/player/medic.mdl" => playsrc_tf2::PlayerClass::Medic,
        "models/player/heavy.mdl" => playsrc_tf2::PlayerClass::Heavy,
        _ => return Err(()),
    };
    let hull = playsrc_tf2::MovementPolicy { class, modifiers: Default::default() }.resolve().standing_hull;
    Ok(std::array::from_fn(|axis| origin[axis] + (hull.mins[axis] + hull.maxs[axis]) * 0.5))
}

pub fn control_point(model: &PresentationModel, pose: &SampledPose, panel: bool) -> Result<Matrix3x4, ()> {
    if let Some(attachment) = pose.attachments.iter().find(|attachment| attachment.name.eq_ignore_ascii_case(b"muzzle")) {
        return Ok(attachment.model_transform);
    }
    // Model panels use the hat/head bone search; world wearables follow the root.
    let bone = if panel {
        [b"bip_head".as_slice(), b"prp_helmet", b"prp_hat"].iter()
            .find_map(|name| model.bones.iter().position(|bone| bone.name.eq_ignore_ascii_case(name)))
            .unwrap_or(0)
    } else { 0 };
    pose.model_matrices.get(bone).copied().ok_or(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use playsrc_tf2::{equipment::ItemAttribute, schema::LoadoutPosition};

    fn item() -> EquippedItem {
        EquippedItem { item_id: 379, definition_index: 378, quality: 5, style: 0,
            slot: LoadoutPosition::Misc, attributes: vec![ItemAttribute { definition: 134, value: 13.0 }] }
    }

    #[test]
    fn consumes_shared_equipment_without_registering_unsupported_cosmetics() {
        let mut equipped = item();
        for class in ["soldier", "medic", "heavy"] {
            assert_eq!(model(&equipped, &format!("models/player/{class}.mdl")).unwrap().unwrap(),
                format!("models/player/items/{class}/{class}_officer.mdl"));
        }
        assert!(model(&equipped, "models/player/spy.mdl").is_err());
        assert_eq!(effect(&equipped), Ok(13));
        equipped.attributes[0].value = 14.0;
        assert!(effect(&equipped).is_err());
        equipped.attributes.clear();
        assert_eq!(effect(&equipped), Ok(0));
        equipped.style = 1;
        assert!(model(&equipped, "models/player/soldier.mdl").is_err());
        equipped.definition_index = 18;
        assert_eq!(model(&equipped, "models/player/soldier.mdl"), Ok(None));
    }

    #[test]
    fn preview_release_and_absent_actors_retire_only_their_owned_effects() {
        let mut states = ParticleStates::default();
        let state = |identity| State { model: "hat".into(), actor: 2, skin: 0, world: None, identity, event: 1, position: [0.0; 3] };
        states.states.insert((1, 100, 379), state(1));
        states.states.insert((0, 200, 379), state(2));
        states.states.insert((2, 300, 379), state(3));
        states.retain(&[]);
        assert_eq!(states.states.len(), 2);
        assert!(states.pending.is_empty());
        states.retain_gameplay(None, 1.0);
        assert!(states.states.is_empty());
        assert_eq!(states.pending.len(), 1);
        assert_eq!(states.pending[0].command, EventCommand::Destroy { effect_identity: 2 });
    }
}
