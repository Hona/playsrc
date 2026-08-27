//! Opt-in, bounded, pointer-free evidence of the authoritative gameplay owner.
//! The checkpoint is the deterministic compiled-map initial state, identified by
//! its BSP/world hashes. The journal includes every subsequent state mutation,
//! the admitted host commands, and hashes of complete tick/event publications.
use super::*;
use playsrc_simulation::MetricsClock;

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_RECORDS: usize = 16_384;
static ACTIVE: AtomicU32 = AtomicU32::new(0);

struct Journal {
    handle: u32,
    bytes: Vec<u8>,
    records: usize,
    overflow: bool,
    observing: bool,
    last_attack: Option<AttackAdmission>,
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
        append_record(
            handle,
            2,
            &[
                &host_tick.to_le_bytes(),
                &elapsed.to_le_bytes(),
                &Sha256::digest(output),
                &(command.len() as u32).to_le_bytes(),
                command,
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
pub(super) fn mutation(handle: u32, kind: u32, bytes: &[u8]) {
    append(handle, kind, &[bytes]);
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
        let mut bytes = b"PGRP".to_vec();
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&slot.bsp_hash);
        bytes.extend_from_slice(&slot.collision.as_ref()?.identity);
        bytes.extend_from_slice(&0_u64.to_le_bytes());
        bytes.extend_from_slice(&slot.collision_revision.to_le_bytes());
        Some(bytes)
    })
    .flatten() else {
        return 0;
    };
    *journal().lock().expect("gameplay replay") = Some(Journal {
        handle,
        bytes: header,
        records: 0,
        overflow: false,
        observing: false,
        last_attack: None,
    });
    ACTIVE.store(handle, Ordering::Relaxed);
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
    if ACTIVE.load(Ordering::Relaxed) == handle {
        playsrc_gameplay_replay_stop(handle);
    }
    let mut value = journal().lock().expect("gameplay replay");
    if value.as_ref().is_some_and(|value| value.handle == handle) {
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
mod tests {
    use super::*;
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
        let handle = 0xffff_ffff;
        let setup = |bytes| {
            *journal().lock().unwrap() = Some(Journal {
                handle,
                bytes,
                records: 0,
                overflow: false,
                observing: false,
                last_attack: None,
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
