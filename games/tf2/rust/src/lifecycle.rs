use crate::PlayerTeam;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScoreCounters {
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub captures: u32,
    pub headshots: u32,
    pub backstabs: u32,
    pub damage: u32,
    pub killstreak: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreEntry {
    pub identity: u32,
    pub team: PlayerTeam,
    pub counters: ScoreCounters,
    pub respawn_tick: Option<u64>,
}
