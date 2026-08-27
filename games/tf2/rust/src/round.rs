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
    pub initial_seconds: i32,
    pub setup_seconds: i32,
    pub maximum_seconds: i32,
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
    now: f32,
    end_time: f32,
    paused_remaining: f32,
    next_think: f32,
    outputs: u16,
    warnings: u16,
}

impl Timer {
    fn new(configuration: TimerConfiguration) -> Self {
        let setup = configuration.setup_seconds > 0;
        let remaining = if setup {
            configuration.setup_seconds
        } else {
            configuration.initial_seconds
        } as f32;
        Self {
            configuration,
            state: if setup {
                TimerState::Setup
            } else {
                TimerState::Normal
            },
            remaining: remaining.max(0.0),
            paused: true,
            disabled: false,
            finished: remaining <= 0.0,
            now: 0.0,
            end_time: remaining,
            paused_remaining: remaining,
            next_think: 0.05,
            outputs: output_flags(remaining),
            warnings: output_flags(if remaining > 10.0 {
                remaining + 2.0
            } else {
                remaining
            }),
        }
    }

    pub fn pause(&mut self) {
        if !self.disabled && !self.paused {
            self.paused_remaining = self.end_time - self.now;
            self.paused = true;
        }
    }

    pub fn resume(&mut self) {
        if !self.disabled && self.paused {
            self.end_time = self.now + self.paused_remaining;
            self.paused = false;
        }
    }

    pub fn set_time(&mut self, seconds: i32) {
        if self.disabled {
            return;
        }
        let seconds = if self.configuration.maximum_seconds > 0 {
            seconds.min(self.configuration.maximum_seconds)
        } else {
            seconds
        };
        self.paused_remaining = seconds as f32;
        self.end_time = self.now + seconds as f32;
        self.refresh(self.now);
        self.recalculate_outputs();
    }

    pub fn add_time(&mut self, seconds: i32) -> Option<i32> {
        if self.disabled {
            return None;
        }
        let maximum = self.configuration.maximum_seconds;
        let seconds = if maximum > 0 && self.remaining + seconds as f32 > maximum as f32 {
            (maximum as f32 - self.remaining) as i32
        } else {
            seconds
        };
        if self.paused {
            self.paused_remaining += seconds as f32;
        } else {
            self.end_time += seconds as f32;
        }
        self.refresh(self.now);
        self.recalculate_outputs();
        Some(seconds)
    }

    fn refresh(&mut self, now: f32) {
        self.now = now;
        self.remaining = if self.paused {
            self.paused_remaining
        } else {
            self.end_time - now
        }
        .max(0.0);
    }

    fn recalculate_outputs(&mut self) {
        self.finished = self.remaining <= 0.0;
        self.outputs = output_flags(self.remaining);
        self.warnings = output_flags(if self.remaining > 10.0 {
            self.remaining + 2.0
        } else {
            self.remaining
        });
    }

    fn reset(&mut self) {
        let now = self.now;
        *self = Self::new(self.configuration);
        self.now = now;
        self.end_time = now + self.paused_remaining;
        self.next_think = now + 0.05;
    }
}

const OUTPUT_SECONDS: [u16; 12] = [300, 240, 180, 120, 60, 30, 10, 5, 4, 3, 2, 1];

fn output_flags(remaining: f32) -> u16 {
    OUTPUT_SECONDS
        .iter()
        .enumerate()
        .fold(0, |flags, (index, seconds)| {
            flags | (u16::from(remaining >= f32::from(*seconds)) << index)
        })
}

#[derive(Clone, Debug, PartialEq)]
pub struct Configuration {
    pub waiting_seconds: f32,
    pub preround_seconds: f32,
    pub bonus_seconds: f32,
    pub stalemate_enabled: bool,
    pub stalemate_seconds: f32,
    pub timers: Vec<TimerConfiguration>,
    pub koth: Option<crate::koth::Configuration>,
    pub round_wins: Vec<RoundWin>,
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
            timers: Vec::new(),
            koth: None,
            round_wins: Vec::new(),
            defending_team: None,
        }
    }
}

