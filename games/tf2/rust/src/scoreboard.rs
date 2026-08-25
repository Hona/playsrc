use crate::{PLAYER_IDENTITY, PlayerClass, PlayerLifecycle, PlayerTeam, bot, ctf};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Counters {
    pub kills: u32,
    pub deaths: u32,
    pub captures: u32,
    pub damage: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Player {
    pub identity: u32,
    pub name: String,
    pub team: PlayerTeam,
    pub class: PlayerClass,
    pub alive: bool,
    pub fake: bool,
    pub score: i32,
    pub counters: Counters,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Snapshot {
    pub red_score: i32,
    pub blue_score: i32,
    pub red_count: u8,
    pub blue_count: u8,
    pub players: Vec<Player>,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    local: Counters,
}

impl State {
    pub fn local_damage(&mut self, amount: u32) {
        self.local.damage = self.local.damage.saturating_add(amount);
    }

    pub fn local_kill(&mut self) {
        self.local.kills = self.local.kills.saturating_add(1);
    }

    pub fn local_death(&mut self) {
        self.local.deaths = self.local.deaths.saturating_add(1);
    }

    pub fn local_capture(&mut self) {
        self.local.captures = self.local.captures.saturating_add(1);
    }

    pub fn snapshot(
        &self,
        team: PlayerTeam,
        class: PlayerClass,
        lifecycle: PlayerLifecycle,
        bots: &[bot::Snapshot],
        objectives: Option<ctf::Scores>,
        round_scores: (u16, u16),
    ) -> Snapshot {
        let mut players = Vec::with_capacity(bots.len() + 1);
        players.push(Player {
            identity: PLAYER_IDENTITY,
            name: "unnamed".to_owned(),
            team,
            class,
            alive: lifecycle == PlayerLifecycle::Active,
            fake: false,
            score: i32::try_from(
                self.local
                    .kills
                    .saturating_add(self.local.captures.saturating_mul(2))
                    .saturating_add(self.local.damage / 600),
            )
            .unwrap_or(i32::MAX),
            counters: self.local,
        });
        players.extend(bots.iter().map(|bot| {
            Player {
                identity: bot.identity,
                name: bot.name.clone(),
                team: bot.team,
                class: bot.class,
                alive: bot.lifecycle == PlayerLifecycle::Active,
                fake: true,
                score: i32::try_from(bot.kills.saturating_add(bot.captures.saturating_mul(2)))
                    .unwrap_or(i32::MAX),
                counters: Counters {
                    kills: bot.kills,
                    deaths: bot.deaths,
                    captures: bot.captures,
                    damage: bot.damage,
                },
            }
        }));
        let red_count = players
            .iter()
            .filter(|player| player.team == PlayerTeam::Red)
            .count() as u8;
        let blue_count = players
            .iter()
            .filter(|player| player.team == PlayerTeam::Blue)
            .count() as u8;
        let (red_score, blue_score) = objectives.map_or(
            (i32::from(round_scores.0), i32::from(round_scores.1)),
            |scores| {
                if scores.limit == 0 {
                    (i32::from(scores.red_score), i32::from(scores.blue_score))
                } else {
                    (
                        i32::from(scores.red_captures),
                        i32::from(scores.blue_captures),
                    )
                }
            },
        );
        Snapshot {
            red_score,
            blue_score,
            red_count,
            blue_count,
            players,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connected_local_player_moves_between_team_and_spectator_rosters() {
        let mut state = State::default();
        state.local_damage(125);
        state.local_kill();
        state.local_death();
        let red = state.snapshot(
            PlayerTeam::Red,
            PlayerClass::Soldier,
            PlayerLifecycle::Dying,
            &[],
            None,
            (0, 0),
        );
        assert_eq!((red.red_count, red.blue_count), (1, 0));
        assert_eq!(red.players[0].name, "unnamed");
        assert_eq!(red.players[0].score, 1);
        assert_eq!(
            red.players[0].counters,
            Counters {
                kills: 1,
                deaths: 1,
                captures: 0,
                damage: 125
            }
        );
        assert!(!red.players[0].alive);
        let spectator = state.snapshot(
            PlayerTeam::Spectator,
            PlayerClass::Soldier,
            PlayerLifecycle::Observer,
            &[],
            None,
            (0, 0),
        );
        assert_eq!((spectator.red_count, spectator.blue_count), (0, 0));
        assert_eq!(spectator.players[0].team, PlayerTeam::Spectator);
    }

    #[test]
    fn objective_owner_controls_team_scores_and_capture_points() {
        let mut state = State::default();
        state.local_capture();
        state.local_damage(600);
        let scores = ctf::Scores {
            red_captures: 2,
            blue_captures: 1,
            red_score: 7,
            blue_score: 4,
            limit: 3,
            winner: None,
        };
        let limited = state.snapshot(
            PlayerTeam::Red,
            PlayerClass::Soldier,
            PlayerLifecycle::Active,
            &[],
            Some(scores),
            (8, 6),
        );
        assert_eq!((limited.red_score, limited.blue_score), (2, 1));
        assert_eq!(limited.players[0].score, 3);
        assert_eq!(limited.players[0].counters.captures, 1);
        let unlimited = state.snapshot(
            PlayerTeam::Red,
            PlayerClass::Soldier,
            PlayerLifecycle::Active,
            &[],
            Some(ctf::Scores { limit: 0, ..scores }),
            (8, 6),
        );
        assert_eq!((unlimited.red_score, unlimited.blue_score), (7, 4));
        let round = state.snapshot(
            PlayerTeam::Red,
            PlayerClass::Soldier,
            PlayerLifecycle::Active,
            &[],
            None,
            (8, 6),
        );
        assert_eq!((round.red_score, round.blue_score), (8, 6));
    }
}
