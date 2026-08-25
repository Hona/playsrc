use crate::class::PlayerTeam;

pub const MAX_ROSTER_PLAYERS: usize = 64;
pub const HIGHLANDER_TEAM_SIZE: usize = 9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamRules {
    pub allows_spectators: bool,
    pub spectators_restricted: bool,
    pub unbalance_limit: u8,
    pub highlander: bool,
    pub mann_vs_machine: bool,
    pub defenders_team_size: u8,
    pub competitive: bool,
    pub attack_defend: bool,
    pub allow_team_change: bool,
    pub game_over: bool,
    pub training: bool,
}

impl Default for TeamRules {
    fn default() -> Self {
        Self {
            allows_spectators: true,
            spectators_restricted: false,
            unbalance_limit: 1,
            highlander: false,
            mann_vs_machine: false,
            defenders_team_size: 6,
            competitive: false,
            attack_defend: false,
            allow_team_change: true,
            game_over: false,
            training: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RosterPlayer {
    pub identity: u32,
    pub team: PlayerTeam,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeamSnapshot {
    pub local_identity: u32,
    pub local_team: PlayerTeam,
    pub red_count: usize,
    pub blue_count: usize,
    pub red_disabled: bool,
    pub blue_disabled: bool,
    pub spectators_visible: bool,
    pub auto_assign_visible: bool,
    pub cancel_visible: bool,
    pub highlander: bool,
    pub teams_full: bool,
    pub teams_full_arrow: bool,
    pub rules: TeamRules,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeamChoice {
    Red,
    Blue,
    Spectator,
    Auto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TeamSelectionError {
    DuplicatePlayer,
    MissingLocalPlayer,
    RosterLimit,
    InvalidIdentity,
    TeamChangeDisallowed,
    SpectatorsDisallowed,
    SpectatorUnbalancesTeams,
    TeamDisabled,
    NoAvailableTeam,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeamSelection {
    local_identity: u32,
    roster: Vec<RosterPlayer>,
    rules: TeamRules,
}

impl TeamSelection {
    pub fn new(local_identity: u32, rules: TeamRules) -> Result<Self, TeamSelectionError> {
        if local_identity == 0 {
            return Err(TeamSelectionError::InvalidIdentity);
        }
        Ok(Self {
            local_identity,
            roster: vec![RosterPlayer {
                identity: local_identity,
                team: PlayerTeam::Unassigned,
            }],
            rules,
        })
    }

    pub fn replace_roster(&mut self, roster: Vec<RosterPlayer>) -> Result<(), TeamSelectionError> {
        if roster.len() > MAX_ROSTER_PLAYERS {
            return Err(TeamSelectionError::RosterLimit);
        }
        let mut identities = std::collections::BTreeSet::new();
        for player in &roster {
            if player.identity == 0 {
                return Err(TeamSelectionError::InvalidIdentity);
            }
            if !identities.insert(player.identity) {
                return Err(TeamSelectionError::DuplicatePlayer);
            }
        }
        if !identities.contains(&self.local_identity) {
            return Err(TeamSelectionError::MissingLocalPlayer);
        }
        self.roster = roster;
        Ok(())
    }

    pub fn replace_rules(&mut self, rules: TeamRules) {
        self.rules = rules;
    }

    fn counts(&self) -> (usize, usize) {
        self.roster.iter().fold((0, 0), |(red, blue), player| {
            (
                red + usize::from(player.team == PlayerTeam::Red),
                blue + usize::from(player.team == PlayerTeam::Blue),
            )
        })
    }

    fn local(&self) -> &RosterPlayer {
        self.roster
            .iter()
            .find(|player| player.identity == self.local_identity)
            .expect("validated local roster identity")
    }

    fn would_unbalance(&self, team: PlayerTeam, current: PlayerTeam) -> bool {
        if team == current
            || !team.is_gameplay()
            || self.rules.unbalance_limit == 0
            || self.rules.competitive
        {
            return false;
        }
        let (red, blue) = self.counts();
        let destination = if team == PlayerTeam::Red {
            red + 1
        } else {
            blue + 1
        };
        let opposite = if team == PlayerTeam::Red {
            blue.saturating_sub(usize::from(current == PlayerTeam::Blue))
        } else {
            red.saturating_sub(usize::from(current == PlayerTeam::Red))
        };
        destination.saturating_sub(opposite) > usize::from(self.rules.unbalance_limit)
    }

    pub fn snapshot(&self) -> TeamSnapshot {
        let local = self.local();
        let (red_count, blue_count) = self.counts();
        let unbalanced = self.rules.unbalance_limit > 0
            && !self.rules.competitive
            && red_count.abs_diff(blue_count) > usize::from(self.rules.unbalance_limit);
        let red_disabled = (unbalanced && red_count > blue_count)
            || self.would_unbalance(PlayerTeam::Red, local.team)
            || (self.rules.highlander && red_count >= HIGHLANDER_TEAM_SIZE)
            || (self.rules.mann_vs_machine
                && red_count >= usize::from(self.rules.defenders_team_size));
        let blue_disabled = (unbalanced && blue_count > red_count)
            || self.would_unbalance(PlayerTeam::Blue, local.team)
            || (self.rules.highlander && blue_count >= HIGHLANDER_TEAM_SIZE)
            || self.rules.mann_vs_machine;
        let teams_full = red_disabled && blue_disabled;
        TeamSnapshot {
            local_identity: self.local_identity,
            local_team: local.team,
            red_count,
            blue_count,
            red_disabled,
            blue_disabled,
            spectators_visible: self.rules.allows_spectators,
            auto_assign_visible: self.rules.allows_spectators
                || !self.rules.highlander
                || !teams_full,
            cancel_visible: local.team != PlayerTeam::Unassigned,
            highlander: self.rules.highlander,
            teams_full: self.rules.highlander && teams_full,
            teams_full_arrow: self.rules.highlander && teams_full && self.rules.allows_spectators,
            rules: self.rules,
        }
    }

    pub fn select(
        &mut self,
        choice: TeamChoice,
        random_bit: bool,
    ) -> Result<Option<PlayerTeam>, TeamSelectionError> {
        let snapshot = self.snapshot();
        if self.rules.game_over
            || self.rules.training
            || (snapshot.local_team.is_gameplay() && !self.rules.allow_team_change)
        {
            return Err(TeamSelectionError::TeamChangeDisallowed);
        }
        let target = match choice {
            TeamChoice::Red => PlayerTeam::Red,
            TeamChoice::Blue => PlayerTeam::Blue,
            TeamChoice::Spectator => PlayerTeam::Spectator,
            TeamChoice::Auto => {
                if self.rules.highlander
                    && snapshot.red_count >= HIGHLANDER_TEAM_SIZE
                    && snapshot.blue_count >= HIGHLANDER_TEAM_SIZE
                {
                    PlayerTeam::Spectator
                } else if self.rules.mann_vs_machine {
                    PlayerTeam::Red
                } else if snapshot.blue_count < snapshot.red_count {
                    PlayerTeam::Blue
                } else if snapshot.red_count < snapshot.blue_count {
                    PlayerTeam::Red
                } else if self.rules.attack_defend || !random_bit {
                    PlayerTeam::Blue
                } else {
                    PlayerTeam::Red
                }
            }
        };
        if target == snapshot.local_team {
            return Ok(None);
        }
        if target == PlayerTeam::Spectator {
            if !self.rules.allows_spectators {
                return Err(TeamSelectionError::SpectatorsDisallowed);
            }
            if self.rules.spectators_restricted && snapshot.local_team.is_gameplay() {
                let gap = if snapshot.local_team == PlayerTeam::Red {
                    snapshot.blue_count as isize - snapshot.red_count as isize
                } else {
                    snapshot.red_count as isize - snapshot.blue_count as isize
                };
                if gap >= isize::from(self.rules.unbalance_limit) {
                    return Err(TeamSelectionError::SpectatorUnbalancesTeams);
                }
            }
        } else if (target == PlayerTeam::Red && snapshot.red_disabled)
            || (target == PlayerTeam::Blue && snapshot.blue_disabled)
        {
            return Err(TeamSelectionError::TeamDisabled);
        }
        self.roster
            .iter_mut()
            .find(|player| player.identity == self.local_identity)
            .expect("validated local roster identity")
            .team = target;
        Ok(Some(target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_join_has_no_fake_players_or_cancel() {
        let state = TeamSelection::new(1, TeamRules::default()).unwrap();
        let snapshot = state.snapshot();
        assert_eq!(snapshot.local_team, PlayerTeam::Unassigned);
        assert_eq!((snapshot.red_count, snapshot.blue_count), (0, 0));
        assert!(!snapshot.cancel_visible);
        assert!(snapshot.spectators_visible && snapshot.auto_assign_visible);
        assert!(!snapshot.red_disabled && !snapshot.blue_disabled);
    }

    #[test]
    fn choice_updates_the_authoritative_roster_once() {
        let mut state = TeamSelection::new(1, TeamRules::default()).unwrap();
        assert_eq!(
            state.select(TeamChoice::Red, false),
            Ok(Some(PlayerTeam::Red))
        );
        assert_eq!(
            (state.snapshot().red_count, state.snapshot().blue_count),
            (1, 0)
        );
        assert_eq!(state.select(TeamChoice::Red, true), Ok(None));
        assert_eq!(
            state.select(TeamChoice::Blue, true),
            Ok(Some(PlayerTeam::Blue))
        );
        assert_eq!(
            (state.snapshot().red_count, state.snapshot().blue_count),
            (0, 1)
        );
    }

    #[test]
    fn balance_and_highlander_visibility_follow_source_rules() {
        let mut state = TeamSelection::new(1, TeamRules::default()).unwrap();
        state
            .replace_roster(vec![
                RosterPlayer {
                    identity: 1,
                    team: PlayerTeam::Unassigned,
                },
                RosterPlayer {
                    identity: 2,
                    team: PlayerTeam::Red,
                },
            ])
            .unwrap();
        assert!(state.snapshot().red_disabled);
        assert!(!state.snapshot().blue_disabled);
        assert_eq!(
            state.select(TeamChoice::Red, false),
            Err(TeamSelectionError::TeamDisabled)
        );
        assert_eq!(
            state.select(TeamChoice::Auto, true),
            Ok(Some(PlayerTeam::Blue))
        );

        let mut roster = vec![RosterPlayer {
            identity: 1,
            team: PlayerTeam::Unassigned,
        }];
        for identity in 2..=19 {
            roster.push(RosterPlayer {
                identity,
                team: if identity % 2 == 0 {
                    PlayerTeam::Red
                } else {
                    PlayerTeam::Blue
                },
            });
        }
        state.replace_roster(roster).unwrap();
        state.replace_rules(TeamRules {
            highlander: true,
            allows_spectators: false,
            ..TeamRules::default()
        });
        let full = state.snapshot();
        assert!(full.teams_full);
        assert!(!full.auto_assign_visible && !full.spectators_visible && !full.teams_full_arrow);
    }

    #[test]
    fn ties_use_source_random_choice_and_attack_defend_prefers_blu() {
        let mut blue = TeamSelection::new(1, TeamRules::default()).unwrap();
        assert_eq!(
            blue.select(TeamChoice::Auto, false),
            Ok(Some(PlayerTeam::Blue))
        );
        let mut red = TeamSelection::new(1, TeamRules::default()).unwrap();
        assert_eq!(
            red.select(TeamChoice::Auto, true),
            Ok(Some(PlayerTeam::Red))
        );
        let mut attack = TeamSelection::new(
            1,
            TeamRules {
                attack_defend: true,
                ..TeamRules::default()
            },
        )
        .unwrap();
        assert_eq!(
            attack.select(TeamChoice::Auto, true),
            Ok(Some(PlayerTeam::Blue))
        );
    }

    #[test]
    fn roster_replacement_rejects_duplicate_missing_and_out_of_range_players_atomically() {
        let mut state = TeamSelection::new(1, TeamRules::default()).unwrap();
        let initial = state.snapshot();
        assert_eq!(
            state.replace_roster(vec![
                RosterPlayer {
                    identity: 1,
                    team: PlayerTeam::Red
                },
                RosterPlayer {
                    identity: 1,
                    team: PlayerTeam::Blue
                },
            ]),
            Err(TeamSelectionError::DuplicatePlayer)
        );
        assert_eq!(state.snapshot(), initial);
        assert_eq!(
            state.replace_roster(vec![RosterPlayer {
                identity: 2,
                team: PlayerTeam::Blue
            }]),
            Err(TeamSelectionError::MissingLocalPlayer)
        );
        assert_eq!(state.snapshot(), initial);
    }

    #[test]
    fn mvm_disables_blu_and_respects_exact_defender_capacity() {
        let mut state = TeamSelection::new(
            1,
            TeamRules {
                mann_vs_machine: true,
                defenders_team_size: 1,
                ..TeamRules::default()
            },
        )
        .unwrap();
        assert!(state.snapshot().blue_disabled);
        assert_eq!(
            state.select(TeamChoice::Auto, false),
            Ok(Some(PlayerTeam::Red))
        );
        assert!(state.snapshot().red_disabled);
    }

    #[test]
    fn spectator_permission_is_checked_against_live_state() {
        let mut state = TeamSelection::new(
            1,
            TeamRules {
                allows_spectators: false,
                ..TeamRules::default()
            },
        )
        .unwrap();
        assert_eq!(
            state.select(TeamChoice::Spectator, false),
            Err(TeamSelectionError::SpectatorsDisallowed)
        );
        state.replace_rules(TeamRules::default());
        assert_eq!(
            state.select(TeamChoice::Spectator, false),
            Ok(Some(PlayerTeam::Spectator))
        );
        assert!(state.snapshot().cancel_visible);
    }
}
