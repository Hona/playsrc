use crate::{Session, GameplayWorld, Weapon, PLAYER_IDENTITY, SoundDefinition, SoundQueryPhase, RandomContext,
    AudioAction, AudioEvent, AudioEventIdentity, AudioSourceKind, weapon::{MinigunState, WeaponSource}, audio::WeaponSoundSlot};

#[derive(Clone, Copy, Debug)]
struct Patch { slot: WeaponSoundSlot, source: WeaponSource, event: AudioEvent }

#[derive(Clone, Default)]
pub(crate) struct State { patch: Option<Patch> }

impl<W: GameplayWorld + Clone> Session<W> {
    pub(crate) fn stop_minigun_audio(&mut self) {
        if let Some(patch) = self.minigun_audio.patch.take() {
            self.push_audio_event(AudioEvent { action: AudioAction::Stop, tick: self.tick, ordinal: 0, ..patch.event });
        }
    }

    pub(crate) fn update_minigun_audio(&mut self, previous: MinigunState) {
        let Some(source) = self.weapon_source(PLAYER_IDENTITY, Weapon::Minigun) else { self.stop_minigun_audio(); return; };
        if self.minigun_audio.patch.is_some_and(|patch| patch.source != source) { self.stop_minigun_audio(); }
        let runtime = self.loadout[&Weapon::Minigun];
        let spin_sounds = self.equipped_weapon_attribute(PLAYER_IDENTITY, Weapon::Minigun, "minigun_no_spin_sounds", 0.0).round_ties_even() == 0.0;
        let current = self.minigun_audio.patch.map(|patch| patch.slot);
        let desired = match runtime.minigun_state {
            MinigunState::Starting => Some((WeaponSoundSlot::Special1, SoundDefinition::MinigunWindUp)),
            MinigunState::Firing if runtime.critical.result.is_some_and(|result| result.kind == crate::damage::CritKind::Full) => Some((WeaponSoundSlot::Burst, SoundDefinition::MinigunCritical)),
            MinigunState::Firing => Some((WeaponSoundSlot::Double, SoundDefinition::MinigunFire)),
            MinigunState::DryFire => Some((WeaponSoundSlot::Empty, SoundDefinition::MinigunEmpty)),
            MinigunState::Spinning if spin_sounds => Some((WeaponSoundSlot::Special3, SoundDefinition::MinigunSpin)),
            MinigunState::Spinning if previous == MinigunState::Firing => Some((WeaponSoundSlot::Special2, SoundDefinition::MinigunWindDown)),
            MinigunState::Spinning => return,
            MinigunState::Idle if !spin_sounds && (previous == MinigunState::Firing || current == Some(WeaponSoundSlot::Special2)) => Some((WeaponSoundSlot::Special2, SoundDefinition::MinigunWindDown)),
            MinigunState::Idle if spin_sounds && (previous != MinigunState::Idle || current == Some(WeaponSoundSlot::Special2)) => Some((WeaponSoundSlot::Special2, SoundDefinition::MinigunWindDown)),
            MinigunState::Idle => None,
        };
        let Some((slot, class_sound)) = desired else { self.stop_minigun_audio(); return; };
        if current == Some(slot) { return; }
        self.stop_minigun_audio();
        let definition = class_sound.equipment_slot(source.definition_index, slot);
        // SoundCreate resolves one client-side script query, then retains that
        // selected WAV for the sound envelope rather than re-sampling rndwave.
        let samples = self.sample_sound(RandomContext::PredictedPresentation, definition, SoundQueryPhase::Inspect);
        let delay = runtime.profile().fire_delay / crate::weapon::WeaponProfile::configured(Weapon::Minigun).fire_delay;
        let pitch = if delay == 1.0 { 100.0 } else { 80.0 + ((1.5 - delay) / 1.0).clamp(0.0, 1.0) * 40.0 };
        let event = AudioEvent { action: AudioAction::PlayAtPitch(pitch), tick: self.tick, ordinal: 0,
            identity: AudioEventIdentity::WeaponSingle, definition, source_kind: AudioSourceKind::Entity,
            source_identity: PLAYER_IDENTITY, owner_identity: Some(PLAYER_IDENTITY), position: self.movement.position, samples };
        self.minigun_audio.patch = Some(Patch { slot, source, event });
        self.push_audio_event(event);
    }
}
