//! `tf_logic_koth` configuration. Point ownership and victory belong to the
//! control point master; clock activation is driven by authored entity outputs.
use playsrc_entity::Graph;

use crate::round::{Error, TimerConfiguration, class, integer};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Configuration {
    pub identity: u32,
    pub timer_length: i32,
    pub unlock_point: i32,
    /// Source creates BLU first, then RED. These are runtime entity identities,
    /// not indices of control points.
    pub blue_timer: u32,
    pub red_timer: u32,
}

impl Configuration {
    pub fn from_graph(graph: &Graph) -> Result<Option<Self>, Error> {
        let Some(entity) = graph
            .entities
            .iter()
            .find(|entity| class(entity, b"tf_logic_koth"))
        else {
            return Ok(None);
        };
        let blue_timer = u32::try_from(graph.entities.len()).map_err(|_| Error::InvalidTimer)?;
        Ok(Some(Self {
            identity: u32::try_from(entity.index).map_err(|_| Error::InvalidTimer)?,
            timer_length: integer(entity, b"timer_length", 180)?,
            unlock_point: integer(entity, b"unlock_point", 30)?,
            blue_timer,
            red_timer: blue_timer.checked_add(1).ok_or(Error::InvalidTimer)?,
        }))
    }

    pub fn timers(self) -> [TimerConfiguration; 2] {
        [self.blue_timer, self.red_timer].map(|identity| TimerConfiguration {
            identity,
            initial_seconds: self.timer_length,
            setup_seconds: 0,
            maximum_seconds: 0,
            show_in_hud: true,
            auto_countdown: true,
            start_paused: true,
            reset_on_round_start: true,
        })
    }

