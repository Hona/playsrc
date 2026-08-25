use playsrc_entity::{Entity, Graph};

use crate::PlayerTeam;

pub const DEFAULT_WAITING_SECONDS: f32 = 30.0;
pub const DEFAULT_PREROUND_SECONDS: f32 = 5.0;
pub const DEFAULT_BONUS_SECONDS: f32 = 15.0;
pub const DEFAULT_STALEMATE_SECONDS: f32 = 240.0;
pub const WIN_REASON_OPPONENTS_DEAD: u8 = 2;
pub const WIN_REASON_FLAG_CAPTURE_LIMIT: u8 = 3;
pub const WIN_REASON_DEFEND_UNTIL_TIME_LIMIT: u8 = 4;
pub const WIN_REASON_STALEMATE: u8 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum State {
    Init = 0,
    Pregame = 1,
    StartGame = 2,
    Preround = 3,
    Running = 4,
    TeamWin = 5,
    Restart = 6,
    Stalemate = 7,
    GameOver = 8,
    Bonus = 9,
    BetweenRounds = 10,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TimerState {
    Normal = 0,
    Setup = 1,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimerConfiguration {
    pub identity: u32,
    pub initial_seconds: u32,
    pub setup_seconds: u32,
    pub maximum_seconds: u32,
    pub show_in_hud: bool,
    pub auto_countdown: bool,
    pub start_paused: bool,
    pub reset_on_round_start: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timer {
    pub configuration: TimerConfiguration,
    pub state: TimerState,
    pub remaining: f32,
    pub paused: bool,
    pub disabled: bool,
    finished: bool,
}

impl Timer {
    fn new(configuration: TimerConfiguration) -> Self {
        let setup = configuration.setup_seconds > 0;
        Self {
            configuration,
            state: if setup {
                TimerState::Setup
            } else {
                TimerState::Normal
            },
            remaining: if setup {
                configuration.setup_seconds as f32
            } else {
                configuration.initial_seconds as f32
            },
            paused: true,
            disabled: false,
            finished: false,
        }
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        if !self.disabled {
            self.paused = false;
        }
    }

    pub fn set_time(&mut self, seconds: u32) {
        self.remaining = seconds as f32;
        self.finished = false;
    }

    pub fn add_time(&mut self, seconds: i32) {
        let maximum = if self.state == TimerState::Setup {
            self.configuration.setup_seconds
        } else {
            self.configuration.maximum_seconds
        };
        self.remaining = (self.remaining + seconds as f32)
            .max(0.0)
            .min(if maximum == 0 {
                f32::MAX
            } else {
                maximum as f32
            });
        self.finished = false;
    }

    fn reset(&mut self) {
        *self = Self::new(self.configuration);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Configuration {
    pub waiting_seconds: f32,
    pub preround_seconds: f32,
    pub bonus_seconds: f32,
    pub stalemate_enabled: bool,
    pub stalemate_seconds: f32,
    pub timer: Option<TimerConfiguration>,
    pub defending_team: Option<PlayerTeam>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            waiting_seconds: DEFAULT_WAITING_SECONDS,
            preround_seconds: DEFAULT_PREROUND_SECONDS,
            bonus_seconds: DEFAULT_BONUS_SECONDS,
            stalemate_enabled: false,
            stalemate_seconds: DEFAULT_STALEMATE_SECONDS,
            timer: None,
            defending_team: None,
        }
    }
}

impl Configuration {
    pub fn from_graph(graph: &Graph) -> Result<Self, Error> {
        let mut result = Self::default();
        for entity in &graph.entities {
            if !class(entity, b"team_round_timer") {
                continue;
            }
            if result.timer.is_some() {
                return Err(Error::MultipleTimers);
            }
            let identity = u32::try_from(entity.index).map_err(|_| Error::InvalidTimer)?;
            let initial_seconds = unsigned(entity, b"timer_length", 0)?;
            let setup_seconds = unsigned(entity, b"setup_length", 0)?;
            let maximum_seconds = unsigned(entity, b"max_length", 0)?;
            result.timer = Some(TimerConfiguration {
                identity,
                initial_seconds,
                setup_seconds,
                maximum_seconds,
                show_in_hud: boolean(entity, b"show_in_hud", false)?,
                auto_countdown: boolean(entity, b"auto_countdown", false)?,
                start_paused: boolean(entity, b"start_paused", false)?,
                reset_on_round_start: boolean(entity, b"reset_time", true)?,
            });
        }
        if graph
            .entities
            .iter()
            .any(|entity| class(entity, b"team_train_watcher"))
        {
            result.defending_team = Some(PlayerTeam::Red);
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidConfiguration,
    InvalidTimer,
    MultipleTimers,
    InvalidWinner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Event {
    StateChanged { previous: State, current: State },
    WaitingBegan,
    WaitingAboutToEnd,
    WaitingEnded,
    RoundStarted { full_reset: bool },
    RoundActive,
    SetupFinished { timer: u32 },
    TimerFinished { timer: u32 },
    OvertimeChanged { active: bool },
    RoundWon { team: PlayerTeam, reason: u8 },
    RoundRespawn,
    ScoresReset,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub state: State,
    pub waiting_for_players: bool,
    pub waiting_remaining: Option<f32>,
    pub in_setup: bool,
    pub in_overtime: bool,
    pub winning_team: Option<PlayerTeam>,
    pub win_reason: u8,
    pub red_score: u16,
    pub blue_score: u16,
    pub rounds_played: u32,
    pub timer: Option<Timer>,
    pub events: Vec<Event>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Facts {
    pub red_players: usize,
    pub blue_players: usize,
    pub red_alive: usize,
    pub blue_alive: usize,
    pub objective_contested: bool,
    pub flag_away_from_home: bool,
}

impl Facts {
    fn have_players(self) -> bool {
        self.red_players + self.blue_players > 0
    }
}

#[derive(Clone, Debug)]
pub struct Rules {
    configuration: Configuration,
    state: State,
    timer: Option<Timer>,
    now: f32,
    transition_at: Option<f32>,
    waiting_until: Option<f32>,
    waiting_warning: bool,
    waiting_completed: bool,
    in_overtime: bool,
    winning_team: Option<PlayerTeam>,
    win_reason: u8,
    red_score: u16,
    blue_score: u16,
    rounds_played: u32,
}

impl Rules {
    pub fn new(configuration: Configuration) -> Result<Self, Error> {
        if [
            configuration.waiting_seconds,
            configuration.preround_seconds,
            configuration.bonus_seconds,
            configuration.stalemate_seconds,
        ]
        .into_iter()
        .any(|value| !value.is_finite() || value < 0.0)
            || configuration
                .defending_team
                .is_some_and(|team| !team.is_gameplay())
        {
            return Err(Error::InvalidConfiguration);
        }
        Ok(Self {
            configuration,
            state: State::Init,
            timer: configuration.timer.map(Timer::new),
            now: 0.0,
            transition_at: None,
            waiting_until: None,
            waiting_warning: false,
            waiting_completed: false,
            in_overtime: false,
            winning_team: None,
            win_reason: 0,
            red_score: 0,
            blue_score: 0,
            rounds_played: 0,
        })
    }

    pub fn active(configuration: Configuration) -> Result<Self, Error> {
        let mut rules = Self::new(configuration)?;
        rules.state = State::Running;
        rules.waiting_completed = true;
        if let Some(timer) = &mut rules.timer
            && timer.configuration.auto_countdown
            && !timer.configuration.start_paused
        {
            timer.resume();
        }
        Ok(rules)
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn timer(&self) -> Option<&Timer> {
        self.timer.as_ref()
    }

    pub fn timer_mut(&mut self) -> Option<&mut Timer> {
        self.timer.as_mut()
    }

    pub fn waiting_for_players(&self) -> bool {
        self.waiting_until.is_some()
    }

    pub fn winning_team(&self) -> Option<PlayerTeam> {
        self.winning_team
    }

    pub fn configure_waiting(&mut self, seconds: f32) -> Result<(), Error> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(Error::InvalidConfiguration);
        }
        self.configuration.waiting_seconds = seconds;
        Ok(())
    }

    pub fn snapshot(&self, events: Vec<Event>) -> Snapshot {
        Snapshot {
            state: self.state,
            waiting_for_players: self.waiting_for_players(),
            waiting_remaining: self.waiting_until.map(|until| (until - self.now).max(0.0)),
            in_setup: self
                .timer
                .as_ref()
                .is_some_and(|timer| timer.state == TimerState::Setup),
            in_overtime: self.in_overtime,
            winning_team: self.winning_team,
            win_reason: self.win_reason,
            red_score: self.red_score,
            blue_score: self.blue_score,
            rounds_played: self.rounds_played,
            timer: self.timer,
            events,
        }
    }

    pub fn win(&mut self, team: PlayerTeam, reason: u8) -> Result<Vec<Event>, Error> {
        if !team.is_gameplay() || reason == 0 {
            return Err(Error::InvalidWinner);
        }
        if self.state == State::TeamWin {
            return Ok(Vec::new());
        }
        self.winning_team = Some(team);
        self.win_reason = reason;
        if team == PlayerTeam::Red {
            self.red_score = self.red_score.saturating_add(1);
        } else {
            self.blue_score = self.blue_score.saturating_add(1);
        }
        self.rounds_played = self.rounds_played.saturating_add(1);
        if let Some(timer) = &mut self.timer {
            timer.pause();
        }
        let mut events = vec![Event::RoundWon { team, reason }];
        self.transition(State::TeamWin, &mut events);
        self.transition_at = Some(self.now + self.configuration.bonus_seconds.max(5.0));
        Ok(events)
    }

    pub fn restart(&mut self, reset_scores: bool) -> Vec<Event> {
        let mut events = Vec::new();
        if reset_scores {
            self.red_score = 0;
            self.blue_score = 0;
            self.rounds_played = 0;
            events.push(Event::ScoresReset);
        }
        self.enter_preround(reset_scores, &mut events);
        events
    }

    pub fn advance(&mut self, now: f32, interval: f32, facts: Facts) -> Result<Vec<Event>, Error> {
        if !now.is_finite() || now < self.now || !interval.is_finite() || interval <= 0.0 {
            return Err(Error::InvalidConfiguration);
        }
        self.now = now;
        let mut events = Vec::new();
        match self.state {
            State::Init => self.transition(State::Pregame, &mut events),
            State::Pregame if facts.have_players() => {
                self.transition(State::StartGame, &mut events);
                self.transition_at = Some(now);
            }
            State::StartGame if self.transition_at.is_some_and(|time| now > time) => {
                if !self.waiting_completed && self.configuration.waiting_seconds > 0.0 {
                    self.waiting_until = Some(now + self.configuration.waiting_seconds);
                    self.waiting_warning = false;
                    events.push(Event::WaitingBegan);
                }
                self.enter_preround(false, &mut events);
            }
            State::Preround if self.transition_at.is_some_and(|time| now > time) => {
                self.transition(State::Running, &mut events);
                events.push(Event::RoundActive);
                if self.waiting_until.is_none()
                    && let Some(timer) = &mut self.timer
                    && timer.configuration.auto_countdown
                    && !timer.configuration.start_paused
                {
                    timer.resume();
                }
            }
            State::Running | State::Stalemate if !facts.have_players() => {
                self.transition(State::Pregame, &mut events);
            }
            State::TeamWin if self.transition_at.is_some_and(|time| now > time) => {
                self.enter_preround(false, &mut events);
            }
            State::Stalemate => self.stalemate(facts, &mut events),
            _ => {}
        }

        if let Some(until) = self.waiting_until {
            if !self.waiting_warning && until - now <= 10.0 {
                self.waiting_warning = true;
                events.push(Event::WaitingAboutToEnd);
            }
            if now > until {
                self.waiting_until = None;
                self.waiting_completed = true;
                events.push(Event::WaitingEnded);
                self.enter_preround(true, &mut events);
            }
        } else if self.state == State::Running {
            self.advance_timer(interval, facts, &mut events)?;
        }
        Ok(events)
    }

    fn advance_timer(
        &mut self,
        interval: f32,
        facts: Facts,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let Some(timer) = &mut self.timer else {
            return Ok(());
        };
        if timer.paused || timer.disabled || timer.finished {
            return Ok(());
        }
        timer.remaining = (timer.remaining - interval).max(0.0);
        if timer.remaining > 0.0 {
            if self.in_overtime {
                self.in_overtime = false;
                events.push(Event::OvertimeChanged { active: false });
            }
            return Ok(());
        }
        let identity = timer.configuration.identity;
        if timer.state == TimerState::Setup {
            timer.state = TimerState::Normal;
            timer.remaining = timer.configuration.initial_seconds as f32;
            events.push(Event::SetupFinished { timer: identity });
            return Ok(());
        }
        if facts.objective_contested || facts.flag_away_from_home {
            if !self.in_overtime {
                self.in_overtime = true;
                events.push(Event::OvertimeChanged { active: true });
            }
            return Ok(());
        }
        timer.finished = true;
        events.push(Event::TimerFinished { timer: identity });
        if self.in_overtime {
            self.in_overtime = false;
            events.push(Event::OvertimeChanged { active: false });
        }
        if let Some(team) = self.configuration.defending_team {
            events.extend(self.win(team, WIN_REASON_DEFEND_UNTIL_TIME_LIMIT)?);
        } else if self.configuration.stalemate_enabled {
            self.transition(State::Stalemate, events);
            self.transition_at = Some(self.now + self.configuration.stalemate_seconds);
        }
        Ok(())
    }

    fn stalemate(&mut self, facts: Facts, events: &mut Vec<Event>) {
        let winner = if facts.red_players > 0 && facts.red_alive == 0 && facts.blue_alive > 0 {
            Some(PlayerTeam::Blue)
        } else if facts.blue_players > 0 && facts.blue_alive == 0 && facts.red_alive > 0 {
            Some(PlayerTeam::Red)
        } else {
            None
        };
        if let Some(team) = winner {
            events.extend(
                self.win(team, WIN_REASON_OPPONENTS_DEAD)
                    .expect("valid stalemate winner"),
            );
        } else if self.transition_at.is_some_and(|time| self.now > time) {
            self.enter_preround(false, events);
        }
    }

    fn enter_preround(&mut self, full_reset: bool, events: &mut Vec<Event>) {
        self.winning_team = None;
        self.win_reason = 0;
        self.in_overtime = false;
        if let Some(timer) = &mut self.timer
            && (timer.configuration.reset_on_round_start || full_reset)
        {
            timer.reset();
        }
        self.transition(State::Preround, events);
        self.transition_at = Some(self.now + self.configuration.preround_seconds);
        events.push(Event::RoundRespawn);
        events.push(Event::RoundStarted { full_reset });
    }

    fn transition(&mut self, current: State, events: &mut Vec<Event>) {
        if self.state != current {
            events.push(Event::StateChanged {
                previous: self.state,
                current,
            });
            self.state = current;
        }
    }
}

fn class(entity: &Entity, expected: &[u8]) -> bool {
    entity
        .classname
        .as_ref()
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}

fn value<'a>(entity: &'a Entity, name: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .rev()
        .find(|pair| pair.key.eq_ignore_ascii_case(name))
        .map(|pair| pair.value.as_slice())
}

fn unsigned(entity: &Entity, name: &[u8], default: u32) -> Result<u32, Error> {
    value(entity, name).map_or(Ok(default), |bytes| {
        std::str::from_utf8(bytes)
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or(Error::InvalidTimer)
    })
}

fn boolean(entity: &Entity, name: &[u8], default: bool) -> Result<bool, Error> {
    match value(entity, name) {
        None => Ok(default),
        Some(b"0") => Ok(false),
        Some(b"1") => Ok(true),
        _ => Err(Error::InvalidTimer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            red_players: 1,
            blue_players: 1,
            red_alive: 1,
            blue_alive: 1,
            ..Facts::default()
        }
    }

    #[test]
    fn waiting_preserves_exact_state_order_and_restarts_after_thirty_seconds() {
        let mut rules = Rules::new(Configuration::default()).unwrap();
        assert_eq!(
            rules.advance(0.0, 0.015, facts()).unwrap()[0],
            Event::StateChanged {
                previous: State::Init,
                current: State::Pregame
            }
        );
        rules.advance(0.015, 0.015, facts()).unwrap();
        let start = rules.advance(0.030, 0.015, facts()).unwrap();
        assert!(start.contains(&Event::WaitingBegan));
        assert!(start.contains(&Event::RoundRespawn));
        assert_eq!(rules.state(), State::Preround);
        rules.advance(5.045, 0.015, facts()).unwrap();
        assert_eq!(rules.state(), State::Running);
        assert!(rules.waiting_for_players());
        let warning = rules.advance(20.030, 0.015, facts()).unwrap();
        assert!(warning.contains(&Event::WaitingAboutToEnd));
        let end = rules.advance(30.045, 0.015, facts()).unwrap();
        assert!(end.contains(&Event::WaitingEnded));
        assert!(end.contains(&Event::RoundStarted { full_reset: true }));
        assert_eq!(rules.state(), State::Preround);
    }

    fn timer() -> TimerConfiguration {
        TimerConfiguration {
            identity: 9,
            initial_seconds: 330,
            setup_seconds: 70,
            maximum_seconds: 600,
            show_in_hud: true,
            auto_countdown: true,
            start_paused: false,
            reset_on_round_start: true,
        }
    }

    #[test]
    fn upward_setup_then_defender_victory_uses_authored_timer_lengths() {
        let mut rules = Rules::active(Configuration {
            timer: Some(timer()),
            defending_team: Some(PlayerTeam::Red),
            ..Configuration::default()
        })
        .unwrap();
        assert_eq!(rules.timer().unwrap().remaining, 70.0);
        for second in 1..=70 {
            let events = rules.advance(second as f32, 1.0, facts()).unwrap();
            assert_eq!(
                events.contains(&Event::SetupFinished { timer: 9 }),
                second == 70
            );
        }
        assert_eq!(rules.timer().unwrap().remaining, 330.0);
        for second in 71..=400 {
            rules.advance(second as f32, 1.0, facts()).unwrap();
        }
        let snapshot = rules.snapshot(Vec::new());
        assert_eq!(snapshot.state, State::TeamWin);
        assert_eq!(snapshot.winning_team, Some(PlayerTeam::Red));
        assert_eq!(snapshot.win_reason, WIN_REASON_DEFEND_UNTIL_TIME_LIMIT);
        assert_eq!(snapshot.red_score, 1);
    }

    #[test]
    fn contested_timer_enters_overtime_and_finishes_only_when_clear() {
        let mut rules = Rules::active(Configuration {
            timer: Some(TimerConfiguration {
                initial_seconds: 1,
                setup_seconds: 0,
                ..timer()
            }),
            defending_team: Some(PlayerTeam::Red),
            ..Configuration::default()
        })
        .unwrap();
        let contested = Facts {
            objective_contested: true,
            ..facts()
        };
        assert!(
            rules
                .advance(1.0, 1.0, contested)
                .unwrap()
                .contains(&Event::OvertimeChanged { active: true })
        );
        assert_eq!(rules.state(), State::Running);
        let result = rules.advance(2.0, 1.0, facts()).unwrap();
        assert!(result.contains(&Event::OvertimeChanged { active: false }));
        assert_eq!(rules.state(), State::TeamWin);
    }

    #[test]
    fn round_scores_persist_across_bonus_and_reset_only_when_requested() {
        let mut rules = Rules::active(Configuration::default()).unwrap();
        rules
            .win(PlayerTeam::Blue, WIN_REASON_FLAG_CAPTURE_LIMIT)
            .unwrap();
        assert_eq!(rules.snapshot(Vec::new()).blue_score, 1);
        rules.advance(15.0, 0.015, facts()).unwrap();
        assert_eq!(rules.state(), State::TeamWin);
        rules.advance(15.015, 0.015, facts()).unwrap();
        assert_eq!(rules.state(), State::Preround);
        assert_eq!(rules.snapshot(Vec::new()).blue_score, 1);
        assert!(rules.restart(true).contains(&Event::ScoresReset));
        assert_eq!(rules.snapshot(Vec::new()).blue_score, 0);
    }

    #[test]
    fn authored_upward_timer_configuration_is_parsed_without_cart_simulation() {
        let graph = playsrc_entity::parse(b"{\"classname\"\"team_round_timer\"\"timer_length\"\"330\"\"setup_length\"\"70\"\"max_length\"\"600\"\"show_in_hud\"\"1\"\"auto_countdown\"\"1\"\"start_paused\"\"0\"}\n{\"classname\"\"team_train_watcher\"}\0", playsrc_entity::Limits::default()).unwrap();
        let configuration = Configuration::from_graph(&graph).unwrap();
        assert_eq!(
            configuration.timer.unwrap(),
            TimerConfiguration {
                identity: 0,
                ..timer()
            }
        );
        assert_eq!(configuration.defending_team, Some(PlayerTeam::Red));
    }
}
