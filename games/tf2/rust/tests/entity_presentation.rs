use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use playsrc_collision::Hull;
use playsrc_entity::{ModelBounds, RuntimeFailureCode};
use playsrc_movement::{Error as MoveError, Trace, Tracer};
use playsrc_tf2::{GameplayWorld, MapRuntime, PresentationError, PresentationRevision, Session};

#[derive(Clone)]
struct RevisionWorld {
    revision: Arc<AtomicU64>,
    advance_on_read: bool,
    available: bool,
}

impl RevisionWorld {
    fn stable(revision: u64) -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(revision)),
            advance_on_read: false,
            available: true,
        }
    }

    fn advancing(revision: u64) -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(revision)),
            advance_on_read: true,
            available: true,
        }
    }

    fn unavailable() -> Self {
        Self {
            revision: Arc::new(AtomicU64::new(0)),
            advance_on_read: false,
            available: false,
        }
    }
}

impl Tracer for RevisionWorld {
    fn trace(
        &self,
        _start: [f32; 3],
        end: [f32; 3],
        _hull: Hull,
        _mask: u32,
    ) -> Result<Trace, MoveError> {
        Ok(Trace {
            fraction: 1.0,
            start_solid: false,
            all_solid: false,
            end,
            normal: None,
            hit: None,
            contents: 0,
        })
    }
}

impl GameplayWorld for RevisionWorld {
    fn collision_snapshot_revision(&self) -> Option<u64> {
        if !self.available {
            None
        } else if self.advance_on_read {
            Some(self.revision.fetch_add(1, Ordering::SeqCst))
        } else {
            Some(self.revision.load(Ordering::SeqCst))
        }
    }

    fn overlaps_model_hull(
        &self,
        _model: usize,
        _origin: [f32; 3],
        _position: [f32; 3],
        _hull: Hull,
    ) -> Result<bool, MoveError> {
        Ok(false)
    }
}

fn make_session(world: RevisionWorld) -> Session<RevisionWorld> {
    let graph = playsrc_entity::parse(
        br#"{"classname" "func_brush" "model" "*1" "origin" "3 4 5"}"#,
        playsrc_entity::Limits::default(),
    )
    .unwrap();
    let map = MapRuntime::compile(
        &graph,
        0.015,
        0x1020_3040,
        vec![ModelBounds {
            model: 1,
            mins: [0.0; 3],
            maxs: [8.0; 3],
        }],
    )
    .unwrap();
    Session::new(world, [0.0; 3], map)
}

#[test]
fn session_publishes_one_atomic_entity_and_collision_revision_join() {
    let session = make_session(RevisionWorld::stable(91));
    let entity_revision = session.entity_revision();
    let snapshot = session
        .entity_presentation(PresentationRevision {
            entity: entity_revision,
            collision: 91,
        })
        .unwrap();
    assert_eq!(snapshot.collision_revision, 91);
    assert_eq!(snapshot.entities.source_identity, 0x1020_3040);
    assert_eq!(snapshot.entities.registry_identity, 0x5446_325f_454e_5433);
    assert_eq!(snapshot.entities.tick, 0);
    assert_eq!(snapshot.entities.revision, entity_revision);
    assert_eq!(snapshot.entities.models.len(), 1);
    assert_eq!(snapshot.entities.models[0].source_index, 0);
    assert_eq!(
        snapshot.entities.models[0].world_transform.origin,
        [3.0, 4.0, 5.0]
    );

    let restored = session.clone();
    assert_eq!(
        restored
            .entity_presentation(PresentationRevision {
                entity: entity_revision,
                collision: 91,
            })
            .unwrap(),
        snapshot
    );
}

#[test]
fn revision_failures_publish_nothing_and_do_not_mutate_gameplay() {
    let session = make_session(RevisionWorld::stable(17));
    let producer = session.producer_snapshot();
    let movement = session.movement_snapshot_bytes();
    let entity_revision = session.entity_revision();

    let entity_error = session
        .entity_presentation(PresentationRevision {
            entity: entity_revision - 1,
            collision: 17,
        })
        .unwrap_err();
    assert!(matches!(
        entity_error,
        PresentationError::Entity(error) if error.code == RuntimeFailureCode::RevisionMismatch
    ));
    let collision_error = session
        .entity_presentation(PresentationRevision {
            entity: entity_revision,
            collision: 16,
        })
        .unwrap_err();
    assert_eq!(
        collision_error,
        PresentationError::CollisionRevisionMismatch {
            expected: 16,
            actual: 17,
        }
    );
    assert_eq!(session.producer_snapshot(), producer);
    assert_eq!(session.movement_snapshot_bytes(), movement);
    assert_eq!(session.entity_revision(), entity_revision);

    let unavailable = make_session(RevisionWorld::unavailable());
    assert_eq!(
        unavailable
            .entity_presentation(PresentationRevision {
                entity: unavailable.entity_revision(),
                collision: 0,
            })
            .unwrap_err(),
        PresentationError::CollisionRevisionUnavailable
    );
}

#[test]
fn collision_revision_change_during_publication_rolls_back_the_whole_output() {
    let session = make_session(RevisionWorld::advancing(7));
    let before = session.producer_snapshot();
    let entity_revision = session.entity_revision();
    assert_eq!(
        session
            .entity_presentation(PresentationRevision {
                entity: entity_revision,
                collision: 7,
            })
            .unwrap_err(),
        PresentationError::CollisionRevisionMismatch {
            expected: 7,
            actual: 8,
        }
    );
    assert_eq!(session.producer_snapshot(), before);
    assert_eq!(session.entity_revision(), entity_revision);
}
