//! team_control_point_round selection and round-associated spawn points.
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct Round {
    pub identity: u32,
    pub name: String,
    pub priority: i32,
    pub restricted_winner: u8,
    pub points: Vec<usize>,
    pub disabled: bool,
    pub(super) initial_disabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> World {
        let mut text = String::from(r#"{"classname" "team_control_point_master" "switch_teams" "1" "score_style" "1"}"#);
        for stage in 0..3 {
            let a = stage * 2;
            let b = a + 1;
            text += &format!(r#"{{"classname" "team_control_point_round" "targetname" "r{stage}" "cpr_priority" "{}" "cpr_restrict_team_cap_win" "2" "cpr_cp_names" "p{a} p{b}"}}
                {{"classname" "info_player_teamspawn" "TeamNum" "3" "round_bluespawn" "r{stage}"}}
                {{"classname" "info_player_teamspawn" "TeamNum" "2" "round_redspawn" "r{stage}"}}"#, 100 - stage * 25);
            for point in [a, b] {
                text += &format!(r#"{{"classname" "team_control_point" "targetname" "p{point}" "point_index" "{point}" "point_default_owner" "2" "team_previouspoint_3_0" "p{a}"}}
                    {{"classname" "trigger_capture_area" "model" "*{}" "area_cap_point" "p{point}" "area_time_to_cap" "3" "team_cancap_3" "1" "team_cancap_2" "0"}}"#, point + 1);
            }
        }
        World::from_graph(&playsrc_entity::parse(text.as_bytes(), Default::default()).unwrap()).unwrap().unwrap()
    }

    fn think(world: &mut World, point: usize, now: f32, actors: &[Actor], start: bool) -> Vec<Event> {
        let index = world.areas.iter().position(|a| a.point == point).unwrap();
        world.areas[index].touching = actors.iter().map(|a| a.identity).collect();
        let facts = Facts { points_may_be_captured: true, round_running: true, ..Default::default() };
        world.facts = facts;
        let mut events = Vec::new();
        world.think_area(index, now, facts, actors, start, &mut events);
        events
    }

    #[test]
    fn three_stage_live_capture_cancel_rollback_win_and_full_reset() {
        let mut world = fixture();
        let mut random = crate::UniformRandomStream::from_seed(123).unwrap();
        let blue = Actor::active(1, PlayerTeam::Blue, PlayerClass::Soldier, [0.0; 3], Hull { mins: [0.0; 3], maxs: [0.0; 3] });
        let red = Actor { identity: 2, team: PlayerTeam::Red, ..blue };
        let mut time = 1.0;
        for stage in 0..3 {
            let mut events = Vec::new();
            world.select_round(None, &mut random, &mut events);
            assert_eq!(world.current_round(), Some(stage));
            assert_eq!(world.spawns.iter().filter(|s| !s.disabled).count(), 2);
            assert_eq!(world.bot_capture_points(PlayerTeam::Blue).map(|p| p.index).collect::<Vec<_>>(), [stage * 2]);
            assert_eq!(world.bot_defend_points(PlayerTeam::Red).map(|p| p.index).collect::<Vec<_>>(), [stage * 2]);
            let a = stage * 2;
            let b = a + 1;
            let outside = (a + 2) % 6;
            assert!(think(&mut world, outside, time, &[blue], true).is_empty());
            assert!(think(&mut world, b, time, &[blue], true).is_empty());
            think(&mut world, a, time, &[blue], true);
            time += 1.0;
            think(&mut world, a, time, &[blue], false);
            let progress = world.areas[a].remaining;
            time += 0.1;
            think(&mut world, a, time, &[blue, red], false);
            assert_eq!(world.areas[a].remaining, progress, "defender blocks current stage");
            time += 100.0;
            assert!(think(&mut world, a, time, &[], false).iter().any(|e| matches!(e, Event::CaptureBroken { .. })));
            time += 0.1;
            think(&mut world, a, time, &[blue], true);
            time += 6.1;
            think(&mut world, a, time, &[blue], false);
            assert_eq!(world.points[a].owner, PlayerTeam::Blue);
            think(&mut world, b, time, &[blue], true);
            world.change_owner(a, PlayerTeam::Red, false, &mut Vec::new());
            time += 0.1;
            assert!(think(&mut world, b, time, &[blue], false).iter().any(|e| matches!(e, Event::CaptureBroken { .. })));
            world.change_owner(a, PlayerTeam::Blue, false, &mut Vec::new());
            think(&mut world, b, time, &[blue], true);
            time += 6.1;
            let captured = think(&mut world, b, time, &[blue], false);
            assert!(captured.iter().any(|e| matches!(e, Event::RoundWon { team: PlayerTeam::Blue, full_reset, switch_teams, .. } if *full_reset == (stage == 2) && *switch_teams == (stage == 2))));
            let capture = captured.iter().position(|e| matches!(e, Event::Captured { .. })).unwrap();
            let won = captured.iter().position(|e| matches!(e, Event::RoundWon { .. })).unwrap();
            assert!(capture < won);
            world.end_round(&mut events);
            assert!(events.iter().any(|e| matches!(e, Event::MapOutput { output: "OnEnd", .. })));
            world.round_spawn(time, stage == 2, &mut events);
        }
        assert!(world.points.iter().all(|p| p.owner == PlayerTeam::Red));
        world.select_round(None, &mut random, &mut Vec::new());
        assert_eq!(world.current_round(), Some(0));
    }

    #[test]
    fn selection_uses_priority_restricted_winner_previous_rounds_and_source_random() {
        let mut world = fixture();
        let mut random = crate::UniformRandomStream::from_seed(19).unwrap();
        // Disabled is an entity input state, not an IsPlayable gate in the SDK.
        world.rounds[0].disabled = true;
        world.select_round(None, &mut random, &mut Vec::new());
        assert_eq!(world.current_round(), Some(0));
        world.end_round(&mut Vec::new());
        for round in &mut world.rounds { round.priority = 100; }
        world.select_round(None, &mut random, &mut Vec::new());
        let second = world.current_round().unwrap();
        assert_ne!(second, 0);
        world.end_round(&mut Vec::new());
        world.select_round(None, &mut random, &mut Vec::new());
        assert_ne!(world.current_round(), Some(0));
        assert_ne!(world.current_round(), Some(second));
        world.end_round(&mut Vec::new());
        world.select_round(None, &mut random, &mut Vec::new());
        assert_eq!(world.current_round(), Some(0), "only the two most recent rounds are excluded");
        assert_eq!(world.previous_rounds.len(), 2);
        world.preserve_waiting_round();
        world.end_round(&mut Vec::new());
        world.round_spawn(1.0, true, &mut Vec::new());
        let before = random.state();
        world.select_round(None, &mut random, &mut Vec::new());
        assert_eq!(world.current_round(), Some(0));
        assert_eq!(random.state(), before, "waiting restart uses the selected round before random choice");
        world.end_round(&mut Vec::new());
        world.select_round(Some("r0"), &mut random, &mut Vec::new());
        assert_eq!(world.current_round(), Some(0), "explicit round precedes random exclusions");
    }
}

pub(super) fn compile(graph: &Graph, points: &[Point]) -> Vec<Round> {
    let mut rounds: Vec<_> = graph.entities.iter().rev().filter(|e| class(e, b"team_control_point_round")).map(|e| {
        let names = text(e, b"cpr_cp_names").to_ascii_lowercase();
        let mut selected: Vec<_> = points.iter().filter(|p| {
            // FindControlPoints uses the first case-insensitive substring and
            // checks only its trailing delimiter, not a tokenized name list.
            names.find(&p.name.to_ascii_lowercase()).is_some_and(|i| {
                names.as_bytes().get(i + p.name.len()).is_none_or(|b| *b == b' ')
            })
        }).collect();
        selected.sort_by_key(|p| std::cmp::Reverse(p.identity));
        let disabled = integer(e, b"StartDisabled", 0) != 0;
        Round { identity: e.index as u32, name: text(e, b"targetname"), priority: integer(e, b"cpr_priority", 0),
            restricted_winner: integer(e, b"cpr_restrict_team_cap_win", 0) as u8,
            points: selected.into_iter().map(|p| p.index).collect(), disabled, initial_disabled: disabled }
    }).collect();
    rounds.sort_by_key(|r| std::cmp::Reverse(r.priority));
    rounds
}

impl World {
    pub fn rounds(&self) -> &[Round] { &self.rounds }
    pub fn current_round(&self) -> Option<usize> { self.current_round }
    pub fn preserve_waiting_round(&mut self) { self.round_after_waiting = self.current_round; }
    pub fn in_round(&self, point: usize) -> bool {
        self.current_round.is_none_or(|i| self.rounds[i].points.contains(&point))
    }

    pub(super) fn group_winner(&self, points: impl Iterator<Item = usize>, override_owner: Option<(usize, PlayerTeam)>) -> PlayerTeam {
        let mut groups = [None; 8];
        for index in points {
            let point = &self.points[index];
            let owner = override_owner.filter(|(i, _)| *i == index).map_or(point.owner, |(_, t)| t);
            groups[point.group] = Some(match groups[point.group] {
                None => owner,
                Some(previous) if previous == owner => owner,
                _ => PlayerTeam::Unassigned,
            });
        }
        groups.into_iter().flatten().find(|t| t.is_gameplay()).unwrap_or(PlayerTeam::Unassigned)
    }

    pub fn round_playable(&self, index: usize) -> bool {
        let round = &self.rounds[index];
        let owner = self.group_winner(round.points.iter().copied(), None);
        // IsPlayable does not consult StartDisabled.
        round.restricted_winner == 1 || !owner.is_gameplay() || slot(owner) == round.restricted_winner as usize
    }

    pub fn end_round(&mut self, events: &mut Vec<Event>) {
        if let Some(index) = self.current_round.take() {
            output(events, self.rounds[index].identity, "OnEnd", Variant::Void);
        }
    }

    pub fn select_round(&mut self, specific: Option<&str>, random: &mut crate::UniformRandomStream, events: &mut Vec<Event>) {
        if self.rounds.is_empty() || self.current_round.is_some() { return; }
        let specific = specific.and_then(|name| self.rounds.iter().position(|r| r.name.eq_ignore_ascii_case(name))).or(self.round_after_waiting.take());
        let selected = specific.filter(|i| self.round_playable(*i) || self.make_round_playable(*i, events));
        let selected = selected.or_else(|| {
            let priority = self.rounds.iter().enumerate().find(|(i, _)| self.round_playable(*i))?.1.priority;
            let mut candidates: Vec<_> = (0..self.rounds.len()).rev().filter(|i| self.rounds[*i].priority == priority && self.round_playable(*i)).collect();
            for excluded in [self.previous_rounds.first().copied(), if self.first_after_restart { self.first_round } else { None }].into_iter().flatten() {
                if candidates.len() > 1 { candidates.retain(|i| *i != excluded); }
            }
            loop {
                let index = random.random_int(0, candidates.len() as i32 - 1).expect("bounded round index") as usize;
                if candidates.len() == 1 || !self.previous_rounds.contains(&candidates[index]) { break Some(candidates[index]); }
                candidates.remove(index);
            }
        });
        let Some(index) = selected else { return; };
        self.current_round = Some(index);
        self.setup_round_spawns();
        output(events, self.rounds[index].identity, "OnStart", Variant::Void);
        self.previous_rounds.insert(0, index);
        self.previous_rounds.truncate(2);
        if self.first_after_restart { self.first_round = Some(index); self.first_after_restart = false; }
    }

    fn make_round_playable(&mut self, index: usize, events: &mut Vec<Event>) -> bool {
        for team in TEAMS {
            for point in self.rounds[index].points.clone() {
                if self.master.base_points.contains(&Some(point)) || self.group_winner(self.rounds[index].points.iter().copied(), Some((point, team))) == team { continue; }
                if self.areas.iter().any(|a| a.point == point && a.teams[slot(team)].can_cap) {
                    self.change_owner(point, team, false, events);
                    return true;
                }
            }
        }
        false
    }

    fn setup_round_spawns(&mut self) {
        let Some(index) = self.current_round else { return; };
        self.spawn_revision += 1;
        for spawn in &mut self.spawns {
            let team = if let Some(point) = spawn.point.filter(|p| self.rounds[index].points.contains(p)) {
                Some(self.points[point].owner)
            } else if spawn.round_blue == Some(index) { Some(PlayerTeam::Blue) }
            else if spawn.round_red == Some(index) { Some(PlayerTeam::Red) }
            else { None };
            spawn.disabled = team.is_none();
            if let Some(team) = team { spawn.team = team; }
        }
    }
}
