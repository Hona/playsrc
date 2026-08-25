use playsrc_visibility::{AreaState, CandidateSet, SkyVisibility, ViewQuery, ViewResult};
use sha2::{Digest, Sha256};
use std::{hint::black_box, io::Read, time::Instant};

const SAMPLE_COUNT: usize = 33;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target = std::env::args()
        .nth(1)
        .ok_or("one declared map target is required")?;
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes)?;
    let bsp = playsrc_bsp::parse(
        &bytes,
        playsrc_bsp::Profile::Source2013V20,
        playsrc_bsp::Limits::default(),
    )?;
    let started = Instant::now();
    let world = playsrc_visibility::compile(&bsp)?;
    let compile_nanoseconds = started.elapsed().as_nanos();
    let area = AreaState::new(&world);
    let mut points = Vec::new();
    let mut outside = None;
    let mut sky_origin = None;
    for (index, leaf) in world.leaves.iter().enumerate() {
        let point = std::array::from_fn(|axis| {
            (f32::from(leaf.mins[axis]) + f32::from(leaf.maxs[axis])) * 0.5
        });
        if world.locate_leaf(point)? != index {
            continue;
        }
        if leaf.cluster < 0 {
            if outside.is_none() {
                outside = Some(point);
            }
        } else {
            if sky_origin.is_none() && (leaf.area_and_flags >> 9) & 0x05 != 0 {
                sky_origin = Some(point);
            }
            if leaf.area_and_flags & 0x01ff != 0 && points.len() < 16 {
                points.push(point);
            }
        }
    }
    let fixed = if target == "jump_beef" {
        [5328.0, 3376.0, f32::from_bits(0xc53f_b35c)]
    } else {
        *points.first().ok_or("no interior camera origin")?
    };
    if world.leaves[world.locate_leaf(fixed)?].cluster < 0 {
        return Err("fixed camera is outside the world".into());
    }
    let same_leaf = same_leaf_points(&world, fixed);
    let outside = outside.ok_or("no outside-world camera origin")?;
    let second = points
        .iter()
        .find(|point| world.locate_leaf(**point).ok() != world.locate_leaf(fixed).ok())
        .copied()
        .unwrap_or(fixed);

    let ordinary_states = std::slice::from_ref(&area);
    let mut scenarios = vec![
        measure(&world, ordinary_states, "same-origin", |_, _| vec![fixed])?,
        measure(
            &world,
            ordinary_states,
            "same-leaf-motion",
            |iteration, _| vec![same_leaf[iteration % same_leaf.len()]],
        )?,
        measure(
            &world,
            ordinary_states,
            "leaf-transitions",
            |iteration, _| vec![points[iteration % points.len()]],
        )?,
        measure(&world, ordinary_states, "multi-origin", |_, _| {
            vec![fixed, second]
        })?,
        measure(&world, ordinary_states, "outside-world", |_, _| {
            vec![outside]
        })?,
    ];
    if let Some(portal) = world.portals.first() {
        let mut open = area.clone();
        open.set_portals(&[(portal.key, true)])?;
        scenarios.push(measure(
            &world,
            &[area.clone(), open],
            "portal-revisions",
            |_, _| vec![fixed],
        )?);
    }
    if let Some(origin) = sky_origin {
        scenarios.push(measure(&world, ordinary_states, "sky-origin", |_, _| {
            vec![origin]
        })?);
    }
    println!(
        "{{\"worldIdentity\":\"{}\",\"compileNanoseconds\":{},\"topology\":{{\"nodes\":{},\"leaves\":{},\"clusters\":{},\"areas\":{},\"directedPortals\":{},\"leafFaces\":{}}},\"scenarios\":[{}]}}",
        hex(&world.identity),
        compile_nanoseconds,
        world.nodes.len(),
        world.leaves.len(),
        world.cluster_count,
        world.areas.len(),
        world.portals.len(),
        world.leaf_faces.len(),
        scenarios.join(","),
    );
    Ok(())
}

