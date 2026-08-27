use playsrc_audio::acoustics::*;

struct BoxWorld {
    minimum: Position,
    maximum: Position,
    sky: bool,
    traces: Vec<TraceKind>,
}
impl Geometry for BoxWorld {
    fn trace(&mut self, start: Position, end: Position, kind: TraceKind) -> Hit {
        self.traces.push(kind);
        let mut fraction = 1.0_f32;
        let mut axis_hit = 0;
        for axis in 0..3 {
            let direction = end[axis] - start[axis];
            let plane = if direction > 0.0 {
                self.maximum[axis]
            } else {
                self.minimum[axis]
            };
            if direction != 0.0 {
                let crossing = (plane - start[axis]) / direction;
                if crossing >= 0.0 && crossing < fraction {
                    fraction = crossing;
                    axis_hit = axis;
                }
            }
        }
        Hit {
            start,
            end: std::array::from_fn(|axis| start[axis] + fraction * (end[axis] - start[axis])),
            hit: fraction < 1.0,
            sky: self.sky && axis_hit == 2 && end[2] > start[2],
            reflectivity: Some(0.5),
        }
    }
}

#[test]
fn bounded_scan_waits_then_constructs_and_reuses_a_real_room_node() {
    let mut world = BoxWorld {
        minimum: [-128.0, -256.0, 0.0],
        maximum: [128.0, 256.0, 192.0],
        sky: false,
        traces: vec![],
    };
    let mut detector = Detector::default();
    for frame in 0..16 {
        assert!(
            detector
                .update(true, frame as f64 * 0.015, [0.0, 0.0, 64.0], &mut world)
                .is_none()
        );
    }
    assert!(world.traces.is_empty());
    let mut constructed = None;
    for frame in 17..30 {
        let before = world.traces.len();
        if let Some(change) =
            detector.update(true, frame as f64 * 0.015, [0.0, 0.0, 64.0], &mut world)
        {
            constructed = Some(change);
            break;
        }
        assert!(world.traces.len() - before <= 6);
    }
    let change = constructed.unwrap();
    let room = change.created.unwrap();
    assert_eq!(change.node, 0);
    assert_eq!((room.width, room.length, room.height), (256, 512, 192));
    assert!(!room.outside);
    assert_eq!(room.reflectivity, 0.5);
    assert_eq!(room.diffusion, 0.0);
    let mut reused = false;
    for frame in 30..100 {
        if let Some(change) =
            detector.update(true, frame as f64 * 0.015, [8.0, 0.0, 64.0], &mut world)
        {
            assert_eq!(change.node, 0);
            assert!(change.created.is_none());
            reused = true;
            break;
        }
    }
    assert!(reused);
    detector.reset();
    assert_eq!(detector.selected(), None);
}

#[test]
fn jumping_does_not_begin_wall_scan_and_disabled_dsp_never_traces() {
    let mut world = BoxWorld {
        minimum: [-128.0, -128.0, 0.0],
        maximum: [128.0, 128.0, 256.0],
        sky: false,
        traces: vec![],
    };
    let mut detector = Detector::default();
    assert!(
        detector
            .update(false, 1.0, [0.0, 0.0, 80.0], &mut world)
            .is_none()
    );
    assert!(world.traces.is_empty());
    assert!(
        detector
            .update(true, 1.0, [0.0, 0.0, 80.0], &mut world)
            .is_none()
    );
    assert_eq!(world.traces, [TraceKind::WorldAndStaticProps; 3]);
}
