use std::collections::BTreeSet;
use playsrc_content::Resolution;
use playsrc_particle::{DefinitionLookup, PcfSource, Registry, RegistryLimits};
use playsrc_tf2::particle_resources::{GAME_FILES, manifest_files, roots};
use crate::Resolver;

/// Resolve the configured manifests, then retain only files needed by the actual
/// game/map root closure. The ordered source list preserves name overrides in WASM.
pub fn collect(resolver: &mut Resolver<'_>, graph: &playsrc_entity::Graph, target: &str,
    has_bsp_manifest: bool) -> Result<(Registry, Vec<u8>), String> {
    let manifest = resolver.required("particles/particles_manifest.txt", "particle-manifest")?;
    let mut paths = manifest_files(&manifest)?;
    let map_path = format!("maps/{target}_particles.txt");
    let map_manifest = if has_bsp_manifest {
        Some(resolver.required("particles.txt", "map-particle-manifest")?)
    } else {
        resolver.optional(&map_path, "map-particle-manifest")?
    };
    if let Some(manifest) = map_manifest {
        paths.extend(manifest_files(&manifest)?.into_iter()
            .filter(|path| path.rsplit_once('/').is_some_and(|(directory, _)| directory == "particles"))
            .take(64));
    }
    for path in GAME_FILES {
        if !paths.iter().any(|candidate| candidate == path) {
            return Err(format!("game particle file absent from configured manifest: {path}"));
        }
    }
    let bytes = paths.iter().map(|path| {
        match resolver.content.resolve_resource(path).map_err(|error| error.to_string())? {
            Resolution::Found(resource) => Ok(resource.bytes),
            Resolution::Missing { .. } => Err(format!("manifest particle file unavailable: {path}")),
        }
    }).collect::<Result<Vec<_>, String>>()?;
    let sources = paths.iter().zip(&bytes).map(|(path, bytes)| PcfSource { logical_path: path, bytes }).collect::<Vec<_>>();
    let registry = Registry::from_pcf(&sources, RegistryLimits::default()).map_err(|error| error.to_string())?;
    let roots = roots(graph).into_iter().map(DefinitionLookup::Name).collect::<Vec<_>>();
    let closure = registry.dependency_closure(&roots).map_err(|error| error.to_string())?;
    let needed = closure.definitions.iter().map(|index| registry.definition_at(*index)
        .expect("closed particle definition").source.as_str()).collect::<BTreeSet<_>>();
    let mut retained = Vec::new();
    for path in paths.iter().filter(|path| needed.contains(path.as_str())) {
        resolver.required(path, "particle-registry")?;
        retained.extend_from_slice(path.as_bytes());
        retained.push(b'\n');
    }
    Ok((registry, retained))
}
