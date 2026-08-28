use std::{alloc::{GlobalAlloc, Layout, System}, cell::Cell, hint::black_box};
use playsrc_collision::Hull;
use playsrc_entity::{Graph, ModelBounds, Transform, Variant};
use playsrc_movement::{Error as MoveError, Trace, Tracer};
use playsrc_tf2::{bot, Command, GameplayWorld, MapRuntime, MoverResult, MoverResultKind, PlayerTeam, Session};

#[derive(Clone, Copy, Debug, Default)]
struct Metrics { requests: usize, bytes: usize, live: isize, peak: isize }
thread_local! {
    static METRICS: Cell<Metrics> = const { Cell::new(Metrics { requests: 0, bytes: 0, live: 0, peak: 0 }) };
    static PEAKS: Cell<[isize; 8]> = const { Cell::new([0; 8]) };
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}
struct Allocator;
fn record(allocated: usize, freed: usize, request: bool) {
    let _ = METRICS.try_with(|cell| {
        let mut m = cell.get();
        m.requests += usize::from(request); m.bytes += allocated;
        m.live += allocated as isize - freed as isize; m.peak = m.peak.max(m.live);
        cell.set(m);
        let _ = PEAKS.try_with(|cell| {
            let mut peaks = cell.get();
            for peak in &mut peaks[..DEPTH.get()] { *peak = (*peak).max(m.live); }
            cell.set(peaks);
        });
    });
}
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) }; if !p.is_null() { record(l.size(), 0, true); } p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) }; if !p.is_null() { record(l.size(), 0, true); } p
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(p, l, size) }; if !p.is_null() { record(size, l.size(), true); } p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) { unsafe { System.dealloc(p, l) }; record(0, l.size(), false); }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn measure<T>(f: impl FnOnce() -> T) -> (T, Metrics) {
    let before = METRICS.get(); let depth = DEPTH.get();
    let mut peaks = PEAKS.get(); peaks[depth] = before.live; PEAKS.set(peaks); DEPTH.set(depth + 1);
    struct Reset(usize);
    impl Drop for Reset { fn drop(&mut self) { DEPTH.set(self.0); } }
    let _reset = Reset(depth); let value = f(); let after = METRICS.get();
    (value, Metrics { requests: after.requests - before.requests, bytes: after.bytes - before.bytes,
        live: after.live - before.live, peak: PEAKS.get()[depth] - before.live })
}