    pub fn round_activate(
        self,
        points: &mut crate::control_point::World,
        now: f32,
        facts: crate::control_point::Facts,
        events: &mut Vec<crate::control_point::Event>,
    ) {
        let identities: Vec<_> = points.points().iter().map(|point| point.identity).collect();
        for identity in identities {
            let value = playsrc_entity::Variant::Integer(self.unlock_point);
            points.apply_input(identity, b"SetLocked", &value, now, facts, events);
            if self.unlock_point > 0 {
                points.apply_input(identity, b"SetUnlockTime", &value, now, facts, events);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlayerClass, PlayerTeam, control_point, round};
    use playsrc_collision::Hull;
    use playsrc_movement::{Error as MoveError, Trace, Tracer};

    #[derive(Clone)]
    struct CaptureBrush;
    impl Tracer for CaptureBrush {
        fn trace(
            &self,
            _start: [f32; 3],
            end: [f32; 3],
            _hull: Hull,
            _mask: u32,
        ) -> Result<Trace, MoveError> {
            Ok(Trace {
                fraction: 1.0,
                start_solid: false,
                all_solid: false,
                end,
                normal: None,
                hit: None,
                contents: 0,
            })
        }
    }
    impl crate::GameplayWorld for CaptureBrush {
        fn overlaps_model_hull(
            &self,
            _model: usize,
            _origin: [f32; 3],
            position: [f32; 3],
            _hull: Hull,
        ) -> Result<bool, MoveError> {
            Ok(position[0].abs() < 64.0)
        }
    }

    struct Match {
        map: crate::MapRuntime,
        rules: round::Rules,
        tick: u64,
    }
    impl Match {
        fn new() -> Self {
            let graph = playsrc_entity::parse(br#"
                {"classname" "tf_logic_koth" "targetname" "koth" "unlock_point" "1"}
                {"classname" "tf_gamerules" "targetname" "rules"}
                {"classname" "team_control_point_master"}
                {"classname" "team_control_point" "targetname" "point" "point_index" "0" "point_default_owner" "0"}
                {"classname" "trigger_capture_area" "model" "*1" "area_cap_point" "point" "area_time_to_cap" "1" "team_cancap_2" "1" "team_cancap_3" "1"
                 "OnCapTeam1" "rules,SetRedKothClockActive,,0,-1" "OnCapTeam2" "rules,SetBlueKothClockActive,,0,-1"}
            "#, Default::default()).unwrap();
            let mut map = crate::MapRuntime::compile(
                &graph,
                0.015,
                1,
                vec![playsrc_entity::ModelBounds {
                    model: 1,
                    mins: [-64.0; 3],
                    maxs: [64.0; 3],
                }],
            )
            .unwrap();
            let rules = round::Rules::active(map.round_configuration()).unwrap();
            let facts = control_point::Facts {
                points_may_be_captured: true,
                round_running: true,
                timer_may_expire: true,
                koth_timer_remaining: Some([180.0; 2]),
                ..Default::default()
            };
            rules.koth_configuration().unwrap().round_activate(
                map.control_points_mut().unwrap(),
                0.0,
                facts,
                &mut vec![],
            );
            Self {
                map,
                rules,
                tick: 0,
            }
        }

        fn step(&mut self, actors: &[control_point::Actor]) -> Vec<control_point::Event> {
            self.tick += 1;
            let now = self.tick as f32 * 0.015;
            self.map.apply_round_inputs(&mut self.rules, now);
            self.rules
                .advance(
                    now,
                    0.015,
                    round::Facts {
                        red_players: 1,
                        blue_players: 1,
                        red_alive: 1,
                        blue_alive: 1,
                        objective_contested: self.map.control_points().unwrap().contested(),
                        flag_away_from_home: false,
                    },
                )
                .unwrap();
            let snapshot = self.rules.snapshot(vec![]);
            let facts = control_point::Facts {
                points_may_be_captured: snapshot.state == round::State::Running,
                round_running: snapshot.state == round::State::Running,
                in_overtime: snapshot.in_overtime,
                waiting_for_players: false,
                koth_timer_remaining: snapshot
                    .koth_timers
                    .map(|timers| timers.map(|timer| timer.remaining)),
                timer_may_expire: self.rules.timer_may_expire(),
            };
            self.map.set_control_point_facts(facts);
            let mut events = vec![];
            self.map
                .control_points_mut()
                .unwrap()
                .step(now, facts, actors, &CaptureBrush, &mut events)
                .unwrap();
            self.map
                .emit_control_point_outputs(self.tick, &events)
                .unwrap();
            self.map.apply_round_inputs(&mut self.rules, now);
            for event in &events {
                if let control_point::Event::RoundWon { team, reason, .. } = event {
                    self.rules.win(*team, *reason).unwrap();
                }
            }
            events
        }

        fn input(&mut self, entity: u32, name: &[u8], value: i32) {
            self.map
                .input(
                    self.tick,
                    entity,
                    name,
                    playsrc_entity::Variant::Integer(value),
                )
                .unwrap();
            self.map
                .apply_round_inputs(&mut self.rules, self.tick as f32 * 0.015);
        }
    }

    fn actor(team: PlayerTeam) -> control_point::Actor {
        control_point::Actor::active(
            team.source_number() as u32,
            team,
            PlayerClass::Soldier,
            [0.0; 3],
            Hull {
                mins: [-24.0, -24.0, 0.0],
                maxs: [24.0, 24.0, 82.0],
            },
        )
    }

    #[test]
    fn brush_capture_io_drives_both_clocks_contest_overtime_retake_and_master_victory() {
        let mut game = Match::new();
        let red = actor(PlayerTeam::Red);
        let blue = actor(PlayerTeam::Blue);
        for _ in 0..60 {
            game.step(&[red]);
        }
        assert!(game.map.control_points().unwrap().points()[0].locked);
        assert_eq!(
            game.rules.koth_timers().unwrap().map(|t| t.remaining),
            [180.0; 2]
        );
        for _ in 0..250 {
            game.step(&[red]);
        }
        assert_eq!(
            game.map.control_points().unwrap().points()[0].owner,
            PlayerTeam::Red
        );
        assert!(!game.rules.koth_timers().unwrap()[0].paused);
        assert_eq!(game.rules.koth_timers().unwrap()[1].remaining, 180.0);
        game.input(0, b"SetRedTimer", 1);
        for _ in 0..20 {
            game.step(&[blue]);
        }
        for _ in 0..75 {
            game.step(&[red, blue]);
        }
        assert!(game.map.control_points().unwrap().areas()[0].blocked);
        assert!(game.rules.snapshot(vec![]).in_overtime);
        assert_eq!(game.rules.state(), round::State::Running);
        for _ in 0..200 {
            game.step(&[blue]);
        }
        assert_eq!(
            game.map.control_points().unwrap().points()[0].owner,
            PlayerTeam::Blue
        );
        let [red_timer, blue_timer] = game.rules.koth_timers().unwrap();
        assert_eq!(red_timer.remaining, 0.0);
        assert!(red_timer.paused && !blue_timer.paused);
        assert!(!game.rules.snapshot(vec![]).in_overtime);
        game.input(0, b"SetBlueTimer", 1);
        let mut won = false;
        for _ in 0..90 {
            won |= game.step(&[blue]).iter().any(|e| {
                matches!(
                    e,
                    control_point::Event::RoundWon {
                        team: PlayerTeam::Blue,
                        reason: 1,
                        ..
                    }
                )
            });
        }
        assert!(won);
        assert_eq!(game.rules.state(), round::State::TeamWin);
        assert_eq!(game.rules.snapshot(vec![]).blue_score, 1);
    }

    #[test]
    fn generated_timer_entities_accept_named_entity_io_and_disabled_timer_inputs_are_ignored() {
        let mut game = Match::new();
        let configuration = game.rules.koth_configuration().unwrap();
        assert!(game.map.source_handle(configuration.blue_timer).is_some());
        assert!(game.map.source_handle(configuration.red_timer).is_some());
        game.input(configuration.blue_timer, b"Disable", 0);
        game.input(0, b"SetBlueTimer", 5);
        assert_eq!(game.rules.koth_timers().unwrap()[1].remaining, 180.0);
        game.input(configuration.blue_timer, b"Enable", 0);
        game.input(0, b"SetBlueTimer", 5);
        assert_eq!(game.rules.koth_timers().unwrap()[1].remaining, 5.0);
    }

    #[test]
    fn local_session_announcer_uses_authored_overtime_wave_cycle_and_replayable_state() {
        let fixture = Match::new();
        let mut session = crate::Session::new(CaptureBrush, [0.0; 3], fixture.map);
        let initial = session.sound_selection.state();
        for _ in 0..4 {
            session.emit_objective_sound(
                crate::PLAYER_IDENTITY,
                crate::SoundDefinition::Overtime,
                [0.0; 3],
            );
        }
        assert_eq!(session.audio_events.len(), 4);
        assert_eq!(session.sound_selection.state().overtime_available, 0);
        assert!(session.sound_selection.restore(initial));
        assert_eq!(session.sound_selection.state().overtime_available, 15);
    }

    #[test]
    fn real_session_round_respawn_restores_dead_local_player_before_next_round_movement() {
        let fixture = Match::new();
        let mut session = crate::Session::new(CaptureBrush, [0.0; 3], fixture.map);
        session.health = 0;
        session.lifecycle = crate::PlayerLifecycle::Dying;
        session.ammo.primary = 0;
        session.movement.position = [1000.0; 3];
        session.round.win(PlayerTeam::Red, 1).unwrap();
        let mut respawn = None;
        for _ in 0..1003 {
            let snapshot = session.advance(crate::Command::default()).unwrap();
            if snapshot
                .events
                .iter()
                .any(|event| matches!(event, crate::Event::Respawned))
            {
                respawn = Some(snapshot);
                break;
            }
        }
        let snapshot = respawn.expect("round transition force-respawns the local player");
        assert_eq!(session.lifecycle, crate::PlayerLifecycle::Active);
        assert_eq!(snapshot.health, 200.0);
        assert_eq!(snapshot.movement.position[0], 0.0);
        assert_eq!(snapshot.round.state, round::State::Preround);
        assert!(snapshot.control_points.unwrap().points[0].locked);
        assert_eq!(
            snapshot
                .round
                .koth_timers
                .unwrap()
                .map(|timer| (timer.remaining, timer.paused)),
            [(180.0, true); 2]
        );
    }

    #[test]
    fn named_delayed_input_routes_through_real_entity_scheduler_without_changing_clock_rate() {
        let mut game = Match::new();
        game.map
            .fire_input(0, b"koth", b"SetRedTimer", b"5", 0.15)
            .unwrap();
        for tick in 1..=11 {
            game.map
                .begin_tick(
                    &CaptureBrush,
                    crate::map_runtime::BeginTickInput {
                        tick,
                        tick_interval: 0.015,
                        activate_entity: None,
                        player_position: [0.0; 3],
                        player_hull: actor(PlayerTeam::Red).hull,
                        grounded: true,
                    },
                )
                .unwrap();
            game.map
                .apply_round_inputs(&mut game.rules, tick as f32 * 0.015);
            if tick < 11 {
                assert_eq!(game.rules.koth_timers().unwrap()[0].remaining, 180.0);
            }
        }
        assert_eq!(game.rules.koth_timers().unwrap()[0].remaining, 5.0);
    }

    #[test]
    fn logic_defaults_and_generated_timer_order_match_sdk() {
        let graph =
            playsrc_entity::parse(b"{\"classname\"\"tf_logic_koth\"}\0", Default::default())
                .unwrap();
        let configuration = Configuration::from_graph(&graph).unwrap().unwrap();
        assert_eq!(configuration.timer_length, 180);
        assert_eq!(configuration.unlock_point, 30);
        assert_eq!(configuration.timers().map(|timer| timer.identity), [1, 2]);
        assert!(
            configuration
                .timers()
                .iter()
                .all(|timer| timer.start_paused && timer.show_in_hud)
        );
    }
}
