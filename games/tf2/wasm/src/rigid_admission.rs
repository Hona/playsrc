use super::*;
use std::collections::BTreeSet;

pub(super) fn prepare(
    graph: &playsrc_entity::Graph,
    bundle: &BTreeMap<String, &[u8]>,
    models: &BTreeMap<String, Arc<RetainedPresentationModel>>,
    hashes: &BTreeMap<String, [u8; 32]>,
    surfaces: &playsrc_material::SurfacePropertyRegistry,
) -> Result<
    (
        playsrc_tf2::RigidProjectileModels,
        BTreeMap<String, playsrc_tf2::StudioRigidResource>,
    ),
    (),
> {
    let compile = |path: &str| -> Result<playsrc_tf2::rigid_body::RigidModel, ()> {
        let bytes = bundle.get(path).ok_or(())?;
        let asset = playsrc_phy::parse_standalone(
            bytes,
            playsrc_phy::Profile::SourcePcPolygon,
            playsrc_phy::Limits::default(),
        )
        .map_err(|_| ())?;
        let identity = u64::from_le_bytes(
            hashes.get(path).ok_or(())?[..8]
                .try_into()
                .map_err(|_| ())?,
        );
        playsrc_tf2::rigid_body::RigidModel::compile(
            identity,
            &asset,
            None,
            surfaces,
            playsrc_collision::SnapshotLimits::default(),
        )
        .map_err(|_| ())
    };
    let projectiles = playsrc_tf2::RigidProjectileModels {
        sticky: compile("models/weapons/w_models/w_stickybomb.phy")?,
        grenade: compile("models/weapons/w_models/w_grenade_grenadelauncher.phy")?,
    };
    let mut required = BTreeSet::new();
    for entity in &graph.entities {
        if entity.classname.as_deref().is_some_and(|name| {
            [
                b"prop_dynamic".as_slice(),
                b"dynamic_prop",
                b"prop_dynamic_override",
                b"training_prop_dynamic",
            ]
            .iter()
            .any(|candidate| name.eq_ignore_ascii_case(candidate))
        }) {
            if let Some(model) = &entity.model {
                required.insert(
                    std::str::from_utf8(model)
                        .map_err(|_| ())?
                        .to_ascii_lowercase(),
                );
            }
        }
    }
    let mut output = BTreeMap::new();
    for path in required {
        let model = models.get(&path).ok_or(())?.source();
        let physics_path = path.strip_suffix(".mdl").ok_or(())?.to_owned() + ".phy";
        let Some(bytes) = bundle.get(&physics_path) else {
            if model.physics_status != playsrc_studio_model::PhysicsStatus::Missing {
                return Err(());
            }
            continue;
        };
        let asset = playsrc_phy::parse_standalone(
            bytes,
            playsrc_phy::Profile::SourcePcPolygon,
            playsrc_phy::Limits::default(),
        )
        .map_err(|_| ())?;
        if asset.header.as_ref().map(|header| header.checksum) != Some(model.checksum) {
            return Err(());
        }
        let mdl = bundle.get(&path).ok_or(())?;
        let keydata = playsrc_studio_model::read_model_keydata(
            &path,
            mdl,
            playsrc_studio_model::Limits::default(),
        )
        .map_err(|_| ())?;
        let keydata = keydata.strip_suffix(&[0]).unwrap_or(keydata);
        let follower_bones = if keydata.is_empty() {
            None
        } else {
            let document = playsrc_keyvalues::parse_text(
                keydata,
                playsrc_keyvalues::EscapeMode::LiteralBackslash,
                playsrc_keyvalues::Limits::default(),
            )
            .map_err(|_| ())?;
            let root = document
                .roots
                .iter()
                .find(|node| node.key.bytes.eq_ignore_ascii_case(b"bone_followers"));
            match root {
                None => None,
                Some(root) => {
                    let playsrc_keyvalues::Value::Object(children) = &root.value else {
                        return Err(());
                    };
                    let mut bones = Vec::new();
                    for child in children {
                        let playsrc_keyvalues::Value::Scalar(value) = &child.value else {
                            return Err(());
                        };
                        bones.push(value.token.bytes.clone());
                    }
                    Some(bones)
                }
            }
        };
        let digest = hashes.get(&physics_path).ok_or(())?;
        let shape_identities = (0..asset.solids.len())
            .map(|index| {
                let mut hash = Sha256::new();
                hash.update(digest);
                hash.update((index as u64).to_le_bytes());
                u64::from_le_bytes(hash.finalize()[..8].try_into().expect("shape identity"))
            })
            .collect();
        output.insert(
            path,
            playsrc_tf2::StudioRigidResource {
                physics: Arc::new(asset),
                model: Arc::clone(model),
                shape_identities,
                follower_bones,
            },
        );
    }
    Ok((projectiles, output))
}