impl Configuration {
    pub fn from_graph(graph: &Graph) -> Result<Self, Error> {
        let mut result = Self::default();
        for entity in &graph.entities {
            if class(entity, b"game_round_win") {
                result.round_wins.push(RoundWin {
                    identity: entity.index as u32,
                    team: integer(entity, b"TeamNum", 0)?,
                    reason: integer(
                        entity,
                        b"win_reason",
                        i32::from(WIN_REASON_DEFEND_UNTIL_TIME_LIMIT),
                    )? as u8,
                });
            }
            if !class(entity, b"team_round_timer") {
                continue;
            }
            let identity = u32::try_from(entity.index).map_err(|_| Error::InvalidTimer)?;
            let initial_seconds = integer(entity, b"timer_length", 0)?;
            let setup_seconds = integer(entity, b"setup_length", 0)?;
            let maximum_seconds = integer(entity, b"max_length", 0)?;
            result.timers.push(TimerConfiguration {
                identity,
                initial_seconds,
                setup_seconds,
                maximum_seconds,
                show_in_hud: boolean(entity, b"show_in_hud", false)?,
                auto_countdown: boolean(entity, b"auto_countdown", true)?,
                start_paused: boolean(entity, b"start_paused", true)?,
                reset_on_round_start: boolean(entity, b"reset_time", false)?,
            });
        }
        if graph
            .entities
            .iter()
            .any(|entity| class(entity, b"team_train_watcher"))
        {
            result.defending_team = Some(PlayerTeam::Red);
        }
        result.koth = crate::koth::Configuration::from_graph(graph)?;
        if let Some(koth) = result.koth {
            result.timers.extend(koth.timers());
        }
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidConfiguration,
    InvalidTimer,
    InvalidWinner,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundWin {
    identity: u32,
    team: i32,
    reason: u8,
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
    TimerThreshold { timer: u32, seconds: u16 },
    TimerWarning { timer: u32, seconds: u16 },
    TimerTimeAdded { timer: u32, seconds: i32 },
    WinningCapper { player: u32 },
    MapRoundWin { entity: u32 },
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
    /// RED, BLU. Separate from the ordinary waiting/setup timer panel.
    pub koth_timers: Option<[Timer; 2]>,
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
    timers: Vec<Timer>,
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
    cap_in_progress_until: f32,
    pending_events: Vec<Event>,
    most_recent_cappers: Vec<u32>,
    respawn_waves: [Option<f32>; 2],
    original_respawn_waves: [Option<f32>; 2],
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
            timers: configuration
                .timers
                .iter()
                .copied()
                .map(Timer::new)
                .collect(),
            configuration,
            state: State::Init,
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
            cap_in_progress_until: 0.0,
            pending_events: Vec::new(),
            most_recent_cappers: Vec::new(),
            respawn_waves: [None; 2],
            original_respawn_waves: [None; 2],
        })
    }

    pub fn active(configuration: Configuration) -> Result<Self, Error> {
        let mut rules = Self::new(configuration)?;
        rules.state = State::Running;
        rules.waiting_completed = true;
        for timer in &mut rules.timers {
            if !timer.configuration.start_paused {
                timer.resume();
            }
        }
        Ok(rules)
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn timer(&self) -> Option<&Timer> {
        self.timers
            .iter()
            .rev()
            .find(|timer| timer.configuration.show_in_hud)
            .or_else(|| self.timers.first())
    }

    pub fn timer_mut(&mut self, identity: u32) -> Option<&mut Timer> {
        self.timers
            .iter_mut()
            .find(|timer| timer.configuration.identity == identity)
    }

    pub fn koth_configuration(&self) -> Option<crate::koth::Configuration> {
        self.configuration.koth
    }

    pub fn koth_timers(&self) -> Option<[Timer; 2]> {
        let koth = self.configuration.koth?;
        Some([koth.red_timer, koth.blue_timer].map(|identity| {
            *self
                .timers
                .iter()
                .find(|timer| timer.configuration.identity == identity)
                .expect("KOTH timer")
        }))
    }

    /// CTFBot::GetTimeLeftToCapture uses the enemy's independent KOTH clock.
    pub fn bot_capture_time_left(&self) -> [f32; 2] {
        self.koth_timers().map_or_else(
            || [self.timer().map_or(0.0, |timer| timer.remaining); 2],
            |[red, blue]| [blue.remaining, red.remaining],
        )
    }

    pub fn timer_may_expire(&self) -> bool {
        self.cap_in_progress_until < self.now
    }

    /// Apply an input already resolved by the entity I/O world. No point state
    /// is inferred here: maps explicitly select the active KOTH clock.
    pub fn apply_input(&mut self, entity: u32, input: &[u8], value: i32, now: f32) {
        self.now = now;
        for timer in &mut self.timers {
            timer.refresh(now);
        }
        if let Some(win) = self
            .configuration
            .round_wins
            .iter_mut()
            .find(|win| win.identity == entity)
        {
            if input.eq_ignore_ascii_case(b"SetTeam") {
                win.team = value;
            } else if input.eq_ignore_ascii_case(b"RoundWin") {
                let (team, reason) = (win.team, win.reason);
                if team == 2 || team == 3 {
                    if let Ok(events) = self.win(
                        if team == 2 {
                            PlayerTeam::Red
                        } else {
                            PlayerTeam::Blue
                        },
                        reason,
                    ) {
                        self.pending_events.extend(events);
                    }
                } else {
                    let events = self.set_stalemate();
                    self.pending_events.extend(events);
                }
                self.pending_events.push(Event::MapRoundWin { entity });
            }
            return;
        }
        let koth = self.configuration.koth;
        if let Some(koth) = koth {
            let active = if input.eq_ignore_ascii_case(b"SetRedKothClockActive") {
                Some(koth.red_timer)
            } else if input.eq_ignore_ascii_case(b"SetBlueKothClockActive") {
                Some(koth.blue_timer)
            } else {
                None
            };
            if let Some(active) = active {
                // Preserve the SDK's BLU-before-RED input ordering.
                for identity in [koth.blue_timer, koth.red_timer] {
                    let timer = self.timer_mut(identity).expect("KOTH timer");
                    if identity == active {
                        timer.resume();
                    } else {
                        timer.pause();
                    }
                }
                return;
            }
            if entity == koth.identity {
                let operation = if input.eq_ignore_ascii_case(b"SetRedTimer") {
                    Some((koth.red_timer, false))
                } else if input.eq_ignore_ascii_case(b"SetBlueTimer") {
                    Some((koth.blue_timer, false))
                } else if input.eq_ignore_ascii_case(b"AddRedTimer") {
                    Some((koth.red_timer, true))
                } else if input.eq_ignore_ascii_case(b"AddBlueTimer") {
                    Some((koth.blue_timer, true))
                } else {
                    None
                };
                if let Some((identity, add)) = operation {
                    if add && !matches!(self.state, State::Running | State::TeamWin) {
                        return;
                    }
                    if add {
                        self.add_timer_seconds(identity, value);
                    } else {
                        self.timer_mut(identity)
                            .expect("KOTH timer")
                            .set_time(value);
                    }
                    return;
                }
                if input.eq_ignore_ascii_case(b"RoundSpawn") {
                    for identity in [koth.blue_timer, koth.red_timer] {
                        self.timer_mut(identity).expect("KOTH timer").reset();
                    }
                    return;
                }
            }
        }
        let running = matches!(self.state, State::Running | State::TeamWin);
        if input.eq_ignore_ascii_case(b"AddTime") && running {
            self.add_timer_seconds(entity, value);
            return;
        }
        let Some(timer) = self.timer_mut(entity) else {
            return;
        };
        if input.eq_ignore_ascii_case(b"Pause") {
            timer.pause();
        } else if input.eq_ignore_ascii_case(b"Resume") {
            timer.resume();
        } else if input.eq_ignore_ascii_case(b"SetTime") {
            timer.set_time(value);
        } else if input.eq_ignore_ascii_case(b"ShowInHUD") {
            timer.configuration.show_in_hud = value != 0;
        } else if input.eq_ignore_ascii_case(b"Disable") {
            timer.pause();
            timer.disabled = true;
        } else if input.eq_ignore_ascii_case(b"Enable") {
            timer.disabled = false;
            timer.resume();
        }
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.pending_events)
    }

    pub fn record_capture(&mut self, cappers: &[u32]) {
        self.most_recent_cappers.clear();
        self.most_recent_cappers.extend_from_slice(cappers);
    }

    pub fn recreate_map_entities(&mut self, authored: &Configuration) {
        self.configuration.round_wins = authored.round_wins.clone();
        for timer in &mut self.timers {
            if let Some(configuration) = authored.timers.iter().find(|c| c.identity == timer.configuration.identity) {
                timer.configuration = *configuration;
                timer.now = self.now;
                timer.reset();
            }
        }
    }

    fn add_timer_seconds(&mut self, identity: u32, seconds: i32) {
        if !matches!(self.state, State::Running | State::TeamWin) {
            return;
        }
        let Some(timer) = self.timer_mut(identity) else {
            return;
        };
        if let Some(seconds) = timer.add_time(seconds)
            && timer.configuration.show_in_hud
        {
            self.pending_events.push(Event::TimerTimeAdded {
                timer: identity,
                seconds,
            });
        }
    }

    pub fn set_respawn_wave(&mut self, team: PlayerTeam, seconds: f32) {
        if seconds >= 0.0 && seconds.is_finite() && team.is_gameplay() {
            let index = usize::from(team == PlayerTeam::Blue);
            self.original_respawn_waves[index].get_or_insert(seconds);
            self.respawn_waves[index] = Some(seconds);
        }
    }

    pub fn add_respawn_wave(&mut self, team: PlayerTeam, seconds: f32) {
        if !team.is_gameplay() || !seconds.is_finite() {
            return;
        }
        let index = usize::from(team == PlayerTeam::Blue);
        let current = self.respawn_waves[index].unwrap_or(crate::bot::RESPAWN_WAVE_SECONDS);
        self.original_respawn_waves[index].get_or_insert(current);
        self.respawn_waves[index] = Some((current + seconds).max(0.0));
    }

    pub fn respawn_waves(&self) -> [Option<f32>; 2] {
        self.respawn_waves
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
                .timers
                .iter()
                .any(|timer| timer.state == TimerState::Setup),
            in_overtime: self.in_overtime,
            winning_team: self.winning_team,
            win_reason: self.win_reason,
            red_score: self.red_score,
            blue_score: self.blue_score,
            rounds_played: self.rounds_played,
            timer: self.timer().copied(),
            koth_timers: self.koth_timers(),
            events,
        }
    }

    pub fn win(&mut self, team: PlayerTeam, reason: u8) -> Result<Vec<Event>, Error> {
        if (!team.is_gameplay() && !(team == PlayerTeam::Unassigned && reason == WIN_REASON_STALEMATE)) || reason == 0 {
            return Err(Error::InvalidWinner);
        }
        if self.state == State::TeamWin {
            return Ok(Vec::new());
        }
        self.winning_team = Some(team);
        self.win_reason = reason;
        if team == PlayerTeam::Red {
            self.red_score = self.red_score.saturating_add(1);
        } else if team == PlayerTeam::Blue {
            self.blue_score = self.blue_score.saturating_add(1);
        }
        self.rounds_played = self.rounds_played.saturating_add(1);
        for timer in &mut self.timers {
            timer.pause();
        }
        let mut events = vec![Event::RoundWon { team, reason }];
        if reason == 1 || reason == WIN_REASON_FLAG_CAPTURE_LIMIT {
            events.extend(
                self.most_recent_cappers
                    .iter()
                    .map(|player| Event::WinningCapper { player: *player }),
            );
        }
        self.transition(State::TeamWin, &mut events);
        self.transition_at = Some(self.now + self.configuration.bonus_seconds.max(5.0));
        Ok(events)
    }

    pub fn set_stalemate(&mut self) -> Vec<Event> {
        if self.configuration.stalemate_enabled {
            let mut events = Vec::new();
            self.transition(State::Stalemate, &mut events);
            self.transition_at = Some(self.now + self.configuration.stalemate_seconds);
            events
        } else {
            self.win(PlayerTeam::Unassigned, WIN_REASON_STALEMATE).expect("draw winner")
        }
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
        for timer in &mut self.timers {
            timer.refresh(now);
        }
        if facts.objective_contested {
            self.cap_in_progress_until = now + 0.1;
        }
        let mut events = self.take_events();
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
                if self.waiting_until.is_none() {
                    for timer in &mut self.timers {
                        if !timer.configuration.start_paused {
                            timer.resume();
                        }
                    }
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
        if !self.waiting_for_players() {
            let hud_timer = self
                .timer()
                .filter(|timer| timer.configuration.show_in_hud)
                .map(|timer| timer.configuration.identity);
            for timer in &mut self.timers {
                if timer.disabled
                    || timer.paused
                    || timer.state == TimerState::Setup
                    || !timer.configuration.auto_countdown
                    || self.configuration.koth.is_none()
                        && hud_timer != Some(timer.configuration.identity)
                {
                    continue;
                }
                for (index, seconds) in OUTPUT_SECONDS.into_iter().enumerate().skip(4) {
                    if timer.warnings & (1 << index) != 0
                        && timer.remaining <= f32::from(seconds) + 1.0
                    {
                        timer.warnings &= !(1 << index);
                        events.push(Event::TimerWarning {
                            timer: timer.configuration.identity,
                            seconds,
                        });
                        break;
                    }
                }
            }
        }
        Ok(events)
    }

    fn advance_timer(
        &mut self,
        _interval: f32,
        facts: Facts,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let may_expire = self.timer_may_expire() && !facts.flag_away_from_home;
        let mut finished_normal = false;
        for timer in &mut self.timers {
            if self.now < timer.next_think {
                continue;
            }
            timer.next_think = self.now + 0.05;
            if timer.paused || timer.disabled {
                continue;
            }
            if (timer.remaining > 0.0 && timer.configuration.show_in_hud
                || timer.state == TimerState::Setup)
                && self.in_overtime
            {
                self.in_overtime = false;
                events.push(Event::OvertimeChanged { active: false });
            }
            let identity = timer.configuration.identity;
            if timer.remaining <= 0.0 && !timer.finished {
                if timer.state == TimerState::Setup {
                    timer.state = TimerState::Normal;
                    timer.set_time(timer.configuration.initial_seconds);
                    events.push(Event::SetupFinished { timer: identity });
                    continue;
                }
                if !may_expire {
                    timer.end_time = self.now;
                    if timer.configuration.show_in_hud && !self.in_overtime {
                        self.in_overtime = true;
                        events.push(Event::OvertimeChanged { active: true });
                    }
                    continue;
                }
                timer.finished = true;
                events.push(Event::TimerFinished { timer: identity });
                finished_normal = true;
            } else {
                for (index, seconds) in OUTPUT_SECONDS.into_iter().enumerate() {
                    if timer.state == TimerState::Setup && seconds > 60 {
                        continue;
                    }
                    if timer.outputs & (1 << index) != 0 && timer.remaining <= f32::from(seconds) {
                        timer.outputs &= !(1 << index);
                        events.push(Event::TimerThreshold {
                            timer: identity,
                            seconds,
                        });
                        break;
                    }
                }
            }
        }
        if !finished_normal || self.configuration.koth.is_some() || !self.configuration.round_wins.is_empty() {
            return Ok(());
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
        self.most_recent_cappers.clear();
        self.respawn_waves = self.original_respawn_waves;
        self.winning_team = None;
        self.win_reason = 0;
        self.in_overtime = false;
        for timer in &mut self.timers {
            if timer.configuration.reset_on_round_start || full_reset {
                timer.reset();
            }
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

pub(crate) fn class(entity: &Entity, expected: &[u8]) -> bool {
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

pub(crate) fn integer(entity: &Entity, name: &[u8], default: i32) -> Result<i32, Error> {
    value(entity, name).map_or(Ok(default), |bytes| {
        let text = std::str::from_utf8(bytes).map_err(|_| Error::InvalidTimer)?;
        let text = text.trim_start();
        let (negative, digits) = if let Some(rest) = text.strip_prefix('-') {
            (true, rest)
        } else if let Some(rest) = text.strip_prefix('+') {
            (false, rest)
        } else {
            (false, text)
        };
        let mut magnitude = 0_i64;
        let mut found = false;
        for digit in digits.bytes().take_while(u8::is_ascii_digit) {
            found = true;
            magnitude = magnitude
                .saturating_mul(10)
                .saturating_add(i64::from(digit - b'0'));
        }
        if !found {
            return Ok(0);
        }
        Ok(if negative {
            magnitude.saturating_neg() as i32
        } else {
            magnitude as i32
        })
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

    #[test]
    fn point_capture_wave_adjustments_use_default_then_restore_original_on_restart() {
        let mut rules = Rules::active(Configuration::default()).unwrap();
        rules.add_respawn_wave(PlayerTeam::Blue, -4.0);
        assert_eq!(rules.respawn_waves(), [None, Some(6.0)]);
        rules.add_respawn_wave(PlayerTeam::Blue, 4.0);
        assert_eq!(rules.respawn_waves()[1], Some(10.0));
        rules.add_respawn_wave(PlayerTeam::Blue, -50.0);
        assert_eq!(rules.respawn_waves()[1], Some(0.0));
        rules.restart(false);
        assert_eq!(rules.respawn_waves()[1], Some(10.0));
        rules.set_respawn_wave(PlayerTeam::Red, 8.0);
        rules.add_respawn_wave(PlayerTeam::Red, -4.0);
        rules.restart(true);
        assert_eq!(rules.respawn_waves(), [Some(8.0), Some(10.0)]);
    }

    fn koth() -> Rules {
        let graph =
            playsrc_entity::parse(b"{\"classname\"\"tf_logic_koth\"}\0", Default::default())
                .unwrap();
        Rules::active(Configuration::from_graph(&graph).unwrap()).unwrap()
    }

    #[test]
    fn koth_independent_clocks_only_run_on_authored_activation_inputs() {
        let mut rules = koth();
        rules.advance(30.0, 0.015, facts()).unwrap();
        assert_eq!(
            rules.koth_timers().unwrap().map(|timer| timer.remaining),
            [180.0, 180.0]
        );
        rules.apply_input(99, b"SetRedKothClockActive", 0, 30.0);
        rules.advance(45.0, 0.015, facts()).unwrap();
        assert_eq!(
            rules.koth_timers().unwrap().map(|timer| timer.remaining),
            [165.0, 180.0]
        );
        rules.apply_input(99, b"SetBlueKothClockActive", 0, 45.0);
        rules.advance(55.0, 0.015, facts()).unwrap();
        assert_eq!(
            rules.koth_timers().unwrap().map(|timer| timer.remaining),
            [165.0, 170.0]
        );
        rules.apply_input(99, b"SetRedKothClockActive", 0, 55.0);
        rules.advance(60.0, 0.015, facts()).unwrap();
        assert_eq!(
            rules.koth_timers().unwrap().map(|timer| timer.remaining),
            [160.0, 170.0]
        );
    }

    #[test]
    fn bot_capture_urgency_uses_enemy_clock_not_own_or_hud_selected_clock() {
        let mut rules = koth();
        rules.apply_input(0, b"SetBlueTimer", 40, 0.0);
        assert_eq!(rules.bot_capture_time_left(), [40.0, 180.0]);
        rules.apply_input(99, b"SetBlueKothClockActive", 0, 0.0);
        rules.advance(1.0, 0.015, facts()).unwrap();
        assert_eq!(rules.bot_capture_time_left(), [39.0, 180.0]);
        rules.apply_input(0, b"SetRedTimer", 0, 1.0);
        assert_eq!(rules.bot_capture_time_left(), [39.0, 0.0]);
    }

    #[test]
    fn delayed_koth_victory_keeps_the_actual_capture_player_order_until_round_reset() {
        let mut rules = koth();
        rules.record_capture(&[2, 7, 9]);
        rules.apply_input(99, b"SetRedKothClockActive", 0, 0.0);
        rules.advance(180.0, 0.015, facts()).unwrap();
        let won = rules.win(PlayerTeam::Red, 1).unwrap();
        assert_eq!(
            won.iter()
                .filter_map(|event| match event {
                    Event::WinningCapper { player } => Some(*player),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            [2, 7, 9]
        );
        rules.restart(false);
        assert!(
            !rules
                .win(PlayerTeam::Blue, 1)
                .unwrap()
                .iter()
                .any(|event| matches!(event, Event::WinningCapper { .. }))
        );
    }

    #[test]
    fn koth_overtime_uses_capture_buffer_and_timer_finish_never_awards_victory() {
        let mut rules = koth();
        rules.apply_input(0, b"SetRedTimer", 1, 0.0);
        rules.apply_input(99, b"SetRedKothClockActive", 0, 0.0);
        let contested = Facts {
            objective_contested: true,
            ..facts()
        };
        assert!(
            rules
                .advance(1.0, 0.015, contested)
                .unwrap()
                .contains(&Event::OvertimeChanged { active: true })
        );
        assert!(!rules.timer_may_expire());
        rules.advance(1.1, 0.015, facts()).unwrap();
        assert!(!rules.timer_may_expire(), "inclusive 0.1s cap buffer");
        let events = rules.advance(1.16, 0.015, facts()).unwrap();
        assert!(rules.timer_may_expire());
        assert!(events.contains(&Event::TimerFinished { timer: 2 }));
        assert_eq!(
            rules.state(),
            State::Running,
            "master owns victory, not the timer"
        );
        assert_eq!(rules.snapshot(vec![]).red_score, 0);
    }

    #[test]
    fn koth_overtime_retake_stops_zero_clock_and_new_owner_clears_global_overtime() {
        let mut rules = koth();
        rules.apply_input(0, b"SetBlueTimer", 1, 0.0);
        rules.apply_input(99, b"SetBlueKothClockActive", 0, 0.0);
        rules
            .advance(
                1.0,
                0.015,
                Facts {
                    objective_contested: true,
                    ..facts()
                },
            )
            .unwrap();
        rules.apply_input(99, b"SetRedKothClockActive", 0, 1.0);
        let events = rules.advance(1.06, 0.015, facts()).unwrap();
        assert!(events.contains(&Event::OvertimeChanged { active: false }));
        let [red, blue] = rules.koth_timers().unwrap();
        assert!(!red.paused);
        assert!(blue.paused);
        assert_eq!(blue.remaining, 0.0);
    }

    #[test]
    fn koth_restart_restores_both_clocks_and_preserves_round_score() {
        let mut rules = koth();
        rules.apply_input(0, b"SetRedTimer", 4, 0.0);
        rules.apply_input(0, b"AddBlueTimer", -50, 0.0);
        assert_eq!(
            rules.koth_timers().unwrap().map(|timer| timer.remaining),
            [4.0, 130.0]
        );
        rules.win(PlayerTeam::Red, 1).unwrap();
        rules.advance(15.015, 0.015, facts()).unwrap();
        let timers = rules.koth_timers().unwrap();
        assert_eq!(timers.map(|timer| timer.remaining), [180.0, 180.0]);
        assert!(timers.iter().all(|timer| timer.paused));
        assert_eq!(rules.snapshot(vec![]).red_score, 1);
    }

    #[test]
    fn absolute_timer_deadline_does_not_accumulate_sixty_six_hz_subtraction_error() {
        let mut rules = koth();
        rules.apply_input(99, b"SetRedKothClockActive", 0, 0.0);
        for tick in 1..=12_000 {
            rules.advance(tick as f32 * 0.015, 0.015, facts()).unwrap();
        }
        assert_eq!(rules.koth_timers().unwrap()[0].remaining, 0.0);
        assert_eq!(rules.koth_timers().unwrap()[1].remaining, 180.0);
    }

    #[test]
    fn timer_threshold_output_is_single_shot_and_add_time_rearms_it() {
        let mut rules = koth();
        rules.apply_input(0, b"SetRedTimer", 31, 0.0);
        rules.apply_input(99, b"SetRedKothClockActive", 0, 0.0);
        let threshold = Event::TimerThreshold {
            timer: 2,
            seconds: 30,
        };
        assert!(
            rules
                .advance(1.0, 0.015, facts())
                .unwrap()
                .contains(&threshold)
        );
        assert!(
            !rules
                .advance(1.06, 0.015, facts())
                .unwrap()
                .contains(&threshold)
        );
        rules.apply_input(0, b"AddRedTimer", 10, 1.06);
        assert!(
            rules
                .advance(11.06, 0.015, facts())
                .unwrap()
                .contains(&threshold)
        );
    }

    #[test]
    fn time_added_event_preserves_signed_seconds_and_disabled_inputs_do_not_publish() {
        let mut rules = koth();
        rules.apply_input(0, b"AddRedTimer", -100_000, 0.0);
        assert_eq!(
            rules.take_events(),
            [Event::TimerTimeAdded {
                timer: 2,
                seconds: -100_000
            }]
        );
        assert_eq!(rules.koth_timers().unwrap()[0].remaining, 0.0);
        rules.apply_input(2, b"Disable", 0, 0.0);
        rules.apply_input(0, b"AddRedTimer", 10, 0.0);
        assert!(rules.take_events().is_empty());
        rules.restart(false);
        rules.apply_input(0, b"AddBlueTimer", 10, 0.0);
        assert!(
            rules.take_events().is_empty(),
            "AddTimerSeconds is inert during preround"
        );
    }

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
            timers: vec![timer()],
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
            timers: vec![TimerConfiguration {
                initial_seconds: 1,
                setup_seconds: 0,
                ..timer()
            }],
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
        rules.advance(2.0, 1.0, facts()).unwrap();
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
    fn configured_jump_timer_preserves_source_signed_atoi_overflow() {
        let graph = playsrc_entity::parse(
            b"{\"classname\"\"team_round_timer\"\"timer_length\"\"9999999999999999999999\"\"show_in_hud\"\"0\"}\0",
            playsrc_entity::Limits::default(),
        )
        .unwrap();
        let configuration = Configuration::from_graph(&graph).unwrap();
        let timer = configuration.timers[0];
        assert_eq!(timer.initial_seconds, -1);
        assert!(!timer.show_in_hud);
    }

    #[test]
    fn authored_upward_timer_configuration_is_parsed_without_cart_simulation() {
        let graph = playsrc_entity::parse(b"{\"classname\"\"team_round_timer\"\"timer_length\"\"330\"\"setup_length\"\"70\"\"max_length\"\"600\"\"show_in_hud\"\"1\"\"auto_countdown\"\"1\"\"start_paused\"\"0\"}\n{\"classname\"\"team_train_watcher\"}\0", playsrc_entity::Limits::default()).unwrap();
        let configuration = Configuration::from_graph(&graph).unwrap();
        assert_eq!(
            configuration.timers[0],
            TimerConfiguration {
                identity: 0,
                reset_on_round_start: false,
                ..timer()
            }
        );
        assert_eq!(configuration.defending_team, Some(PlayerTeam::Red));
    }
}