#[derive(Clone)]
struct Floor;
impl Tracer for Floor {
    fn trace(&self, start: [f32; 3], end: [f32; 3], hull: Hull, _: u32) -> Result<Trace, MoveError> {
        let floor = -hull.mins[2];
        let fraction = if end[2] < floor { ((start[2] - floor) / (start[2] - end[2])).clamp(0.0, 1.0) } else { 1.0 };
        Ok(Trace { fraction, start_solid: false, all_solid: false,
            end: if fraction < 1.0 { [start[0] + (end[0] - start[0]) * fraction, start[1] + (end[1] - start[1]) * fraction, floor] } else { end },
            normal: (fraction < 1.0).then_some([0.0, 0.0, 1.0]), hit: (fraction < 1.0).then_some(0), contents: u32::from(fraction < 1.0) })
    }
}
impl GameplayWorld for Floor {
    fn overlaps_model_hull(&self, model: usize, _: [f32; 3], position: [f32; 3], _: Hull) -> Result<bool, MoveError> {
        Ok(model == 1 && position[0] < 24.0)
    }
}
fn mesh() -> playsrc_nav::Mesh {
    let mut bytes = Vec::new();
    bytes.extend(playsrc_nav::MAGIC.to_le_bytes()); bytes.extend(16_u32.to_le_bytes());
    bytes.extend(2_u32.to_le_bytes()); bytes.extend(128_u32.to_le_bytes()); bytes.push(1);
    bytes.extend(0_u16.to_le_bytes()); bytes.push(1); bytes.extend(3_u32.to_le_bytes());
    for identity in 1_u32..=3 {
        let x = (identity - 1) as f32 * 100.0;
        bytes.extend(identity.to_le_bytes()); bytes.extend(0_u32.to_le_bytes());
        for value in [x, 0.0, 0.0, x + 100.0, 100.0, 0.0, 0.0, 0.0] { bytes.extend(value.to_le_bytes()); }
        for direction in 0..4 {
            let target = match (direction, identity) { (1, 1 | 2) => Some(identity + 1), (3, 2 | 3) => Some(identity - 1), _ => None };
            bytes.extend(u32::from(target.is_some()).to_le_bytes()); if let Some(target) = target { bytes.extend(target.to_le_bytes()); }
        }
        bytes.push(0); bytes.extend(0_u32.to_le_bytes()); bytes.extend(0_u16.to_le_bytes());
        bytes.extend([0; 8 + 8 + 16 + 8 + 4]);
    }
    bytes.extend(0_u32.to_le_bytes());
    playsrc_nav::parse(&bytes, playsrc_nav::Profile::TeamFortress2, Some(128), playsrc_nav::Limits::default()).unwrap()
}
fn graph() -> Graph {
    playsrc_entity::parse(br#"
        {"classname" "info_player_teamspawn" "TeamNum" "2" "origin" "10 50 1"}
        {"classname" "info_player_teamspawn" "TeamNum" "3" "origin" "250 50 1"}
        {"classname" "func_movelinear" "targetname" "mover" "MoveDistance" "32" "speed" "50" "OnFullyOpen" "mover,Close,,0,-1"}
        {"classname" "trigger_multiple" "targetname" "zone" "model" "*1" "spawnflags" "1"}
        {"classname" "team_train_watcher" "train" "cart" "start_node" "first"}
        {"classname" "func_tracktrain" "targetname" "cart" "origin" "150 50 1"}
        {"classname" "path_track" "targetname" "first" "target" "second" "origin" "150 50 1"}
        {"classname" "path_track" "targetname" "second" "origin" "200 50 1"}
    "#, playsrc_entity::Limits::default()).unwrap()
}
fn fixture(roster: u8) -> Session<Floor> {
    let graph = graph();
    let map = MapRuntime::compile(&graph, 0.015, 1, vec![ModelBounds { model: 1, mins: [-24.0; 3], maxs: [24.0; 3] }]).unwrap();
    let mut session = Session::new(Floor, [10.0, 50.0, 1.0], map);
    session.configure_navigation(mesh(), &graph).unwrap();
    if roster != 0 {
        session.advance(Command { bot_request: Some(bot::Request { operation: bot::Operation::Add, count: roster,
            class: Some(playsrc_tf2::PlayerClass::Soldier), team: Some(PlayerTeam::Red), difficulty: if roster == 15 { bot::Difficulty::Easy } else { bot::Difficulty::Normal } }), ..Command::default() }).unwrap();
    } else {
        session.advance(Command::default()).unwrap();
    }
    session
}
fn assert_same(a: &Session<Floor>, b: &Session<Floor>) {
    assert_eq!(a.producer_snapshot(), b.producer_snapshot());
    assert_eq!(a.movement_snapshot_bytes(), b.movement_snapshot_bytes());
    assert_eq!(a.random_state(), b.random_state());
    assert_eq!(a.entity_revision(), b.entity_revision());
    assert_eq!(a.bot_world().unwrap().snapshots(), b.bot_world().unwrap().snapshots());
}
fn run(roster: u8) -> (Metrics, Metrics) {
    let mut session = fixture(roster);
    assert_eq!(session.bot_world().unwrap().snapshots().len(), usize::from(roster));
    let (_, copy) = measure(|| { black_box(session.clone()); });
    let mut total = Metrics::default();
    for tick in 0..120 {
        session.set_position([if tick % 2 == 0 { 10.0 } else { 50.0 }, 50.0, 1.0]).unwrap();
        session.advance(Command::default()).unwrap();
        session.map_input(2, b"Open", Variant::Void).unwrap();
        let request = session.mover_requests()[0];
        let result = MoverResult { request_id: request.request_id, entity: request.entity,
            kind: MoverResultKind::Progress, transform: Transform { origin: request.start, angles: request.start_angles }, carry: [0.0, 0.125, 0.0] };
        let checkpoint = session.clone();
        let mut reference = checkpoint.clone();
        let expected = reference.apply_mover_results(&[result]).unwrap();
        let (actual, m) = measure(|| session.apply_mover_results(&[result]).unwrap());
        assert_eq!(actual, expected); assert_same(&session, &reference);
        assert_eq!(session.movement_state().position[1], checkpoint.movement_state().position[1] + 0.125);
        total.requests += m.requests; total.bytes += m.bytes; total.peak = total.peak.max(m.peak);

        // Completion mutates the map's active mover table before the second
        // record fails. Both the map and every unrelated producer must roll back.
        let completed = MoverResult { kind: MoverResultKind::Completed, transform: Transform { origin: request.destination, angles: request.destination_angles }, ..result };
        for invalid in [completed, MoverResult { entity: u32::MAX, ..result },
            MoverResult { entity: 3, request_id: u64::MAX, ..result },
            MoverResult { entity: 3, carry: [f32::NAN, 0.0, 0.0], ..result }] {
            assert!(session.apply_mover_results(&[completed, invalid]).is_err());
            assert_same(&session, &reference);
        }
        for kind in [MoverResultKind::BlockedStart, MoverResultKind::BlockedStay, MoverResultKind::BlockedEnd] {
            let blocked = MoverResult { kind, ..result };
            assert_eq!(session.apply_mover_results(&[blocked]).unwrap(), reference.apply_mover_results(&[blocked]).unwrap());
            assert_same(&session, &reference);
        }
        let phase = session.apply_mover_results(&[completed]).unwrap();
        assert_eq!(phase, reference.apply_mover_results(&[completed]).unwrap());
        assert_same(&session, &reference);
        assert!(phase.mover_requests.iter().any(|r| r.entity == request.entity && r.request_id != request.request_id && !r.opening));
        assert!(session.apply_mover_results(&[result]).is_err(), "superseded record rejected");
        let closing = session.mover_requests()[0];
        session.apply_mover_results(&[MoverResult { request_id: closing.request_id, entity: closing.entity,
            kind: MoverResultKind::Completed, transform: Transform { origin: closing.destination, angles: closing.destination_angles }, carry: [0.0; 3] }]).unwrap();
    }
    (total, copy)
}
#[test]
fn mover_checkpoint_does_not_copy_unrelated_bot_state() {
    // Initialize process-lifetime schema owners outside fixture lifetime gauges.
    drop(fixture(0));
    let ((empty, empty_clone), empty_life) = measure(|| run(0));
    let ((fifteen, fifteen_clone), fifteen_life) = measure(|| run(15));
    let ((full, full_clone), full_life) = measure(|| run(23));
    println!("120 mover/contact phases: 0={empty:?}, 15={fifteen:?}, 23={full:?}");
    println!("isolated Session clones: 0={empty_clone:?}, 15={fifteen_clone:?}, 23={full_clone:?}");
    println!("whole lifetimes: 0={empty_life:?}, 15={fifteen_life:?}, 23={full_life:?}");
    assert_eq!(empty_life.live, 0); assert_eq!(fifteen_life.live, 0); assert_eq!(full_life.live, 0);
    assert!(full_clone.requests > empty_clone.requests);
    // Contacts legitimately retain actor-dependent map state. Its checkpoint is
    // still necessary; bound the slope well below the isolated unrelated copy,
    // rather than asserting that every actor-dependent allocation disappears.
    for (phase, copy) in [(fifteen, fifteen_clone), (full, full_clone)] {
        assert!(phase.requests - empty.requests < (copy.requests - empty_clone.requests) * 120 / 10);
        assert!(phase.bytes - empty.bytes < (copy.bytes - empty_clone.bytes) * 120 / 10);
        assert!(phase.peak < copy.peak / 10);
    }
}
