//! Opt-in, bounded, pointer-free evidence of the authoritative gameplay owner.
//! The checkpoint is the deterministic compiled-map initial state, identified by
//! its BSP/world hashes. The journal includes every subsequent state mutation,
//! the admitted host commands, and hashes of complete tick/event publications.
use super::*;
use playsrc_simulation::MetricsClock;

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_RECORDS: usize = 16_384;
#[derive(Clone, Copy)]
pub(super) enum Mutation {
    Team = 4,
    Position = 5,
    Course = 6,
    Equipment = 9,
    EntityInput = 10,
}
static ACTIVE: AtomicU32 = AtomicU32::new(0);
static CURRENT_GAME: AtomicU32 = AtomicU32::new(0);
static CLOCK_OWNER: AtomicU32 = AtomicU32::new(0);

struct ClockInput { handle:u32, values:Vec<f64>, cursor:usize }
fn clock_input()->&'static Mutex<Option<ClockInput>>{
    static VALUE:OnceLock<Mutex<Option<ClockInput>>>=OnceLock::new();VALUE.get_or_init(||Mutex::new(None))
}
pub(super) struct GameScope(u32);
pub(super) fn game_scope(handle:u32)->GameScope{GameScope(CURRENT_GAME.swap(handle,Ordering::Relaxed))}
impl Drop for GameScope{fn drop(&mut self){CURRENT_GAME.store(self.0,Ordering::Relaxed);}}

/// A replay supplies the recorded work-clock inputs, not a different simulation
/// cadence or a synthetic performance measurement clock.
pub(super) fn work_clock(actual:f64)->f64{
    let handle=CURRENT_GAME.load(Ordering::Relaxed);
    let value=if handle!=0&&CLOCK_OWNER.load(Ordering::Relaxed)==handle{
        let mut input=clock_input().lock().expect("replay clock");
        let input=input.as_mut().expect("replay clock owner");
        let value=input.values.get(input.cursor).copied().unwrap_or(f64::NAN);
        input.cursor=input.cursor.saturating_add(1);value
    }else{actual};
    if handle!=0&&ACTIVE.load(Ordering::Relaxed)==handle{
        let mut entry=journal().lock().expect("gameplay replay");
        if let Some(entry)=entry.as_mut().filter(|entry|entry.handle==handle&&entry.clock_collecting){
            if entry.clock_samples.len()==4096||!value.is_finite()||value<0.0{entry.overflow=true;}
            else{entry.clock_samples.push(value);}
        }
    }
    value
}

