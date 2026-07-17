use std::{collections::BTreeMap, fmt, ops::Range, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_entities: usize,
    pub max_pairs: usize,
    pub max_pairs_per_entity: usize,
    pub max_string_bytes: usize,
    pub max_connections: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_entities: 16_384,
            max_pairs: 262_144,
            max_pairs_per_entity: 4_096,
            max_string_bytes: 16_384,
            max_connections: 262_144,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pair {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub key_range: Range<usize>,
    pub value_range: Range<usize>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionError {
    FieldCount,
    EmptyTarget,
    EmptyInput,
    InvalidDelay,
    InvalidMaxFires,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Connection {
    Parsed {
        order: usize,
        output: Vec<u8>,
        target: Vec<u8>,
        input: Vec<u8>,
        parameter: Vec<u8>,
        delay_bits: u32,
        max_fires: i32,
    },
    Malformed {
        order: usize,
        output: Vec<u8>,
        error: ConnectionError,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entity {
    pub index: usize,
    pub pairs: Vec<Pair>,
    pub classname: Option<Vec<u8>>,
    pub model: Option<Vec<u8>>,
    pub bsp_model_index: Option<usize>,
    pub targetname: Option<Vec<u8>>,
    pub parentname: Option<Vec<u8>>,
    pub spawnflags: Option<Vec<u8>>,
    pub connections: Vec<Connection>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inventory {
    pub entity_count: usize,
    pub pair_count: usize,
    pub parsed_connections: usize,
    pub malformed_connections: usize,
    pub class_counts: BTreeMap<Vec<u8>, usize>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Graph {
    pub source: Vec<u8>,
    pub source_range: Range<usize>,
    pub terminator_range: Range<usize>,
    pub suffix: Vec<u8>,
    pub entities: Vec<Entity>,
    pub inventory: Inventory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportDestination {
    pub entity: usize,
    pub targetname: Vec<u8>,
    pub position: [f32; 3],
    pub yaw_degrees: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TeleportTrigger {
    pub entity: usize,
    pub model: usize,
    pub origin: [f32; 3],
    pub destination: usize,
    pub preserve_angles: bool,
}

#[derive(Clone, Debug)]
pub struct Runtime {
    collision: Option<Arc<playsrc_collision::World>>,
    pub destinations: Vec<TeleportDestination>,
    pub teleports: Vec<TeleportTrigger>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TeleportResult {
    pub trigger_entity: usize,
    pub destination_entity: usize,
    pub position: [f32; 3],
    pub yaw_degrees: Option<f32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeErrorCode {
    InvalidField,
    MissingModel,
    MissingTarget,
    UnsupportedLandmark,
    UnsupportedFilter,
    UnsupportedState,
    UnsupportedTransform,
    Collision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeError {
    pub code: RuntimeErrorCode,
    pub entity: usize,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "{:?} for entity {}", self.code, self.entity)
    }
}

impl std::error::Error for RuntimeError {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InputLimit,
    InvalidByte,
    UnterminatedString,
    StringLimit,
    UnexpectedToken,
    MissingValue,
    MissingClose,
    EntityLimit,
    PairLimit,
    ConnectionLimit,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub range: Range<usize>,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} at {}..{}",
            self.code, self.range.start, self.range.end
        )
    }
}
impl std::error::Error for Error {}
enum Token {
    Open,
    Close,
    Text(Vec<u8>, Range<usize>),
}

pub fn parse(bytes: &[u8], limits: Limits) -> Result<Graph, Error> {
    if bytes.len() > limits.max_bytes {
        return Err(error(ErrorCode::InputLimit, 0..bytes.len()));
    }
    let terminator = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let mut term_end = terminator;
    while term_end < bytes.len() && bytes[term_end] == 0 {
        term_end += 1;
    }
    if term_end != bytes.len() {
        return Err(error(ErrorCode::InvalidByte, term_end..bytes.len()));
    }
    let source = &bytes[..terminator];
    let tokens = tokenize(source, limits)?;
    let mut at = 0;
    let mut entities = Vec::new();
    let mut total_pairs = 0;
    while at < tokens.len() {
        if !matches!(tokens.get(at), Some(Token::Open)) {
            return Err(error(ErrorCode::UnexpectedToken, 0..0));
        }
        at += 1;
        let mut pairs = Vec::new();
        while !matches!(tokens.get(at), Some(Token::Close)) {
            let Some(Token::Text(key, key_range)) = tokens.get(at) else {
                return Err(error(ErrorCode::UnexpectedToken, 0..0));
            };
            let Some(Token::Text(value, value_range)) = tokens.get(at + 1) else {
                return Err(error(ErrorCode::MissingValue, key_range.clone()));
            };
            if pairs.len() >= limits.max_pairs_per_entity || total_pairs >= limits.max_pairs {
                return Err(error(ErrorCode::PairLimit, key_range.clone()));
            }
            pairs.push(Pair {
                key: key.clone(),
                value: value.clone(),
                key_range: key_range.clone(),
                value_range: value_range.clone(),
            });
            total_pairs += 1;
            at += 2;
        }
        at += 1;
        if entities.len() >= limits.max_entities {
            return Err(error(ErrorCode::EntityLimit, 0..0));
        }
        entities.push(build_entity(entities.len(), pairs, limits)?);
    }
    let mut class_counts = BTreeMap::new();
    let mut parsed = 0;
    let mut malformed = 0;
    for entity in &entities {
        if let Some(class) = &entity.classname {
            *class_counts.entry(class.clone()).or_insert(0) += 1;
        }
        for connection in &entity.connections {
            match connection {
                Connection::Parsed { .. } => parsed += 1,
                Connection::Malformed { .. } => malformed += 1,
            }
        }
    }
    Ok(Graph {
        source: bytes.to_vec(),
        source_range: 0..terminator,
        terminator_range: terminator..term_end,
        suffix: bytes[term_end..].to_vec(),
        inventory: Inventory {
            entity_count: entities.len(),
            pair_count: total_pairs,
            parsed_connections: parsed,
            malformed_connections: malformed,
            class_counts,
        },
        entities,
    })
}

fn tokenize(bytes: &[u8], limits: Limits) -> Result<Vec<Token>, Error> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            0x01..=0x20 => at += 1,
            b'/' if bytes.get(at + 1) == Some(&b'/') => {
                at += 2;
                while at < bytes.len() && !matches!(bytes[at], b'\r' | b'\n') {
                    at += 1;
                }
            }
            b'{' => {
                out.push(Token::Open);
                at += 1;
            }
            b'}' => {
                out.push(Token::Close);
                at += 1;
            }
            b'"' => {
                let start = at;
                at += 1;
                let content = at;
                while at < bytes.len() && bytes[at] != b'"' {
                    if bytes[at] == 0 {
                        return Err(error(ErrorCode::InvalidByte, at..at + 1));
                    }
                    at += 1;
                }
                if at == bytes.len() {
                    return Err(error(ErrorCode::UnterminatedString, start..at));
                }
                if at - content > limits.max_string_bytes {
                    return Err(error(ErrorCode::StringLimit, content..at));
                }
                out.push(Token::Text(bytes[content..at].to_vec(), start..at + 1));
                at += 1;
            }
            _ => {
                let start = at;
                while at < bytes.len()
                    && !matches!(bytes[at], 0x01..=0x20 | b'{' | b'}')
                    && !(bytes[at] == b'/' && bytes.get(at + 1) == Some(&b'/'))
                {
                    at += 1;
                }
                if at - start > limits.max_string_bytes {
                    return Err(error(ErrorCode::StringLimit, start..at));
                }
                out.push(Token::Text(bytes[start..at].to_vec(), start..at));
            }
        }
    }
    Ok(out)
}
fn first(pairs: &[Pair], name: &[u8]) -> Option<Vec<u8>> {
    pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(name))
        .map(|pair| pair.value.clone())
}
fn build_entity(index: usize, pairs: Vec<Pair>, limits: Limits) -> Result<Entity, Error> {
    let classname = first(&pairs, b"classname");
    let model = first(&pairs, b"model");
    let bsp_model_index = model
        .as_deref()
        .and_then(|v| v.strip_prefix(b"*"))
        .and_then(|v| std::str::from_utf8(v).ok()?.parse().ok())
        .or_else(|| {
            classname
                .as_deref()
                .is_some_and(|v| v.eq_ignore_ascii_case(b"worldspawn"))
                .then_some(0)
        });
    let mut connections = Vec::new();
    for (order, pair) in pairs.iter().enumerate() {
        if let Some(connection) = connection(order, pair) {
            if connections.len() >= limits.max_connections {
                return Err(error(ErrorCode::ConnectionLimit, pair.key_range.clone()));
            }
            connections.push(connection);
        }
    }
    Ok(Entity {
        index,
        targetname: first(&pairs, b"targetname"),
        parentname: first(&pairs, b"parentname"),
        spawnflags: first(&pairs, b"spawnflags"),
        pairs,
        classname,
        model,
        bsp_model_index,
        connections,
    })
}
fn connection(order: usize, pair: &Pair) -> Option<Connection> {
    let delimiter = if pair.value.contains(&0x1b) {
        0x1b
    } else {
        b','
    };
    let fields: Vec<_> = pair.value.split(|b| *b == delimiter).collect();
    let shaped =
        fields.len() == 5 && number_f32(fields[3]).is_some() && integer(fields[4]).is_some();
    if delimiter != 0x1b && !pair.key.starts_with(b"On") && !pair.key.starts_with(b"Out") && !shaped
    {
        return None;
    }
    let bad = |e| Connection::Malformed {
        order,
        output: pair.key.clone(),
        error: e,
    };
    if fields.len() != 5 {
        return Some(bad(ConnectionError::FieldCount));
    }
    if fields[0].is_empty() {
        return Some(bad(ConnectionError::EmptyTarget));
    }
    let input = if fields[1].is_empty() {
        b"Use".as_slice()
    } else {
        fields[1]
    };
    let Some(delay) = (if fields[3].is_empty() {
        Some(0.0)
    } else {
        number_f32(fields[3])
    }) else {
        return Some(bad(ConnectionError::InvalidDelay));
    };
    let Some(mut max_fires) = (if fields[4].is_empty() {
        Some(-1)
    } else {
        integer(fields[4])
    }) else {
        return Some(bad(ConnectionError::InvalidMaxFires));
    };
    if max_fires == 0 || max_fires == -1 {
        max_fires = -1;
    } else if max_fires < 0 {
        return Some(bad(ConnectionError::InvalidMaxFires));
    }
    Some(Connection::Parsed {
        order,
        output: pair.key.clone(),
        target: fields[0].to_vec(),
        input: input.to_vec(),
        parameter: fields[2].to_vec(),
        delay_bits: delay.to_bits(),
        max_fires,
    })
}
fn number_f32(bytes: &[u8]) -> Option<f32> {
    let value = std::str::from_utf8(bytes)
        .ok()?
        .trim()
        .parse::<f32>()
        .ok()?;
    value.is_finite().then_some(value)
}
fn integer(bytes: &[u8]) -> Option<i32> {
    std::str::from_utf8(bytes).ok()?.trim().parse().ok()
}
fn runtime_error(code: RuntimeErrorCode, entity: usize) -> RuntimeError {
    RuntimeError { code, entity }
}
fn pair<'a>(entity: &'a Entity, key: &[u8]) -> Option<&'a [u8]> {
    entity
        .pairs
        .iter()
        .find(|pair| pair.key.eq_ignore_ascii_case(key))
        .map(|pair| pair.value.as_slice())
}
fn vector(bytes: &[u8]) -> Option<[f32; 3]> {
    let values = std::str::from_utf8(bytes)
        .ok()?
        .split_ascii_whitespace()
        .map(str::parse::<f32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3 && values.iter().all(|value| value.is_finite()))
        .then(|| [values[0], values[1], values[2]])
}

impl Runtime {
    pub fn empty() -> Self {
        Self {
            collision: None,
            destinations: Vec::new(),
            teleports: Vec::new(),
        }
    }

    pub fn compile(
        graph: &Graph,
        collision: Arc<playsrc_collision::World>,
    ) -> Result<Self, RuntimeError> {
        let mut destinations = Vec::new();
        for entity in &graph.entities {
            if !entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"info_teleport_destination"))
            {
                continue;
            }
            let targetname = entity
                .targetname
                .clone()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| runtime_error(RuntimeErrorCode::InvalidField, entity.index))?;
            let position = pair(entity, b"origin")
                .and_then(vector)
                .ok_or_else(|| runtime_error(RuntimeErrorCode::InvalidField, entity.index))?;
            let angles = match pair(entity, b"angles") {
                Some(value) => vector(value)
                    .ok_or_else(|| runtime_error(RuntimeErrorCode::InvalidField, entity.index))?,
                None => [0.; 3],
            };
            destinations.push(TeleportDestination {
                entity: entity.index,
                targetname,
                position,
                yaw_degrees: angles[1],
            });
        }
        let mut teleports = Vec::new();
        for entity in &graph.entities {
            if !entity
                .classname
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case(b"trigger_teleport"))
            {
                continue;
            }
            match pair(entity, b"StartDisabled") {
                None | Some(b"0") => {}
                Some(b"1") => {
                    return Err(runtime_error(
                        RuntimeErrorCode::UnsupportedState,
                        entity.index,
                    ));
                }
                Some(_) => {
                    return Err(runtime_error(RuntimeErrorCode::InvalidField, entity.index));
                }
            }
            let flags = match entity.spawnflags.as_deref() {
                Some(value) => integer(value)
                    .ok_or_else(|| runtime_error(RuntimeErrorCode::InvalidField, entity.index))?,
                None => 0,
            };
            if flags & 1 == 0 {
                continue;
            }
            if pair(entity, b"landmark").is_some_and(|value| !value.is_empty()) {
                return Err(runtime_error(
                    RuntimeErrorCode::UnsupportedLandmark,
                    entity.index,
                ));
            }
            if pair(entity, b"filtername").is_some_and(|value| !value.is_empty()) {
                return Err(runtime_error(
                    RuntimeErrorCode::UnsupportedFilter,
                    entity.index,
                ));
            }
            if let Some(value) = pair(entity, b"angles") {
                let angles = vector(value)
                    .ok_or_else(|| runtime_error(RuntimeErrorCode::InvalidField, entity.index))?;
                if angles != [0.; 3] {
                    return Err(runtime_error(
                        RuntimeErrorCode::UnsupportedTransform,
                        entity.index,
                    ));
                }
            }
            let model = entity
                .bsp_model_index
                .filter(|index| *index < collision.models.len())
                .ok_or_else(|| runtime_error(RuntimeErrorCode::MissingModel, entity.index))?;
            let target = pair(entity, b"target")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| runtime_error(RuntimeErrorCode::MissingTarget, entity.index))?;
            let destination = destinations
                .iter()
                .position(|value| value.targetname.eq_ignore_ascii_case(target))
                .ok_or_else(|| runtime_error(RuntimeErrorCode::MissingTarget, entity.index))?;
            let origin = match pair(entity, b"origin") {
                Some(value) => vector(value)
                    .ok_or_else(|| runtime_error(RuntimeErrorCode::InvalidField, entity.index))?,
                None => [0.; 3],
            };
            teleports.push(TeleportTrigger {
                entity: entity.index,
                model,
                origin,
                destination,
                preserve_angles: flags & 0x20 != 0,
            });
        }
        Ok(Self {
            collision: Some(collision),
            destinations,
            teleports,
        })
    }

    pub fn teleport(
        &self,
        position: [f32; 3],
        hull: playsrc_collision::Hull,
    ) -> Result<Option<TeleportResult>, RuntimeError> {
        let Some(collision) = &self.collision else {
            return Ok(None);
        };
        for trigger in &self.teleports {
            let overlaps = collision
                .overlaps_model_hull(trigger.model, trigger.origin, position, hull)
                .map_err(|_| runtime_error(RuntimeErrorCode::Collision, trigger.entity))?;
            if !overlaps {
                continue;
            }
            let destination = &self.destinations[trigger.destination];
            return Ok(Some(TeleportResult {
                trigger_entity: trigger.entity,
                destination_entity: destination.entity,
                position: destination.position,
                yaw_degrees: (!trigger.preserve_angles).then_some(destination.yaw_degrees),
            }));
        }
        Ok(None)
    }
}
fn error(code: ErrorCode, range: Range<usize>) -> Error {
    Error { code, range }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_order_repeats_models_and_connections() {
        let bytes=b"{\n\"classname\" \"worldspawn\"\n\"same\" \"a\"\n\"same\" \"b\"\n}\n{\"classname\"\"trigger_multiple\"\"model\"\"*3\"\"OnStartTouch\"\"target,input,param,0.5,-1\"}\0";
        let graph = parse(bytes, Limits::default()).unwrap();
        assert_eq!(graph.entities.len(), 2);
        assert_eq!(graph.entities[0].pairs[1].value, b"a");
        assert_eq!(graph.entities[0].pairs[2].value, b"b");
        assert_eq!(graph.entities[0].bsp_model_index, Some(0));
        assert_eq!(graph.entities[1].bsp_model_index, Some(3));
        assert_eq!(graph.inventory.parsed_connections, 1);
        assert!(graph.suffix.is_empty());
    }
    #[test]
    fn retains_malformed_output() {
        let graph = parse(
            b"{\"classname\"\"logic_relay\"\"OnTrigger\"\"bad\"}\0",
            Limits::default(),
        )
        .unwrap();
        assert!(matches!(
            graph.entities[0].connections[0],
            Connection::Malformed {
                error: ConnectionError::FieldCount,
                ..
            }
        ));
    }
    #[test]
    fn rejects_structure_and_limits() {
        assert_eq!(
            parse(b"x", Limits::default()).unwrap_err().code,
            ErrorCode::UnexpectedToken
        );
        assert_eq!(
            parse(b"{\"a\"}", Limits::default()).unwrap_err().code,
            ErrorCode::MissingValue
        );
        assert_eq!(
            parse(
                b"{}",
                Limits {
                    max_entities: 0,
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::EntityLimit
        );
    }

    #[test]
    fn compiles_and_applies_brush_teleports() {
        use playsrc_bsp::{Float32, Model, Vector3};
        let graph = parse(
            b"{\"classname\"\"info_teleport_destination\"\"targetname\"\"exit\"\"origin\"\"100 200 300\"\"angles\"\"0 90 0\"}{\"classname\"\"trigger_teleport\"\"model\"\"*1\"\"target\"\"exit\"\"origin\"\"0 0 0\"\"spawnflags\"\"1\"}\0",
            Limits::default(),
        )
        .unwrap();
        let planes = [
            ([1., 0., 0.], 10.),
            ([-1., 0., 0.], 10.),
            ([0., 1., 0.], 10.),
            ([0., -1., 0.], 10.),
            ([0., 0., 1.], 10.),
            ([0., 0., -1.], 10.),
        ]
        .map(|(normal, distance)| playsrc_collision::Plane {
            normal,
            distance,
            kind: 0,
        });
        let sides = (0..6)
            .map(|plane| playsrc_collision::Side {
                plane,
                texture_info: -1,
                displacement: -1,
                bevel: 0,
            })
            .collect();
        let model = Model {
            mins: Vector3 {
                x: Float32(0),
                y: Float32(0),
                z: Float32(0),
            },
            maxs: Vector3 {
                x: Float32(0),
                y: Float32(0),
                z: Float32(0),
            },
            origin: Vector3 {
                x: Float32(0),
                y: Float32(0),
                z: Float32(0),
            },
            head_node: -1,
            first_face: 0,
            face_count: 0,
        };
        let collision = Arc::new(playsrc_collision::World {
            planes: planes.to_vec(),
            sides,
            brushes: vec![playsrc_collision::Brush {
                first_side: 0,
                side_count: 6,
                contents: u32::MAX,
            }],
            leaves: Vec::new(),
            leaf_brushes: Vec::new(),
            nodes: Vec::new(),
            models: vec![model, model],
            world_brushes: Vec::new(),
            model_brushes: vec![Vec::new(), vec![0]],
        });
        let runtime = Runtime::compile(&graph, collision).unwrap();
        assert_eq!(runtime.destinations.len(), 1);
        assert_eq!(runtime.teleports.len(), 1);
        assert_eq!(
            runtime
                .teleport(
                    [0.; 3],
                    playsrc_collision::Hull {
                        mins: [0.; 3],
                        maxs: [0.; 3],
                    },
                )
                .unwrap()
                .unwrap(),
            TeleportResult {
                trigger_entity: 1,
                destination_entity: 0,
                position: [100., 200., 300.],
                yaw_degrees: Some(90.),
            }
        );
    }
}
