//! Opt-in diagnostic edges; never part of simulation state or random ordering.
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub tick: u64,
    pub stage: u32,
    pub actor: u32,
    pub value: u64,
}

thread_local! {
    static OBSERVER: Cell<Option<fn(Event)>> = const { Cell::new(None) };
    static TICK: Cell<u64> = const { Cell::new(0) };
}

pub fn set_observer(observer: Option<fn(Event)>) {
    OBSERVER.set(observer);
}

pub fn begin_tick(tick: u64) {
    TICK.set(tick);
}

pub fn emit(stage: u32, actor: u32) {
    emit_value(stage, actor, 0);
}

pub fn emit_value(stage: u32, actor: u32, value: u64) {
    if let Some(observer) = OBSERVER.get() {
        observer(Event {
            tick: TICK.get(),
            stage,
            actor,
            value,
        });
    }
}

pub const QUOTA: u32 = 1;
pub const REQUEST: u32 = 2;
pub const LOADOUT: u32 = 3;
pub const NAVIGATION: u32 = 4;
pub const CONSTRUCTED: u32 = 5;
pub const ROSTER: u32 = 6;
pub const RESPAWN: u32 = 7;
pub const SNAPSHOT_ENCODED: u32 = 8;
pub const TRANSACTION: u32 = 9;
pub const CLONED: u32 = 10;
pub const COLLISION: u32 = 11;
pub const ADVANCED: u32 = 12;
pub const ENCODE: u32 = 13;
pub const PUBLISHED: u32 = 14;
pub const ROLLBACK: u32 = 15;