#[unsafe(no_mangle)]
/// # Safety
/// `pointer` identifies `length` readable bytes containing little-endian f64
/// work-clock inputs from an authenticated replay. No caller view is retained.
pub unsafe extern "C" fn playsrc_gameplay_replay_clock_input(handle:u32,pointer:*const u8,length:usize)->u32{
    if pointer.is_null()||length>MAX_BYTES||length%8!=0||CLOCK_OWNER.load(Ordering::Relaxed)!=0
        ||with(handle,|slot|slot.session.as_ref().is_some_and(|session|session.tick()==0)).unwrap_or(false)==false{return 0;}
    let values=unsafe{std::slice::from_raw_parts(pointer,length)}.chunks_exact(8).map(|bytes|f64::from_le_bytes(bytes.try_into().unwrap())).collect::<Vec<_>>();
    if values.iter().any(|value|!value.is_finite()||*value<0.0)||values.windows(2).any(|pair|pair[1]<pair[0]){return 0;}
    *clock_input().lock().expect("replay clock")=Some(ClockInput{handle,values,cursor:0});CLOCK_OWNER.store(handle,Ordering::Relaxed);1
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_gameplay_replay_clock_remaining(handle:u32)->u32{
    clock_input().lock().expect("replay clock").as_ref().filter(|input|input.handle==handle)
        .map_or(u32::MAX,|input|input.values.len().checked_sub(input.cursor).map_or(u32::MAX,|remaining|remaining as u32))
}

struct Journal {
    handle: u32,
    bytes: Vec<u8>,
    records: usize,
    overflow: bool,
    observing: bool,
    last_attack: Option<AttackAdmission>,
    clock_samples:Vec<f64>,
    clock_collecting:bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct AttackAdmission {
    host_tick: u64,
    player: u32,
}
fn attack_admission(host_tick: u64, command: &[u8], snapshot: &[u8]) -> Option<AttackAdmission> {
    // PCMD button flags and the PSSN player header are the same public wire
    // records already retained by this owner. Do not infer admission from a DOM
    // mouse event, an observe with zero selected ticks, or weapon presentation.
    let flags = u32::from_le_bytes(command.get(28..32)?.try_into().ok()?);
    if flags & (1 << 3) == 0 {
        return None;
    }
    Some(AttackAdmission {
        host_tick,
        player: u32::from(*snapshot.get(16)?)
            | (u32::from(*snapshot.get(18)?) << 8)
            | (u32::from(*snapshot.get(28)?) << 16),
    })
}
fn journal() -> &'static Mutex<Option<Journal>> {
    static VALUE: OnceLock<Mutex<Option<Journal>>> = OnceLock::new();
    VALUE.get_or_init(|| Mutex::new(None))
}
fn append(handle: u32, kind: u32, parts: &[&[u8]]) {
    append_record(handle, kind, parts, None);
}
fn append_record(handle: u32, kind: u32, parts: &[&[u8]], attack: Option<AttackAdmission>) {
    if ACTIVE.load(Ordering::Relaxed) != handle {
        return;
    }
    let mut value = journal().lock().expect("gameplay replay");
    let Some(value) = value.as_mut().filter(|value| value.handle == handle) else {
        return;
    };
    let size = parts
        .iter()
        .try_fold(8_usize, |sum, part| sum.checked_add(part.len()));
    if value.overflow
        || value.records == MAX_RECORDS
        || size.is_none_or(|size| size > MAX_BYTES.saturating_sub(16 + value.bytes.len()))
    {
        value.overflow = true;
        return;
    }
    value
        .bytes
        .extend_from_slice(&(size.unwrap() as u32).to_le_bytes());
    value.bytes.extend_from_slice(&kind.to_le_bytes());
    for part in parts {
        value.bytes.extend_from_slice(part);
    }
    value.records += 1;
    if let Some(attack) = attack {
        value.last_attack = Some(attack);
    }
    if kind == 1 {
        value.observing = true;
    }
    if kind == 3 {
        value.observing = false;
    }
}
pub(super) fn observe(
    handle: u32,
    now: f64,
    suspended: u32,
    acknowledged_snapshot: u64,
    command: &[u8],
) {
    append(
        handle,
        1,
        &[
            &now.to_le_bytes(),
            &suspended.to_le_bytes(),
            &acknowledged_snapshot.to_le_bytes(),
            &(command.len() as u32).to_le_bytes(),
            command,
        ],
    );
}
pub(super) fn published(handle: u32, output: &[u8]) {
    if ACTIVE.load(Ordering::Relaxed) == handle {
        append(handle, 3, &[&Sha256::digest(output)]);
    }
}
pub(super) fn tick_started(handle: u32) -> Option<(RuntimeMetricsClock, u64)> {
    (ACTIVE.load(Ordering::Relaxed) == handle).then(|| {
        if let Some(entry)=journal().lock().expect("gameplay replay").as_mut(){entry.clock_samples.clear();entry.clock_collecting=true;}
        let mut clock = RuntimeMetricsClock::new();
        let now = clock.monotonic_nanoseconds();
        (clock, now)
    })
}
pub(super) fn tick(
    handle: u32,
    host_tick: u64,
    command: &[u8],
    output: &[u8],
    started: Option<(RuntimeMetricsClock, u64)>,
) {
    if let Some((mut clock, started)) = started {
        let elapsed = clock.monotonic_nanoseconds().saturating_sub(started);
        let samples=journal().lock().expect("gameplay replay").as_mut().map_or_else(Vec::new,|entry|{entry.clock_collecting=false;std::mem::take(&mut entry.clock_samples)});
        let clock_bytes=samples.iter().flat_map(|value|value.to_le_bytes()).collect::<Vec<_>>();
        append_record(
            handle,
            2,
            &[
                &host_tick.to_le_bytes(),
                &elapsed.to_le_bytes(),
                &Sha256::digest(output),
                &(command.len() as u32).to_le_bytes(),
                &(samples.len() as u32).to_le_bytes(),
                command,
                &clock_bytes,
            ],
            attack_admission(host_tick, command, output),
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_gameplay_replay_attack_tick(handle: u32) -> u64 {
    journal()
        .lock()
        .expect("gameplay replay")
        .as_ref()
        .filter(|value| value.handle == handle && !value.overflow)
        .and_then(|value| value.last_attack)
        .map_or(0, |attack| attack.host_tick)
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_gameplay_replay_attack_player(handle: u32) -> u32 {
    journal()
        .lock()
        .expect("gameplay replay")
        .as_ref()
        .filter(|value| value.handle == handle && !value.overflow)
        .and_then(|value| value.last_attack)
        .map_or(0, |attack| attack.player)
}
pub(super) fn mutation(handle: u32, kind: Mutation, bytes: &[u8]) {
    append(handle, kind as u32, &[bytes]);
}

// Local equipment updates affect every live map, even when called with handle
// zero. Record them against the active journal's generation, not that API handle.
pub(super) fn local_equipment_mutation(bytes: &[u8]) {
    let handle = ACTIVE.load(Ordering::Relaxed);
    if handle != 0 { mutation(handle, Mutation::Equipment, bytes); }
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_gameplay_replay_begin(handle: u32) -> u32 {
    if ACTIVE.load(Ordering::Relaxed) != 0
        || simulation_hosts()
            .lock()
            .expect("hosts")
            .contains_key(&handle)
    {
        return 0;
    }
    let Some(header) = with(handle, |slot| {
        let session = slot.session.as_ref()?;
        if session.producer_snapshot().tick != 0 || slot.collision_revision != 1 {
            return None;
        }
        let mut bytes = Vec::with_capacity(1024);
        bytes.extend_from_slice(b"PGRP");
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&slot.bsp_hash);
        bytes.extend_from_slice(&slot.collision.as_ref()?.identity);
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&slot.collision_revision.to_le_bytes());
        // Persisted local loadout is independent of the authenticated resource
        // graph and must be restored before reconstructing the compiled session.
        bytes.extend_from_slice(&local_equipment().lock().expect("local equipment").persist());
        Some(bytes)
    })
    .flatten() else {
        return 0;
    };
    {
        let Some((index, generation)) = decode(handle) else { return 0; };
        let mut entries = slots().lock().expect("slots");
        let Some(slot) = entries.get_mut(index).filter(|slot| slot.generation == generation) else { return 0; };
        if let Some((_, state)) = &mut slot.map_particles { state.record_entropy(); }
    }
    *journal().lock().expect("gameplay replay") = Some(Journal {
        handle,
        bytes: header,
        records: 0,
        overflow: false,
        observing: false,
        last_attack: None,
        clock_samples:Vec::new(),clock_collecting:false,
    });
    ACTIVE.store(handle, Ordering::Relaxed);
    admission_metrics::begin();
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_gameplay_replay_mark(handle: u32, mark: u32) -> u32 {
    if ACTIVE.load(Ordering::Relaxed) != handle || mark > 1 {
        return 0;
    }
    append(handle, 7, &[&mark.to_le_bytes()]);
    1
}

/// A workload author may retain a live, unmeasured command tail after sampling.
/// Stop only the phase recorder at its existing boundary; keep every command
/// and complete state hash in the journal until its separate stop operation.
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_gameplay_replay_stop_admission(handle: u32) -> u32 {
    if handle == 0 || ACTIVE.load(Ordering::Relaxed) != handle { return 0; }
    admission_metrics::stop();
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn playsrc_map_particle_entropy_length(handle: u32) -> usize {
    with(handle, |slot| slot.map_particles.as_ref().map_or(12, |(_, state)| state.entropy_bytes().len())).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn playsrc_map_particle_entropy_copy(handle: u32, pointer: *mut u8, capacity: usize) -> usize {
    with(handle, |slot| {
        let bytes = slot.map_particles.as_ref().map_or_else(|| b"MPER\x01\0\0\0\0\0\0\0".to_vec(), |(_, state)| state.entropy_bytes());
        if bytes.len() > capacity { return 0; }
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len()); }
        bytes.len()
    }).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn playsrc_map_particle_entropy_restore(handle: u32, pointer: *const u8, length: usize) -> u32 {
    if length < 12 || length > 4 * 1024 * 1024 || ACTIVE.load(Ordering::Relaxed) != handle { return 0; }
    let bytes = unsafe { std::slice::from_raw_parts(pointer, length) };
    let Some((index, generation)) = decode(handle) else { return 0; };
    let mut entries = slots().lock().expect("slots");
    let Some(slot) = entries.get_mut(index).filter(|slot| slot.generation == generation) else { return 0; };
    match &mut slot.map_particles {
        Some((_, state)) => u32::from(state.restore_entropy(bytes).is_ok()),
        None => u32::from(bytes == b"MPER\x01\0\0\0\0\0\0\0"),
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_gameplay_replay_stop(handle: u32) -> u32 {
    if ACTIVE
        .compare_exchange(handle, 0, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return 0;
    }
    let mut value = journal().lock().expect("gameplay replay");
    let Some(value) = value.as_mut().filter(|value| value.handle == handle) else {
        return 0;
    };
    admission_metrics::stop();
    let complete = !value.overflow && !value.observing;
    value.bytes.extend_from_slice(&12_u32.to_le_bytes());
    value.bytes.extend_from_slice(&8_u32.to_le_bytes());
    value
        .bytes
        .extend_from_slice(&u32::from(complete).to_le_bytes());
    u32::from(complete)
}
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_gameplay_replay_length(handle: u32) -> usize {
    journal()
        .lock()
        .expect("gameplay replay")
        .as_ref()
        .filter(|value| value.handle == handle)
        .map_or(0, |value| value.bytes.len())
}
#[unsafe(no_mangle)]
/// # Safety
/// `pointer` must identify `capacity` writable bytes. No module view is retained.
pub unsafe extern "C" fn playsrc_gameplay_replay_copy(
    handle: u32,
    offset: usize,
    pointer: *mut u8,
    capacity: usize,
) -> usize {
    if pointer.is_null() {
        return 0;
    }
    let value = journal().lock().expect("gameplay replay");
    let Some(bytes) = value
        .as_ref()
        .filter(|value| value.handle == handle)
        .and_then(|value| value.bytes.get(offset..))
    else {
        return 0;
    };
    let count = bytes.len().min(capacity);
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, count);
    }
    count
}
pub(super) fn dispose(handle: u32) {
    if CLOCK_OWNER.load(Ordering::Relaxed)==handle{*clock_input().lock().expect("replay clock")=None;CLOCK_OWNER.store(0,Ordering::Relaxed);}
    if ACTIVE.load(Ordering::Relaxed) == handle {
        playsrc_gameplay_replay_stop(handle);
    }
    let mut value = journal().lock().expect("gameplay replay");
    if value.as_ref().is_some_and(|value| value.handle == handle) {
        admission_metrics::dispose();
        *value = None;
    }
}

#[cfg(feature = "collision-replay")]
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_collision_replay_mode(reference: u32) -> u32 {
    if reference > 2 {
        return 0;
    }
    playsrc_collision::replay_diagnostics::select_reference(reference == 1);
    playsrc_collision::replay_diagnostics::select_displacement_reference(reference == 2);
    1
}
#[cfg(feature = "collision-replay")]
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_collision_replay_reset() {
    playsrc_collision::replay_diagnostics::reset();
}
#[cfg(feature = "collision-replay")]
#[unsafe(no_mangle)]
pub extern "C" fn playsrc_collision_replay_counter(index: u32) -> f64 {
    playsrc_collision::replay_diagnostics::counters()
        .get(index as usize)
        .copied()
        .unwrap_or(0) as f64
}

#[cfg(test)]
pub(super) mod tests {
    #[test]
    fn playback_clock_consumes_owned_inputs_without_replacing_the_metrics_clock(){
        use super::*;
        let _metrics=memory::TEST_METRICS.lock().expect("test metrics");
        let _slots=slots().lock().expect("slots");
        let handle=u32::MAX-1;
        *clock_input().lock().unwrap()=Some(ClockInput{handle,values:vec![10.0,10.25,27.0],cursor:0});
        CLOCK_OWNER.store(handle,Ordering::Relaxed);
        {
            let _scope=game_scope(handle);
            assert_eq!(work_clock(900.0),10.0);assert_eq!(work_clock(1.0),10.25);
            assert_eq!(playsrc_gameplay_replay_clock_remaining(handle),1);
            assert_eq!(work_clock(0.0),27.0);assert_eq!(playsrc_gameplay_replay_clock_remaining(handle),0);
            assert!(work_clock(800.0).is_nan());assert_eq!(playsrc_gameplay_replay_clock_remaining(handle),u32::MAX);
        }
        assert_eq!(work_clock(42.0),42.0);dispose(handle);assert_eq!(CLOCK_OWNER.load(Ordering::Relaxed),0);
    }
    use super::*;

    pub(crate) fn assert_mutations(handle: u32) {
        let _metrics = memory::TEST_METRICS.lock().expect("test metrics");
        let initial_equipment = local_equipment().lock().unwrap().persist();
        let graph = playsrc_entity::parse(br#"
            {"classname" "info_player_teamspawn" "TeamNum" "2" "origin" "0 0 0"}
            {"classname" "func_movelinear" "targetname" "door" "MoveDistance" "32" "speed" "50"}
            {"classname" "trigger_multiple" "targetname" "start" "model" "*1" "spawnflags" "1"}
            {"classname" "trigger_multiple" "targetname" "end" "model" "*2" "spawnflags" "1"}
            {"classname" "team_train_watcher" "train" "cart" "start_node" "first"}
            {"classname" "func_tracktrain" "targetname" "cart" "origin" "50 50 1"}
            {"classname" "path_track" "targetname" "first" "target" "second" "origin" "50 50 1"}
            {"classname" "path_track" "targetname" "second" "origin" "80 50 1"}
        "#, playsrc_entity::Limits::default()).unwrap();
        let map = playsrc_tf2::MapRuntime::compile(&graph, 0.015, 1, vec![
            playsrc_entity::ModelBounds { model: 1, mins: [-24.0; 3], maxs: [24.0; 3] },
            playsrc_entity::ModelBounds { model: 2, mins: [100.0; 3], maxs: [124.0; 3] },
        ]).unwrap();
        let world = Arc::new(playsrc_collision::World::empty());
        let collision = playsrc_collision::Snapshot::compile(&world, 1, vec![], playsrc_collision::SnapshotLimits::default()).unwrap();
        let shared = SharedWorld::new(world.clone(), collision, BTreeMap::new());
        let (index, _) = decode(handle).unwrap();
        {
            let mut slots = slots().lock().unwrap();
            slots[index].collision = Some(world);
            slots[index].collision_revision = 1;
            slots[index].session = Some(playsrc_tf2::Session::new(shared.clone(), [0.0; 3], map));
        }
        assert_eq!(playsrc_gameplay_replay_begin(handle), 1);
        assert_eq!(&journal().lock().unwrap().as_ref().unwrap().bytes[88..780], initial_equipment);
        assert_eq!(playsrc_gameplay_replay_mark(handle, 0), 1);
        assert_eq!(playsrc_team_select(handle, 2), 1);
        assert_eq!(playsrc_player_set_position(handle, 12.5, -3.0, 96.0), 1);
        let equip = [1, 3, 0, 18, 0, 0, 0];
        assert_eq!(unsafe { playsrc_equipment_update(0, equip.as_ptr(), equip.len()) }, 1);
        let mut restore = vec![0]; restore.extend_from_slice(&initial_equipment);
        assert_eq!(unsafe { playsrc_equipment_update(handle, restore.as_ptr(), restore.len()) }, 1);
        let mut entity = vec![0; 4]; entity.extend_from_slice(b"door\0Open\0");
        assert_eq!(unsafe { playsrc_entity_fire(handle, entity.as_ptr(), entity.len()) }, 1);
        let mut course = b"PJMP\x01\0\0\0".to_vec();
        course.extend_from_slice(&1_u64.to_le_bytes()); course.extend_from_slice(&[9; 32]); course.extend_from_slice(&2_u32.to_le_bytes());
        for (zone, trigger, kind) in [(1_u32, 2_u32, 1_u8), (2, 3, 3)] {
            course.extend_from_slice(&zone.to_le_bytes()); course.extend_from_slice(&trigger.to_le_bytes());
            course.extend_from_slice(&[kind, 0, 0, 0]); course.extend_from_slice(&1_u32.to_le_bytes());
        }
        assert_eq!(unsafe { playsrc_jump_configure(handle, course.as_ptr(), course.len()) }, 1);
        let before = playsrc_gameplay_replay_length(handle);
        // Stale generation, malformed mutations and mismatched BSP never enter
        // the journal or mutate the current generation's checkpoint.
        let stale = handle - (1 << 16);
        assert_eq!(playsrc_player_set_position(stale, 1.0, 2.0, 3.0), 0);
        assert_eq!(playsrc_player_set_position(handle, f32::NAN, 2.0, 3.0), 0);
        assert_eq!(unsafe { playsrc_equipment_update(stale, equip.as_ptr(), equip.len()) }, 0);
        assert_eq!(unsafe { playsrc_entity_fire(stale, entity.as_ptr(), entity.len()) }, 0);
        course[16] ^= 1;
        assert_eq!(unsafe { playsrc_jump_configure(handle, course.as_ptr(), course.len()) }, 0);
        assert_eq!(playsrc_gameplay_replay_length(handle), before);
        // Exercise the bot-only equipment entry point too. This synthetic
        // mutation-hook test is not a replay transcript acceptance sample.
        let mut nav = Vec::new();
        for value in [playsrc_nav::MAGIC, 16, 2, 128] { nav.extend_from_slice(&value.to_le_bytes()); }
        nav.push(1); nav.extend_from_slice(&0_u16.to_le_bytes()); nav.push(1); nav.extend_from_slice(&1_u32.to_le_bytes());
        nav.extend_from_slice(&1_u32.to_le_bytes()); nav.extend_from_slice(&0_u32.to_le_bytes());
        for value in [0_f32, 0.0, 0.0, 100.0, 100.0, 0.0, 0.0, 0.0] { nav.extend_from_slice(&value.to_le_bytes()); }
        nav.extend_from_slice(&[0; 16]); nav.push(0); nav.extend_from_slice(&[0; 4 + 2 + 8 + 8 + 16 + 8 + 4 + 4]);
        let mesh = playsrc_nav::parse(&nav, playsrc_nav::Profile::TeamFortress2, Some(128), playsrc_nav::Limits::default()).unwrap();
        let graph = playsrc_entity::parse(br#"
            {"classname" "info_player_teamspawn" "TeamNum" "2" "origin" "0 0 0"}
            {"classname" "team_train_watcher" "train" "cart" "start_node" "first"}
            {"classname" "func_tracktrain" "targetname" "cart" "origin" "50 50 1"}
            {"classname" "path_track" "targetname" "first" "target" "second" "origin" "50 50 1"}
            {"classname" "path_track" "targetname" "second" "origin" "80 50 1"}
        "#, playsrc_entity::Limits::default()).unwrap();
        let identity = {
            let mut slots = slots().lock().unwrap();
            let map = playsrc_tf2::MapRuntime::compile(&graph, 0.015, 1, vec![]).unwrap();
            slots[index].session = Some(playsrc_tf2::Session::new(shared, [0.0; 3], map));
            let session = slots[index].session.as_mut().unwrap();
            session.configure_navigation(mesh, &graph).unwrap();
            session.advance(playsrc_tf2::Command { bot_request: Some(playsrc_tf2::bot::Request {
                operation: playsrc_tf2::bot::Operation::Add, count: 1,
                class: Some(playsrc_tf2::PlayerClass::Soldier), team: Some(playsrc_tf2::PlayerTeam::Red),
                difficulty: playsrc_tf2::bot::Difficulty::Easy,
            }), ..playsrc_tf2::Command::default() }).unwrap().bots[0].identity
        };
        let mut bot_equip = vec![2]; bot_equip.extend_from_slice(&identity.to_le_bytes()); bot_equip.extend_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(unsafe { playsrc_equipment_update(handle, bot_equip.as_ptr(), bot_equip.len()) }, 1);
        assert_eq!(playsrc_gameplay_replay_mark(handle, 1), 1);
        let metric_length = admission_metrics::playsrc_admission_metrics_length();
        assert_eq!(playsrc_gameplay_replay_stop_admission(0), 0);
        assert_eq!(playsrc_gameplay_replay_stop_admission(handle), 1);
        playsrc_tf2::admission_metrics::emit(9, 0);
        assert_eq!(admission_metrics::playsrc_admission_metrics_length(), metric_length);
        assert_eq!(playsrc_player_set_position(handle, 12.5, -3.0, 96.0), 1);
        assert_eq!(playsrc_gameplay_replay_stop(handle), 1);
        let value = journal().lock().unwrap();
        let bytes = &value.as_ref().unwrap().bytes;
        assert_eq!(&bytes[..8], b"PGRP\x04\0\0\0");
        let mut offset = 780;
        let mut kinds = Vec::new();
        while offset < bytes.len() {
            let length = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
            kinds.push(u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()));
            offset += length;
        }
        assert_eq!(kinds, [7, 4, 5, 9, 9, 10, 6, 9, 7, 5, 8]);
        assert_eq!(offset, bytes.len());
        drop(value);
        dispose(handle);
    }
    #[test]
    fn attack_admission_is_an_actual_tick_command_not_a_weapon_shot_claim() {
        let mut command = [0_u8; 84];
        let mut snapshot = [0_u8; 184];
        snapshot[16] = 5;
        snapshot[18] = 19;
        snapshot[28] = 1;
        assert_eq!(attack_admission(24, &command, &snapshot), None);
        command[28] = 1 << 3;
        assert_eq!(
            attack_admission(24, &command, &snapshot),
            Some(AttackAdmission {
                host_tick: 24,
                player: 5 | (19 << 8) | (1 << 16),
            })
        );
        // Preserve the owner lifecycle: an input sent while dead cannot pass a
        // live attack gate, even though the command was consumed by a tick.
        snapshot[28] = 2;
        assert_eq!(
            attack_admission(25, &command, &snapshot).unwrap().player >> 16,
            2
        );
        assert_eq!(attack_admission(25, &command[..28], &snapshot), None);
        assert_eq!(attack_admission(25, &command, &snapshot[..28]), None);
    }
    #[test]
    fn incomplete_and_overflow_journals_keep_their_prefix_but_cannot_pass() {
        let _metrics = memory::TEST_METRICS.lock().expect("test metrics");
        let handle = 0xffff_ffff;
        let setup = |bytes| {
            *journal().lock().unwrap() = Some(Journal {
                handle,
                bytes,
                records: 0,
                overflow: false,
                observing: false,
                last_attack: None,
                clock_samples:Vec::new(),clock_collecting:false,
            });
            ACTIVE.store(handle, Ordering::Relaxed);
        };
        setup(b"checkpoint".to_vec());
        append(handle, 2, &[&[42; 8]]);
        let without_ack = journal().lock().unwrap().as_ref().unwrap().bytes.clone();
        setup(b"checkpoint".to_vec());
        append_record(
            handle,
            2,
            &[&[42; 8]],
            Some(AttackAdmission {
                host_tick: 17,
                player: 3 | (1 << 8) | (1 << 16),
            }),
        );
        assert_eq!(
            journal().lock().unwrap().as_ref().unwrap().bytes,
            without_ack
        );
        assert_eq!(playsrc_gameplay_replay_attack_tick(handle), 17);
        assert_eq!(
            playsrc_gameplay_replay_attack_player(handle),
            3 | (1 << 8) | (1 << 16)
        );
        assert_eq!(playsrc_gameplay_replay_attack_tick(handle - 1), 0);
        // Observing a command without a selected tick does not manufacture an acknowledgement.
        observe(handle, 1.0, 0, 0, &[8; 84]);
        assert_eq!(playsrc_gameplay_replay_attack_tick(handle), 17);
        setup(b"checkpoint".to_vec());
        observe(handle, 1.0, 0, 0, &[0; 84]);
        assert_eq!(playsrc_gameplay_replay_stop(handle), 0);
        assert!(
            journal()
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .bytes
                .starts_with(b"checkpoint")
        );
        setup(vec![0; MAX_BYTES - 16]);
        observe(handle, 2.0, 0, 0, &[0; 84]);
        assert_eq!(playsrc_gameplay_replay_stop(handle), 0);
        assert!(playsrc_gameplay_replay_length(handle) <= MAX_BYTES);
        setup(b"checkpoint".to_vec());
        observe(handle, 3.0, 0, 0, &[0; 84]);
        published(handle, &[42]);
        assert_eq!(playsrc_gameplay_replay_stop(handle), 1);
        assert_eq!(playsrc_gameplay_replay_stop(handle), 0);
        dispose(handle);
    }
}