fn same_leaf_points(world: &playsrc_visibility::World, origin: [f32; 3]) -> Vec<[f32; 3]> {
    let leaf = world.locate_leaf(origin).expect("validated origin");
    let mut points = vec![origin];
    for offset in [0.125, -0.125, 0.25, -0.25, 0.5, -0.5] {
        let mut point = origin;
        point[0] += offset;
        if world.locate_leaf(point).ok() == Some(leaf) {
            points.push(point);
        }
    }
    points
}

fn measure(
    world: &playsrc_visibility::World,
    states: &[AreaState],
    name: &str,
    origins: impl Fn(usize, &playsrc_visibility::World) -> Vec<[f32; 3]>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut main = Vec::with_capacity(SAMPLE_COUNT);
    let mut duplicate = Vec::with_capacity(SAMPLE_COUNT);
    let mut candidate = Vec::with_capacity(SAMPLE_COUNT);
    let mut total = Vec::with_capacity(SAMPLE_COUNT);
    let mut identities = Vec::with_capacity(SAMPLE_COUNT);
    let mut outputs = Vec::with_capacity(SAMPLE_COUNT);
    let mut sky = Vec::with_capacity(SAMPLE_COUNT);
    for iteration in 0..SAMPLE_COUNT {
        let whole = Instant::now();
        let started = Instant::now();
        let candidates = CandidateSet::compile(world, 0, &[])?;
        candidate.push(started.elapsed().as_nanos());
        let query = ViewQuery {
            origins: origins(iteration, world),
            bypass_pvs: false,
        };
        let area = &states[iteration % states.len()];
        let started = Instant::now();
        let result = world.view(area, &candidates, &query)?;
        main.push(started.elapsed().as_nanos());
        let started = Instant::now();
        let repeated = world.view(area, &candidates, &query)?;
        duplicate.push(started.elapsed().as_nanos());
        total.push(whole.elapsed().as_nanos());
        if result != repeated {
            return Err(format!("repeated view differs in {name} at sample {iteration}").into());
        }
        identities.push(hex(&result.cache_identity));
        outputs.push(hex(&view_digest(&result)));
        sky.push(match result.sky {
            SkyVisibility::NotVisible => 0,
            SkyVisibility::Sky2d => 1,
            SkyVisibility::Sky3d => 2,
        });
        black_box(result);
        black_box(repeated);
    }
    Ok(format!(
        "{{\"name\":\"{}\",\"samples\":{},\"candidateNanoseconds\":{},\"mainViewNanoseconds\":{},\"duplicateViewNanoseconds\":{},\"totalNanoseconds\":{},\"cacheIdentities\":{},\"outputSha256\":{},\"skyKinds\":{}}}",
        name,
        SAMPLE_COUNT,
        numbers(&candidate),
        numbers(&main),
        numbers(&duplicate),
        numbers(&total),
        strings(&identities),
        strings(&outputs),
        numbers(&sky),
    ))
}

fn view_digest(view: &ViewResult) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(view.cache_identity);
    digest.update((view.origin_leaves.len() as u64).to_le_bytes());
    for value in &view.origin_leaves {
        digest.update((*value as u64).to_le_bytes());
    }
    for value in &view.origin_clusters {
        digest.update(value.to_le_bytes());
    }
    digest.update([u8::from(view.outside_world)]);
    for value in &view.merged_pvs {
        digest.update(value.to_le_bytes());
    }
    for value in &view.visible_areas {
        digest.update((*value as u64).to_le_bytes());
    }
    digest.update([match view.sky {
        SkyVisibility::NotVisible => 0,
        SkyVisibility::Sky2d => 1,
        SkyVisibility::Sky3d => 2,
    }]);
    for value in &view.leaves {
        digest.update((*value as u64).to_le_bytes());
    }
    for value in &view.world_surfaces {
        digest.update(value.to_le_bytes());
    }
    for value in &view.candidates {
        digest.update([value.kind as u8]);
        digest.update(value.index.to_le_bytes());
    }
    digest.finalize().into()
}

fn numbers(values: &[u128]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn strings(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
