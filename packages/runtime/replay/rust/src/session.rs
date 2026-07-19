use std::{collections::BTreeMap, ops::Range, sync::Arc};

use crate::{
    AuthoritativeState, AuthorityAudit, Coverage, EntityIdentity, Lifecycle, MutationReport,
    PlaybackRate, PlaybackState, RecordIndexEntry, RecordedOperation, RecordedRecord, ReplayCursor,
    ReplayError, ReplayErrorCode, ReplayEvent, ReplayIdentity, ReplayLimits,
    ReplayPresentationSnapshot, ReplaySnapshot, ReplaySource, SetupValue, error,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Checkpoint {
    position_record: Option<usize>,
    state: AuthoritativeState,
}

#[derive(Clone, Debug)]
pub struct ReplaySession {
    source: ReplaySource,
    limits: ReplayLimits,
    lifecycle: Lifecycle,
    playback: PlaybackState,
    rate: PlaybackRate,
    rate_remainder_numerator: u128,
    rate_remainder_denominator: u128,
    accumulated_source_ns: u128,
    position_record: Option<usize>,
    next_record: usize,
    delivered_event_cursor: usize,
    state: Arc<AuthoritativeState>,
    index: Arc<[RecordIndexEntry]>,
    events: Arc<[ReplayEvent]>,
    checkpoints: Arc<[Checkpoint]>,
    tick_records: Arc<[usize]>,
    last_event_range: Range<usize>,
    last_discontinuity: bool,
    audit: AuthorityAudit,
}

impl ReplaySession {
    pub fn open(source: ReplaySource, limits: ReplayLimits) -> Result<Self, ReplayError> {
        validate_limits(&source, limits)?;
        if source.records.len() > limits.max_index_entries {
            return Err(limit_error(
                ReplayErrorCode::IndexLimit,
                "records",
                source.records.len(),
                limits.max_index_entries,
            ));
        }

        let mut state = AuthoritativeState::default();
        let mut index = Vec::with_capacity(source.records.len());
        let mut events = Vec::new();
        let mut checkpoints = vec![Checkpoint {
            position_record: None,
            state: state.clone(),
        }];
        let mut tick_records = Vec::new();
        let mut ready_record = None;
        let mut ready_state = None;
        let mut terminal_record = None;
        let mut ticks_since_checkpoint = 0_usize;
        let mut records_since_checkpoint = 0_usize;
        let mut prior_range_end = 0_u64;

        for (position, record) in source.records.iter().enumerate() {
            validate_record(
                record,
                position,
                prior_range_end,
                source.identity.source_bytes,
            )?;
            prior_range_end = record.source_range.end;
            if terminal_record.is_some() {
                return Err(record_error(
                    ReplayErrorCode::OperationAfterTerminal,
                    "record.after_terminal",
                    record.ordinal,
                ));
            }
            let first_event = events.len();
            let mut candidate = state.clone();
            let terminal = apply_record(&mut candidate, record, &mut events, true, limits)?;
            let setup = record
                .operations
                .iter()
                .any(|operation| matches!(operation, RecordedOperation::ReplaceSetup { .. }));
            let state_bearing = record.operations.iter().any(|operation| {
                !matches!(
                    operation,
                    RecordedOperation::IntentionallyInert | RecordedOperation::Event(_)
                )
            });
            index.push(RecordIndexEntry {
                record_ordinal: record.ordinal,
                source_range: record.source_range.clone(),
                command_tick: record.command_tick,
                server_tick: record.server_tick,
                cursor: candidate.cursor,
                first_event_ordinal: first_event,
                next_event_ordinal: events.len(),
                setup,
                state_bearing,
            });
            if record.server_tick.is_some() {
                tick_records.push(position);
                ticks_since_checkpoint += 1;
            }
            records_since_checkpoint += 1;
            if ready_record.is_none() && candidate.is_ready() {
                ready_record = Some(position);
                ready_state = Some(candidate.clone());
                checkpoints.push(Checkpoint {
                    position_record: Some(position),
                    state: candidate.clone(),
                });
                ticks_since_checkpoint = 0;
                records_since_checkpoint = 0;
            } else if ready_record.is_some()
                && (ticks_since_checkpoint >= limits.checkpoint_interval_ticks
                    || records_since_checkpoint >= limits.max_seek_records)
            {
                checkpoints.push(Checkpoint {
                    position_record: Some(position),
                    state: candidate.clone(),
                });
                ticks_since_checkpoint = 0;
                records_since_checkpoint = 0;
            }
            if checkpoints.len() > limits.max_checkpoint_count {
                return Err(limit_error(
                    ReplayErrorCode::CheckpointLimit,
                    "checkpoints.count",
                    checkpoints.len(),
                    limits.max_checkpoint_count,
                ));
            }
            if terminal {
                terminal_record = Some(position);
            }
            state = candidate;
        }

        let ready_record = ready_record
            .ok_or_else(|| error(ReplayErrorCode::MissingFullState, "replay.ready_state"))?;
        let ready_state = ready_state.expect("ready record and state are paired");
        let terminal_record = terminal_record
            .ok_or_else(|| error(ReplayErrorCode::MissingTerminal, "replay.terminal"))?;
        tick_records.retain(|record| *record >= ready_record);
        if terminal_record + 1 != source.records.len() {
            return Err(error(
                ReplayErrorCode::OperationAfterTerminal,
                "replay.terminal_position",
            ));
        }
        let index_bytes = index
            .len()
            .saturating_mul(std::mem::size_of::<RecordIndexEntry>());
        if index_bytes > limits.max_index_bytes {
            return Err(limit_error(
                ReplayErrorCode::IndexLimit,
                "index.bytes",
                index_bytes,
                limits.max_index_bytes,
            ));
        }
        let checkpoint_bytes: usize = checkpoints
            .iter()
            .map(|checkpoint| checkpoint.state.canonical_bytes().len())
            .sum();
        if checkpoint_bytes > limits.max_checkpoint_bytes {
            return Err(limit_error(
                ReplayErrorCode::CheckpointLimit,
                "checkpoints.bytes",
                checkpoint_bytes,
                limits.max_checkpoint_bytes,
            ));
        }
        let resident = estimate_resident_bytes(
            &source,
            index_bytes,
            checkpoint_bytes,
            &events,
            &ready_state,
        );
        if resident > limits.max_resident_bytes {
            return Err(limit_error(
                ReplayErrorCode::ResidentLimit,
                "replay.resident_bytes",
                resident,
                limits.max_resident_bytes,
            ));
        }
        let event_cursor = ready_state.event_cursor;
        Ok(Self {
            source,
            limits,
            lifecycle: Lifecycle::Ready,
            playback: PlaybackState::Playing,
            rate: PlaybackRate {
                numerator: 1,
                denominator: 1,
            },
            rate_remainder_numerator: 0,
            rate_remainder_denominator: 1,
            accumulated_source_ns: 0,
            position_record: Some(ready_record),
            next_record: ready_record + 1,
            delivered_event_cursor: event_cursor,
            state: Arc::new(ready_state),
            index: Arc::from(index),
            events: Arc::from(events),
            checkpoints: Arc::from(checkpoints),
            tick_records: Arc::from(tick_records),
            last_event_range: event_cursor..event_cursor,
            last_discontinuity: true,
            audit: AuthorityAudit::default(),
        })
    }

    pub fn identity(&self) -> &ReplayIdentity {
        &self.source.identity
    }

    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    pub fn playback_state(&self) -> PlaybackState {
        self.playback
    }

    pub fn rate(&self) -> PlaybackRate {
        self.rate
    }

    pub fn cursor(&self) -> ReplayCursor {
        self.state.cursor
    }

    pub fn authoritative_state(&self) -> Arc<AuthoritativeState> {
        self.state.clone()
    }

    pub fn index(&self) -> &[RecordIndexEntry] {
        &self.index
    }

    pub fn events(&self) -> &[ReplayEvent] {
        &self.events
    }

    pub fn event_range(&self, range: Range<usize>) -> Result<&[ReplayEvent], ReplayError> {
        self.events
            .get(range)
            .ok_or_else(|| error(ReplayErrorCode::MissingCursor, "events.range"))
    }

    pub fn audit(&self) -> AuthorityAudit {
        self.audit
    }

    pub fn pause(&mut self) {
        self.playback = PlaybackState::Paused;
        self.last_event_range = self.delivered_event_cursor..self.delivered_event_cursor;
    }

    pub fn resume(&mut self) {
        if self.lifecycle == Lifecycle::Ready {
            self.playback = PlaybackState::Playing;
        }
        self.last_event_range = self.delivered_event_cursor..self.delivered_event_cursor;
    }

    pub fn set_rate(&mut self, rate: PlaybackRate) -> Result<(), ReplayError> {
        if rate.numerator == 0
            || rate.denominator == 0
            || rate.numerator > self.limits.max_rate_numerator
            || rate.denominator > self.limits.max_rate_denominator
        {
            return Err(error(ReplayErrorCode::InvalidRate, "playback.rate"));
        }
        self.rate = rate;
        Ok(())
    }

    pub fn advance(&mut self, elapsed_ns: u64) -> Result<MutationReport, ReplayError> {
        if self.lifecycle == Lifecycle::Ended || self.playback == PlaybackState::Paused {
            return Ok(self.inert_report());
        }
        if self.lifecycle != Lifecycle::Ready {
            return Err(error(ReplayErrorCode::NotReady, "playback.advance"));
        }
        self.accumulate_elapsed(elapsed_ns)?;
        let first_event = self.delivered_event_cursor;
        let mut applied = 0_usize;
        loop {
            let Some(next_tick) = self.next_tick_record() else {
                if self.next_record < self.source.records.len() && elapsed_ns > 0 {
                    applied += self.apply_forward_through(self.source.records.len() - 1)?;
                }
                break;
            };
            let duration = self.duration_to_tick(next_tick)?;
            if self.accumulated_source_ns < duration {
                break;
            }
            self.accumulated_source_ns -= duration;
            let end = self.tick_group_end(next_tick);
            applied += self.apply_forward_through(end)?;
            if self.lifecycle == Lifecycle::Ended {
                self.accumulated_source_ns = 0;
                break;
            }
        }
        let event_range = first_event..self.state.event_cursor;
        self.delivered_event_cursor = self.state.event_cursor;
        self.last_event_range = event_range.clone();
        self.last_discontinuity = false;
        Ok(MutationReport {
            cursor: self.state.cursor,
            event_range,
            discontinuity: false,
            replayed_records: applied,
        })
    }

    pub fn seek(&mut self, cursor: ReplayCursor) -> Result<MutationReport, ReplayError> {
        let target = self
            .index
            .iter()
            .position(|entry| entry.cursor == cursor)
            .ok_or_else(|| error(ReplayErrorCode::MissingCursor, "seek.cursor"))?;
        self.seek_record(target)
    }

    pub fn seek_server_tick(&mut self, server_tick: i32) -> Result<MutationReport, ReplayError> {
        let target = self
            .index
            .iter()
            .enumerate()
            .rev()
            .find(|(_, entry)| entry.server_tick == Some(server_tick))
            .map(|(index, _)| self.tick_group_end(index))
            .ok_or_else(|| error(ReplayErrorCode::MissingCursor, "seek.server_tick"))?;
        self.seek_record(target)
    }

    pub fn step(&mut self, ticks: i64) -> Result<MutationReport, ReplayError> {
        if self.playback != PlaybackState::Paused {
            return Err(error(ReplayErrorCode::NotPaused, "playback.step"));
        }
        if ticks.unsigned_abs() as usize > self.limits.max_step_ticks {
            return Err(limit_error(
                ReplayErrorCode::StepLimit,
                "playback.step",
                ticks.unsigned_abs() as usize,
                self.limits.max_step_ticks,
            ));
        }
        if ticks == 0 {
            return Ok(self.inert_report());
        }
        let current = self
            .current_tick_position()
            .ok_or_else(|| error(ReplayErrorCode::MissingCursor, "playback.current_tick"))?;
        let target = current as i128 + ticks as i128;
        if target < 0 || target >= self.tick_records.len() as i128 {
            return Err(error(
                ReplayErrorCode::MissingCursor,
                "playback.step_target",
            ));
        }
        let record = self.tick_group_end(self.tick_records[target as usize]);
        self.seek_record(record)
    }

    pub fn snapshot(&self) -> Result<ReplaySnapshot, ReplayError> {
        if self.state.canonical_bytes().len() > self.limits.max_snapshot_bytes {
            return Err(limit_error(
                ReplayErrorCode::SnapshotLimit,
                "snapshot.bytes",
                self.state.canonical_bytes().len(),
                self.limits.max_snapshot_bytes,
            ));
        }
        Ok(ReplaySnapshot {
            identity: self.source.identity.clone(),
            limits: self.limits,
            lifecycle: self.lifecycle,
            playback: self.playback,
            rate: self.rate,
            rate_remainder_numerator: self.rate_remainder_numerator,
            rate_remainder_denominator: self.rate_remainder_denominator,
            accumulated_source_ns: self.accumulated_source_ns,
            position_record: self.position_record,
            next_record: self.next_record,
            delivered_event_cursor: self.delivered_event_cursor,
            state: (*self.state).clone(),
        })
    }

    pub fn restore(&mut self, snapshot: &ReplaySnapshot) -> Result<(), ReplayError> {
        if snapshot.identity != self.source.identity || snapshot.limits != self.limits {
            return Err(error(ReplayErrorCode::IdentityMismatch, "restore.identity"));
        }
        if snapshot.next_record > self.source.records.len()
            || snapshot.delivered_event_cursor > self.events.len()
            || snapshot.rate_remainder_denominator == 0
            || snapshot.state.canonical_bytes().len() > self.limits.max_snapshot_bytes
        {
            return Err(error(ReplayErrorCode::RestoreState, "restore.state"));
        }
        if let Some(position) = snapshot.position_record {
            let expected = self
                .index
                .get(position)
                .ok_or_else(|| error(ReplayErrorCode::RestoreState, "restore.position"))?;
            if expected.cursor != snapshot.state.cursor || snapshot.next_record != position + 1 {
                return Err(error(ReplayErrorCode::RestoreState, "restore.cursor"));
            }
        }
        validate_rate(snapshot.rate, self.limits)?;
        self.lifecycle = snapshot.lifecycle;
        self.playback = snapshot.playback;
        self.rate = snapshot.rate;
        self.rate_remainder_numerator = snapshot.rate_remainder_numerator;
        self.rate_remainder_denominator = snapshot.rate_remainder_denominator;
        self.accumulated_source_ns = snapshot.accumulated_source_ns;
        self.position_record = snapshot.position_record;
        self.next_record = snapshot.next_record;
        self.delivered_event_cursor = snapshot.delivered_event_cursor;
        self.state = Arc::new(snapshot.state.clone());
        self.last_event_range = self.delivered_event_cursor..self.delivered_event_cursor;
        self.last_discontinuity = true;
        self.audit.restored_checkpoints = self.audit.restored_checkpoints.saturating_add(1);
        Ok(())
    }

    pub fn presentation_snapshot(&self) -> Result<ReplayPresentationSnapshot, ReplayError> {
        let prior = self.state.clone();
        let Some(next_tick) = self.next_tick_record() else {
            return Ok(ReplayPresentationSnapshot {
                prior: prior.clone(),
                next: prior,
                fraction_numerator: 0,
                fraction_denominator: 1,
                event_range: self.last_event_range.clone(),
                playback: self.playback,
                discontinuity: self.last_discontinuity || self.lifecycle == Lifecycle::Ended,
            });
        };
        let duration = self.duration_to_tick(next_tick)?;
        let end = self.tick_group_end(next_tick);
        let (next, _) = self.reconstruct(end)?;
        let denominator = duration.max(1);
        Ok(ReplayPresentationSnapshot {
            prior,
            next: Arc::new(next),
            fraction_numerator: self.accumulated_source_ns.min(denominator),
            fraction_denominator: denominator,
            event_range: self.last_event_range.clone(),
            playback: self.playback,
            discontinuity: self.last_discontinuity,
        })
    }

    fn seek_record(&mut self, target: usize) -> Result<MutationReport, ReplayError> {
        let (mut state, work) = self.reconstruct(target)?;
        state.discontinuity = true;
        self.state = Arc::new(state);
        self.position_record = Some(target);
        self.next_record = target + 1;
        self.lifecycle = if contains_terminal(&self.source.records[target]) {
            Lifecycle::Ended
        } else if self.state.is_ready() {
            Lifecycle::Ready
        } else {
            Lifecycle::Signon
        };
        if self.lifecycle == Lifecycle::Ended {
            self.playback = PlaybackState::Paused;
        }
        self.accumulated_source_ns = 0;
        self.rate_remainder_numerator = 0;
        self.rate_remainder_denominator = 1;
        self.delivered_event_cursor = self.state.event_cursor;
        self.last_event_range = self.delivered_event_cursor..self.delivered_event_cursor;
        self.last_discontinuity = true;
        self.audit.restored_checkpoints = self.audit.restored_checkpoints.saturating_add(1);
        Ok(MutationReport {
            cursor: self.state.cursor,
            event_range: self.last_event_range.clone(),
            discontinuity: true,
            replayed_records: work,
        })
    }

    fn reconstruct(&self, target: usize) -> Result<(AuthoritativeState, usize), ReplayError> {
        let checkpoint = self
            .checkpoints
            .iter()
            .filter(|checkpoint| {
                checkpoint
                    .position_record
                    .is_none_or(|position| position <= target)
            })
            .max_by_key(|checkpoint| checkpoint.position_record)
            .expect("initial checkpoint exists");
        let start = checkpoint
            .position_record
            .map_or(0, |position| position + 1);
        let work = target + 1 - start;
        if work > self.limits.max_seek_records {
            return Err(limit_error(
                ReplayErrorCode::SeekWorkLimit,
                "seek.records",
                work,
                self.limits.max_seek_records,
            ));
        }
        let mut state = checkpoint.state.clone();
        let mut event_log = self.events.to_vec();
        for record in &self.source.records[start..=target] {
            let mut candidate = state.clone();
            apply_record(&mut candidate, record, &mut event_log, false, self.limits)?;
            state = candidate;
        }
        Ok((state, work))
    }

    fn apply_forward_through(&mut self, end: usize) -> Result<usize, ReplayError> {
        if end < self.next_record {
            return Ok(0);
        }
        let mut state = (*self.state).clone();
        let mut event_log = self.events.to_vec();
        let mut terminal = false;
        for record in &self.source.records[self.next_record..=end] {
            let mut candidate = state.clone();
            terminal = apply_record(&mut candidate, record, &mut event_log, false, self.limits)?;
            state = candidate;
        }
        let count = end + 1 - self.next_record;
        self.position_record = Some(end);
        self.next_record = end + 1;
        self.state = Arc::new(state);
        self.audit.applied_records = self.audit.applied_records.saturating_add(count as u64);
        if terminal {
            self.lifecycle = Lifecycle::Ended;
            self.playback = PlaybackState::Paused;
        }
        Ok(count)
    }

    fn next_tick_record(&self) -> Option<usize> {
        self.tick_records
            .iter()
            .copied()
            .find(|record| *record >= self.next_record)
    }

    fn tick_group_end(&self, tick_record: usize) -> usize {
        self.tick_records
            .iter()
            .copied()
            .find(|record| *record > tick_record)
            .map_or(self.source.records.len() - 1, |record| record - 1)
    }

    fn current_tick_position(&self) -> Option<usize> {
        let position = self.position_record?;
        self.tick_records
            .iter()
            .rposition(|record| *record <= position)
    }

    fn duration_to_tick(&self, record: usize) -> Result<u128, ReplayError> {
        let next = self.source.records[record]
            .server_tick
            .ok_or_else(|| error(ReplayErrorCode::MissingCursor, "playback.next_server_tick"))?;
        let ticks = match self.state.latest_server_tick {
            Some(current) if next > current => (next as i64 - current as i64) as u64,
            Some(current) if next == current => 0,
            Some(_) | None => 1,
        };
        u128::from(self.source.tick_interval_ns)
            .checked_mul(u128::from(ticks))
            .ok_or_else(|| error(ReplayErrorCode::ElapsedOverflow, "playback.tick_duration"))
    }

    fn accumulate_elapsed(&mut self, elapsed_ns: u64) -> Result<(), ReplayError> {
        let left = self
            .rate_remainder_numerator
            .checked_mul(u128::from(self.rate.denominator))
            .ok_or_else(|| error(ReplayErrorCode::ElapsedOverflow, "playback.rate_remainder"))?;
        let right = u128::from(elapsed_ns)
            .checked_mul(u128::from(self.rate.numerator))
            .and_then(|value| value.checked_mul(self.rate_remainder_denominator))
            .ok_or_else(|| error(ReplayErrorCode::ElapsedOverflow, "playback.elapsed"))?;
        let denominator = self
            .rate_remainder_denominator
            .checked_mul(u128::from(self.rate.denominator))
            .ok_or_else(|| {
                error(
                    ReplayErrorCode::ElapsedOverflow,
                    "playback.rate_denominator",
                )
            })?;
        let numerator = left
            .checked_add(right)
            .ok_or_else(|| error(ReplayErrorCode::ElapsedOverflow, "playback.elapsed"))?;
        let whole = numerator / denominator;
        let remainder = numerator % denominator;
        self.accumulated_source_ns = self
            .accumulated_source_ns
            .checked_add(whole)
            .ok_or_else(|| error(ReplayErrorCode::ElapsedOverflow, "playback.accumulator"))?;
        let divisor = gcd(remainder, denominator);
        self.rate_remainder_numerator = remainder / divisor;
        self.rate_remainder_denominator = denominator / divisor;
        Ok(())
    }

    fn inert_report(&self) -> MutationReport {
        MutationReport {
            cursor: self.state.cursor,
            event_range: self.delivered_event_cursor..self.delivered_event_cursor,
            discontinuity: false,
            replayed_records: 0,
        }
    }
}

fn apply_record(
    state: &mut AuthoritativeState,
    record: &RecordedRecord,
    event_log: &mut Vec<ReplayEvent>,
    building: bool,
    limits: ReplayLimits,
) -> Result<bool, ReplayError> {
    if !matches!(
        record.coverage,
        Coverage::Handled | Coverage::IntentionallyInert
    ) {
        return Err(record_error(
            ReplayErrorCode::Coverage,
            "record.coverage",
            record.ordinal,
        ));
    }
    if let Some(command_tick) = record.command_tick {
        state.latest_command_tick = Some(command_tick);
    }
    if let Some(server_tick) = record.server_tick {
        if state.latest_server_tick != Some(server_tick) {
            state.events_at_latest_tick = 0;
        }
        state.latest_tick_occurrence = if state.latest_server_tick == Some(server_tick) {
            state.latest_tick_occurrence.checked_add(1).ok_or_else(|| {
                record_error(
                    ReplayErrorCode::IndexLimit,
                    "record.tick_occurrence",
                    record.ordinal,
                )
            })?
        } else {
            0
        };
        state.latest_server_tick = Some(server_tick);
    }
    state.discontinuity = record.no_interpolation;
    let mut terminal = false;
    for (operation_ordinal, operation) in record.operations.iter().enumerate() {
        if terminal {
            return Err(operation_error(
                ReplayErrorCode::OperationAfterTerminal,
                "operation.after_terminal",
                record.ordinal,
                operation_ordinal,
            ));
        }
        match operation {
            RecordedOperation::SignonState(value) => state.signon_state = Some(*value),
            RecordedOperation::ReplaceSetup {
                family,
                version,
                bytes,
            } => {
                state.setup.insert(
                    *family,
                    SetupValue {
                        version: *version,
                        bytes: bytes.clone(),
                    },
                );
                state.discontinuity = true;
            }
            RecordedOperation::FullState {
                entities,
                decoder_state,
            } => {
                validate_entities(entities, limits, record.ordinal, operation_ordinal)?;
                state.entities = entities.clone();
                state.decoder_state = decoder_state.clone();
                state.has_full_state = true;
                state.discontinuity = true;
            }
            RecordedOperation::CreateEntity(entity) => {
                validate_entity(entity, limits, record.ordinal, operation_ordinal)?;
                if state.entities.contains_key(&entity.identity.index) {
                    return Err(operation_error(
                        ReplayErrorCode::EntityExists,
                        "entity.create",
                        record.ordinal,
                        operation_ordinal,
                    ));
                }
                state.entities.insert(entity.identity.index, entity.clone());
                state.discontinuity = true;
            }
            RecordedOperation::PatchEntity { identity, fields } => {
                if fields.len() > limits.max_fields_per_entity {
                    return Err(operation_limit_error(
                        ReplayErrorCode::FieldLimit,
                        "entity.patch.fields",
                        fields.len(),
                        limits.max_fields_per_entity,
                        record.ordinal,
                        operation_ordinal,
                    ));
                }
                let entity =
                    matching_entity_mut(state, *identity, record.ordinal, operation_ordinal)?;
                if entity.fields.len().saturating_add(fields.len()) > limits.max_fields_per_entity {
                    return Err(operation_limit_error(
                        ReplayErrorCode::FieldLimit,
                        "entity.fields",
                        entity.fields.len().saturating_add(fields.len()),
                        limits.max_fields_per_entity,
                        record.ordinal,
                        operation_ordinal,
                    ));
                }
                entity
                    .fields
                    .extend(fields.iter().map(|(id, value)| (*id, value.clone())));
            }
            RecordedOperation::DeleteEntity(identity) => {
                matching_entity_mut(state, *identity, record.ordinal, operation_ordinal)?;
                state.entities.remove(&identity.index);
                state.discontinuity = true;
            }
            RecordedOperation::ReplaceDecoderState(bytes) => {
                state.decoder_state = bytes.clone();
            }
            RecordedOperation::Event(event) => {
                if !matches!(
                    event.coverage,
                    Coverage::Handled | Coverage::IntentionallyInert
                ) {
                    return Err(operation_error(
                        ReplayErrorCode::Coverage,
                        "event.coverage",
                        record.ordinal,
                        operation_ordinal,
                    ));
                }
                if event.payload.len() > limits.max_event_payload_bytes {
                    return Err(operation_limit_error(
                        ReplayErrorCode::EventPayloadLimit,
                        "event.payload",
                        event.payload.len(),
                        limits.max_event_payload_bytes,
                        record.ordinal,
                        operation_ordinal,
                    ));
                }
                if state.event_cursor >= limits.max_events {
                    return Err(operation_limit_error(
                        ReplayErrorCode::EventLimit,
                        "events.count",
                        state.event_cursor + 1,
                        limits.max_events,
                        record.ordinal,
                        operation_ordinal,
                    ));
                }
                state.events_at_latest_tick += 1;
                if state.events_at_latest_tick > limits.max_events_per_tick {
                    return Err(operation_limit_error(
                        ReplayErrorCode::EventTickLimit,
                        "events.per_record",
                        state.events_at_latest_tick,
                        limits.max_events_per_tick,
                        record.ordinal,
                        operation_ordinal,
                    ));
                }
                let replay_event = ReplayEvent {
                    ordinal: state.event_cursor,
                    cursor: cursor_after_record(state, record.ordinal),
                    record_ordinal: record.ordinal,
                    operation_ordinal,
                    event: event.clone(),
                };
                if building {
                    event_log.push(replay_event);
                } else if event_log.get(state.event_cursor) != Some(&replay_event) {
                    return Err(operation_error(
                        ReplayErrorCode::RestoreState,
                        "event.reconstruction",
                        record.ordinal,
                        operation_ordinal,
                    ));
                }
                state.event_cursor += 1;
            }
            RecordedOperation::IntentionallyInert => {}
            RecordedOperation::Terminal => terminal = true,
        }
    }
    state.cursor = cursor_after_record(state, record.ordinal);
    let snapshot_bytes = state.canonical_bytes().len();
    if snapshot_bytes > limits.max_snapshot_bytes {
        return Err(record_limit_error(
            ReplayErrorCode::SnapshotLimit,
            "state.bytes",
            snapshot_bytes,
            limits.max_snapshot_bytes,
            record.ordinal,
        ));
    }
    Ok(terminal)
}

fn cursor_after_record(state: &AuthoritativeState, record_ordinal: usize) -> ReplayCursor {
    ReplayCursor {
        server_tick: state.latest_server_tick,
        occurrence: state.latest_tick_occurrence,
        last_applied_record_ordinal: Some(record_ordinal),
    }
}

fn matching_entity_mut(
    state: &mut AuthoritativeState,
    identity: EntityIdentity,
    record: usize,
    operation: usize,
) -> Result<&mut crate::RecordedEntity, ReplayError> {
    let entity = state.entities.get_mut(&identity.index).ok_or_else(|| {
        operation_error(
            ReplayErrorCode::MissingEntity,
            "entity.identity",
            record,
            operation,
        )
    })?;
    if entity.identity != identity {
        return Err(operation_error(
            ReplayErrorCode::EntityGeneration,
            "entity.generation",
            record,
            operation,
        ));
    }
    Ok(entity)
}

fn validate_record(
    record: &RecordedRecord,
    expected_ordinal: usize,
    prior_range_end: u64,
    source_bytes: u64,
) -> Result<(), ReplayError> {
    if record.ordinal != expected_ordinal {
        return Err(record_error(
            ReplayErrorCode::RecordOrder,
            "record.ordinal",
            record.ordinal,
        ));
    }
    if record.source_range.start < prior_range_end
        || record.source_range.start > record.source_range.end
        || record.source_range.end > source_bytes
    {
        return Err(record_error(
            ReplayErrorCode::SourceRange,
            "record.source_range",
            record.ordinal,
        ));
    }
    Ok(())
}

fn validate_limits(source: &ReplaySource, limits: ReplayLimits) -> Result<(), ReplayError> {
    if source.tick_interval_ns == 0
        || limits.max_resident_bytes == 0
        || limits.max_index_entries == 0
        || limits.max_checkpoint_count == 0
        || limits.checkpoint_interval_ticks == 0
        || limits.max_seek_records == 0
        || limits.max_snapshot_bytes == 0
        || limits.max_rate_numerator == 0
        || limits.max_rate_denominator == 0
    {
        return Err(error(ReplayErrorCode::InvalidLimits, "replay.limits"));
    }
    if source.identity.source_bytes as usize > limits.max_resident_bytes {
        return Err(limit_error(
            ReplayErrorCode::SourceLimit,
            "source.bytes",
            source.identity.source_bytes as usize,
            limits.max_resident_bytes,
        ));
    }
    Ok(())
}

fn validate_rate(rate: PlaybackRate, limits: ReplayLimits) -> Result<(), ReplayError> {
    if rate.numerator == 0
        || rate.denominator == 0
        || rate.numerator > limits.max_rate_numerator
        || rate.denominator > limits.max_rate_denominator
    {
        return Err(error(ReplayErrorCode::InvalidRate, "playback.rate"));
    }
    Ok(())
}

fn validate_entities(
    entities: &BTreeMap<u16, crate::RecordedEntity>,
    limits: ReplayLimits,
    record: usize,
    operation: usize,
) -> Result<(), ReplayError> {
    if entities.len() > limits.max_entities {
        return Err(operation_limit_error(
            ReplayErrorCode::EntityLimit,
            "entities.count",
            entities.len(),
            limits.max_entities,
            record,
            operation,
        ));
    }
    for (index, entity) in entities {
        if *index != entity.identity.index {
            return Err(operation_error(
                ReplayErrorCode::EntityGeneration,
                "entity.index",
                record,
                operation,
            ));
        }
        validate_entity(entity, limits, record, operation)?;
    }
    Ok(())
}

fn validate_entity(
    entity: &crate::RecordedEntity,
    limits: ReplayLimits,
    record: usize,
    operation: usize,
) -> Result<(), ReplayError> {
    if entity.fields.len() > limits.max_fields_per_entity {
        return Err(operation_limit_error(
            ReplayErrorCode::FieldLimit,
            "entity.fields",
            entity.fields.len(),
            limits.max_fields_per_entity,
            record,
            operation,
        ));
    }
    Ok(())
}

fn contains_terminal(record: &RecordedRecord) -> bool {
    record
        .operations
        .iter()
        .any(|operation| matches!(operation, RecordedOperation::Terminal))
}

fn estimate_resident_bytes(
    source: &ReplaySource,
    index_bytes: usize,
    checkpoint_bytes: usize,
    events: &[ReplayEvent],
    ready: &AuthoritativeState,
) -> usize {
    let record_bytes = source.records.iter().fold(0_usize, |total, record| {
        total
            .saturating_add(std::mem::size_of::<RecordedRecord>())
            .saturating_add(record.operations.iter().fold(0_usize, |bytes, operation| {
                bytes
                    .saturating_add(std::mem::size_of::<RecordedOperation>())
                    .saturating_add(operation_payload_bytes(operation))
            }))
    });
    let identity_bytes = source.identity.source_name.len()
        + source.identity.profile.len()
        + source.identity.game.len()
        + source.identity.codec.len()
        + source.identity.decoder.len()
        + source.identity.index.len();
    source.identity.source_bytes as usize
        + identity_bytes
        + record_bytes
        + index_bytes
        + checkpoint_bytes
        + std::mem::size_of_val(events)
        + ready.canonical_bytes().len()
}

fn operation_payload_bytes(operation: &RecordedOperation) -> usize {
    match operation {
        RecordedOperation::ReplaceSetup { bytes, .. }
        | RecordedOperation::ReplaceDecoderState(bytes) => bytes.len(),
        RecordedOperation::FullState {
            entities,
            decoder_state,
        } => decoder_state.len() + entities.values().map(entity_payload_bytes).sum::<usize>(),
        RecordedOperation::CreateEntity(entity) => entity_payload_bytes(entity),
        RecordedOperation::PatchEntity { fields, .. } => fields
            .values()
            .map(|field| field.bytes.len() + std::mem::size_of::<crate::RecordedField>())
            .sum(),
        RecordedOperation::Event(event) => event.payload.len(),
        RecordedOperation::SignonState(_)
        | RecordedOperation::DeleteEntity(_)
        | RecordedOperation::IntentionallyInert
        | RecordedOperation::Terminal => 0,
    }
}

fn entity_payload_bytes(entity: &crate::RecordedEntity) -> usize {
    std::mem::size_of::<crate::RecordedEntity>()
        + entity
            .fields
            .values()
            .map(|field| field.bytes.len() + std::mem::size_of::<crate::RecordedField>())
            .sum::<usize>()
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left.max(1)
}

fn record_error(code: ReplayErrorCode, field: &'static str, record: usize) -> ReplayError {
    let mut result = error(code, field);
    result.record_ordinal = Some(record);
    result
}

fn operation_error(
    code: ReplayErrorCode,
    field: &'static str,
    record: usize,
    operation: usize,
) -> ReplayError {
    let mut result = record_error(code, field, record);
    result.operation_ordinal = Some(operation);
    result
}

fn limit_error(
    code: ReplayErrorCode,
    field: &'static str,
    actual: usize,
    limit: usize,
) -> ReplayError {
    let mut result = error(code, field);
    result.required = Some(actual as u64);
    result.limit = Some(limit as u64);
    result
}

fn record_limit_error(
    code: ReplayErrorCode,
    field: &'static str,
    actual: usize,
    limit: usize,
    record: usize,
) -> ReplayError {
    let mut result = limit_error(code, field, actual, limit);
    result.record_ordinal = Some(record);
    result
}

fn operation_limit_error(
    code: ReplayErrorCode,
    field: &'static str,
    actual: usize,
    limit: usize,
    record: usize,
    operation: usize,
) -> ReplayError {
    let mut result = record_limit_error(code, field, actual, limit, record);
    result.operation_ordinal = Some(operation);
    result
}
