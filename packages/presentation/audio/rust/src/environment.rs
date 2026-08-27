use crate::{
    acoustics::{Detector, Geometry},
    dsp::{Error, Preset, Presets},
    room::DEFAULT_TEMPLATES,
    soundscape::{Action, Activation, Listener, Random, Registry, Selection, Soundscape},
};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct PresetChange {
    pub identity: i32,
    pub definition: Preset,
}

#[derive(Clone, Debug, Default)]
pub struct Update {
    pub actions: Vec<Action>,
    pub room: Option<PresetChange>,
    pub player: Option<PresetChange>,
}

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub selection: Selection,
    pub listener: Listener,
    pub elapsed: f32,
    pub game_time: f32,
    pub host_time: f64,
    pub can_set_mixer: bool,
}

/// Immutable documents are shared across simulation transactions. Mutable state
/// is limited to active soundscape layers, a bounded room scan and forty nodes.
#[derive(Clone, Debug)]
pub struct Environment {
    pub registry: Arc<Registry>,
    pub presets: Arc<Presets>,
    soundscape: Soundscape,
    detector: Detector,
    automatic: [Option<Preset>; 40],
    room_request: i32,
    effective_room: i32,
    player_request: i32,
}

impl Environment {
    pub fn new(registry: Registry, presets: Presets) -> Result<Self, Error> {
        if presets.0.len() < 2 {
            return Err(Error::Malformed("missing room DSP definitions"));
        }
        Ok(Self {
            registry: Arc::new(registry),
            presets: Arc::new(presets),
            soundscape: Soundscape::default(),
            detector: Detector::default(),
            automatic: std::array::from_fn(|_| None),
            room_request: 0,
            effective_room: 0,
            player_request: 0,
        })
    }

    pub fn soundscape(&self) -> &Soundscape {
        &self.soundscape
    }
    pub fn effective_room(&self) -> i32 {
        self.effective_room
    }

    pub fn update(
        &mut self,
        frame: Frame,
        random: &mut impl Random,
        geometry: &mut impl Geometry,
    ) -> Result<Update, Error> {
        let Frame {
            selection,
            listener,
            elapsed,
            game_time,
            host_time,
            can_set_mixer,
        } = frame;
        let mut update = Update::default();
        self.soundscape.select(
            &self.registry,
            selection,
            Activation {
                time: game_time,
                restoring: false,
                can_set_mixer,
            },
            random,
            &mut update.actions,
        );
        self.soundscape
            .update(elapsed, game_time, listener, random, &mut update.actions);
        for action in &update.actions {
            match *action {
                Action::RoomDsp(value) => self.room_request = value,
                Action::PlayerDsp(value)
                    if value >= 0
                        && value < self.presets.0.len() as i32
                        && value != self.player_request =>
                {
                    let definition = self.presets.0[value as usize].clone();
                    // Reject before browser publication; never substitute an
                    // identity processor for a required but unimplemented one.
                    definition.validate_processing()?;
                    self.player_request = value;
                    update.player = Some(PresetChange {
                        identity: value,
                        definition,
                    });
                }
                _ => {}
            }
        }
        if let Some(change) =
            self.detector
                .update(self.room_request == 1, host_time, listener.origin, geometry)
        {
            if let Some(room) = change.created {
                let (_, definition) = self.presets.automatic(room, &DEFAULT_TEMPLATES)?;
                definition.validate_processing()?;
                self.automatic[change.node] = Some(definition);
            }
            let identity = change.node as i32 + 60;
            if identity != self.effective_room {
                let definition = self.automatic[change.node]
                    .clone()
                    .ok_or(Error::Malformed("unconstructed room node"))?;
                self.effective_room = identity;
                update.room = Some(PresetChange {
                    identity,
                    definition,
                });
            }
        } else if self.room_request != 1
            && self.room_request != self.effective_room
            && self.room_request >= 0
            && self.room_request < self.presets.0.len() as i32
        {
            let definition = self.presets.0[self.room_request as usize].clone();
            definition.validate_processing()?;
            self.effective_room = self.room_request;
            update.room = Some(PresetChange {
                identity: self.room_request,
                definition,
            });
        }
        Ok(update)
    }

    pub fn reset(&mut self) -> Vec<Action> {
        let mut actions = Vec::new();
        self.soundscape.reset(&mut actions);
        self.detector.reset();
        self.automatic.fill(None);
        self.room_request = 0;
        self.effective_room = 0;
        self.player_request = 0;
        actions
    }
}
