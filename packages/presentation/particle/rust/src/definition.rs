use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Document, Error, ErrorCode, Value,
    dmx::{self, Limits as DmxLimits},
};

const CATEGORIES: [(&str, FunctionCategory); 6] = [
    ("renderers", FunctionCategory::Renderer),
    ("operators", FunctionCategory::Operator),
    ("initializers", FunctionCategory::Initializer),
    ("emitters", FunctionCategory::Emitter),
    ("forces", FunctionCategory::Force),
    ("constraints", FunctionCategory::Constraint),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryLimits {
    pub max_pcf_bytes: usize,
    pub max_sources: usize,
    pub max_strings: usize,
    pub max_string_bytes: usize,
    pub max_elements_per_source: usize,
    pub max_attributes_per_element: usize,
    pub max_array_items: usize,
    pub max_total_values: usize,
    pub max_definitions: usize,
    pub max_definition_depth: usize,
    pub max_functions_per_category: usize,
    pub max_children_per_system: usize,
}

impl Default for RegistryLimits {
    fn default() -> Self {
        Self {
            max_pcf_bytes: 16 * 1024 * 1024,
            max_sources: 256,
            max_strings: 65_535,
            max_string_bytes: 4_096,
            max_elements_per_source: 65_536,
            max_attributes_per_element: 512,
            max_array_items: 65_536,
            max_total_values: 4_000_000,
            max_definitions: 65_536,
            max_definition_depth: 32,
            max_functions_per_category: 128,
            max_children_per_system: 64,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PcfSource<'a> {
    pub logical_path: &'a str,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum FunctionCategory {
    Renderer,
    Operator,
    Initializer,
    Emitter,
    Force,
    Constraint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub category: FunctionCategory,
    pub identity: String,
    pub element_uuid: [u8; 16],
    pub parameters: Vec<(String, Value)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChildDeclaration {
    pub element_uuid: [u8; 16],
    pub definition_uuid: [u8; 16],
    pub definition_name: String,
    pub name_lookup: bool,
    pub delay_seconds: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub name: String,
    pub uuid: [u8; 16],
    pub name_lookup: bool,
    pub source: String,
    pub material: String,
    pub attributes: Vec<(String, Value)>,
    pub functions: Vec<Function>,
    pub children: Vec<ChildDeclaration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionLookup<'a> {
    Name(&'a str),
    Uuid([u8; 16]),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetClosure {
    pub definitions: Vec<[u8; 16]>,
    pub materials: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Registry {
    definitions: Vec<Definition>,
    by_uuid: BTreeMap<[u8; 16], usize>,
    by_name: BTreeMap<String, usize>,
    shadowed_names: Vec<(String, [u8; 16], [u8; 16])>,
    limits: RegistryLimits,
}

impl Registry {
    pub fn from_pcf(sources: &[PcfSource<'_>], limits: RegistryLimits) -> Result<Self, Error> {
        validate_limits(limits)?;
        if sources.len() > limits.max_sources {
            return Err(Error::new(
                ErrorCode::BoundExceeded,
                "particle-registry",
                0,
                "PCF source count exceeds max_sources",
            ));
        }
        let mut registry = Self {
            definitions: Vec::new(),
            by_uuid: BTreeMap::new(),
            by_name: BTreeMap::new(),
            shadowed_names: Vec::new(),
            limits,
        };
        for source in sources {
            validate_logical_path(source.logical_path)?;
            let document = dmx::parse(
                source.bytes,
                source.logical_path,
                DmxLimits {
                    max_bytes: limits.max_pcf_bytes,
                    max_strings: limits.max_strings,
                    max_string_bytes: limits.max_string_bytes,
                    max_elements: limits.max_elements_per_source,
                    max_attributes_per_element: limits.max_attributes_per_element,
                    max_array_items: limits.max_array_items,
                    max_total_values: limits.max_total_values,
                },
            )?;
            registry.add_document(source.logical_path, document)?;
        }
        Ok(registry)
    }

    pub fn definitions(&self) -> &[Definition] {
        &self.definitions
    }

    pub fn shadowed_names(&self) -> &[(String, [u8; 16], [u8; 16])] {
        &self.shadowed_names
    }

    pub fn definition(&self, lookup: DefinitionLookup<'_>) -> Option<&Definition> {
        let index = match lookup {
            DefinitionLookup::Name(name) => self.by_name.get(&name.to_ascii_lowercase()),
            DefinitionLookup::Uuid(uuid) => self.by_uuid.get(&uuid),
        }?;
        self.definitions.get(*index)
    }

    pub fn definition_by_uuid(&self, uuid: [u8; 16]) -> Option<&Definition> {
        self.by_uuid
            .get(&uuid)
            .and_then(|index| self.definitions.get(*index))
    }

    pub fn target_closure(&self, roots: &[DefinitionLookup<'_>]) -> Result<TargetClosure, Error> {
        let mut ordered = Vec::new();
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for root in roots {
            let definition = self.definition(root.clone()).ok_or_else(|| {
                Error::new(
                    ErrorCode::MissingDefinition,
                    "particle-registry",
                    0,
                    "requested particle definition is missing",
                )
            })?;
            self.visit(
                definition.uuid,
                0,
                &mut visiting,
                &mut visited,
                &mut ordered,
            )?;
        }
        let mut materials = BTreeSet::new();
        for uuid in &ordered {
            let definition = self.definition_by_uuid(*uuid).expect("visited definition");
            if !definition.material.is_empty() {
                materials.insert(definition.material.clone());
            }
        }
        Ok(TargetClosure {
            definitions: ordered,
            materials: materials.into_iter().collect(),
        })
    }

    fn visit(
        &self,
        uuid: [u8; 16],
        depth: usize,
        visiting: &mut BTreeSet<[u8; 16]>,
        visited: &mut BTreeSet<[u8; 16]>,
        ordered: &mut Vec<[u8; 16]>,
    ) -> Result<(), Error> {
        if visited.contains(&uuid) {
            return Ok(());
        }
        if depth > self.limits.max_definition_depth {
            return Err(Error::new(
                ErrorCode::BoundExceeded,
                "particle-registry",
                0,
                "definition closure exceeds max_definition_depth",
            ));
        }
        if !visiting.insert(uuid) {
            return Err(Error::new(
                ErrorCode::DefinitionCycle,
                "particle-registry",
                0,
                "definition child graph contains a cycle",
            ));
        }
        let definition = self.definition_by_uuid(uuid).ok_or_else(|| {
            Error::new(
                ErrorCode::MissingDefinition,
                "particle-registry",
                0,
                "child particle definition is missing",
            )
        })?;
        for function in &definition.functions {
            if !supported(function.category, &function.identity) {
                return Err(Error::new(
                    ErrorCode::UnsupportedFunction,
                    &definition.source,
                    0,
                    format!(
                        "{} is not executable by the projectile target",
                        function.identity
                    ),
                ));
            }
            validate_function(function, definition)?;
        }
        ordered.push(uuid);
        for child in &definition.children {
            let child_definition = self.child_definition(child).ok_or_else(|| {
                Error::new(
                    ErrorCode::MissingDefinition,
                    &definition.source,
                    0,
                    "child particle definition is missing",
                )
            })?;
            self.visit(child_definition.uuid, depth + 1, visiting, visited, ordered)?;
        }
        visiting.remove(&uuid);
        visited.insert(uuid);
        Ok(())
    }

    fn add_document(&mut self, source: &str, document: Document) -> Result<(), Error> {
        let Some(root) = document.elements.first() else {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                source,
                0,
                "PCF contains no root element",
            ));
        };
        let definition_indexes = references(root.attribute("particleSystemDefinitions"), source)?;
        for definition_index in definition_indexes {
            if self.definitions.len() >= self.limits.max_definitions {
                return Err(Error::new(
                    ErrorCode::BoundExceeded,
                    source,
                    0,
                    "definition count exceeds max_definitions",
                ));
            }
            let element = document.elements.get(definition_index).ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidReference,
                    source,
                    0,
                    "definition index is invalid",
                )
            })?;
            if !element
                .element_type
                .eq_ignore_ascii_case("DmeParticleSystemDefinition")
            {
                return Err(Error::new(
                    ErrorCode::InvalidType,
                    source,
                    0,
                    "particleSystemDefinitions references a non-definition element",
                ));
            }
            let definition = compile_definition(source, &document, definition_index, self.limits)?;
            if self.by_uuid.contains_key(&definition.uuid) {
                return Err(Error::new(
                    ErrorCode::InvalidValue,
                    source,
                    0,
                    "definition UUID is duplicated",
                ));
            }
            let next_index = self.definitions.len();
            if definition.name_lookup {
                let canonical = definition.name.to_ascii_lowercase();
                if let Some(previous) = self.by_name.insert(canonical.clone(), next_index) {
                    self.shadowed_names.push((
                        canonical,
                        self.definitions[previous].uuid,
                        definition.uuid,
                    ));
                }
            }
            self.by_uuid.insert(definition.uuid, next_index);
            self.definitions.push(definition);
        }
        Ok(())
    }

    pub(crate) fn child_definition(&self, child: &ChildDeclaration) -> Option<&Definition> {
        if child.name_lookup {
            self.definition(DefinitionLookup::Name(&child.definition_name))
        } else {
            self.definition_by_uuid(child.definition_uuid)
        }
    }
}

impl Definition {
    pub fn attribute(&self, name: &str) -> Option<&Value> {
        self.attributes
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    }

    pub fn functions(&self, category: FunctionCategory) -> impl Iterator<Item = &Function> {
        self.functions
            .iter()
            .filter(move |function| function.category == category)
    }
}

impl Function {
    pub fn parameter(&self, name: &str) -> Option<&Value> {
        self.parameters
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
    }
}

fn compile_definition(
    source: &str,
    document: &Document,
    index: usize,
    limits: RegistryLimits,
) -> Result<Definition, Error> {
    let element = &document.elements[index];
    if element.name.is_empty() {
        return Err(Error::new(
            ErrorCode::InvalidString,
            source,
            0,
            "particle definition name is empty",
        ));
    }
    let name_lookup = match element.attribute("preventNameBasedLookup") {
        Some(Value::Bool(value)) => !*value,
        None => true,
        _ => {
            return Err(Error::new(
                ErrorCode::InvalidType,
                source,
                0,
                "preventNameBasedLookup is not a bool",
            ));
        }
    };
    let material = match element.attribute("material") {
        Some(Value::String(value)) => normalize_material(value, source)?,
        None => String::new(),
        _ => {
            return Err(Error::new(
                ErrorCode::InvalidType,
                source,
                0,
                "definition material is not a string",
            ));
        }
    };
    let category_names: BTreeSet<&str> = CATEGORIES.iter().map(|(name, _)| *name).collect();
    let mut attributes = Vec::new();
    for attribute in &element.attributes {
        if !category_names
            .iter()
            .any(|name| attribute.name.eq_ignore_ascii_case(name))
            && !attribute.name.eq_ignore_ascii_case("children")
            && !attribute.name.eq_ignore_ascii_case("material")
        {
            attributes.push((attribute.name.clone(), attribute.value.clone()));
        }
    }
    let mut functions = Vec::new();
    for (attribute, category) in CATEGORIES {
        let indexes = references(element.attribute(attribute), source)?;
        if indexes.len() > limits.max_functions_per_category {
            return Err(Error::new(
                ErrorCode::BoundExceeded,
                source,
                0,
                "function category exceeds max_functions_per_category",
            ));
        }
        for function_index in indexes {
            let function = document.elements.get(function_index).ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidReference,
                    source,
                    0,
                    "function index is invalid",
                )
            })?;
            let identity = match function.attribute("functionName") {
                Some(Value::String(value)) if !value.is_empty() => value.clone(),
                Some(_) => {
                    return Err(Error::new(
                        ErrorCode::InvalidType,
                        source,
                        0,
                        "functionName is not a nonempty string",
                    ));
                }
                None if !function.name.is_empty() => function.name.clone(),
                None => {
                    return Err(Error::new(
                        ErrorCode::InvalidString,
                        source,
                        0,
                        "function identity is empty",
                    ));
                }
            };
            let parameters = function
                .attributes
                .iter()
                .filter(|attribute| !attribute.name.eq_ignore_ascii_case("functionName"))
                .map(|attribute| (attribute.name.clone(), attribute.value.clone()))
                .collect();
            functions.push(Function {
                category,
                identity,
                element_uuid: function.uuid,
                parameters,
            });
        }
    }
    let child_indexes = references(element.attribute("children"), source)?;
    if child_indexes.len() > limits.max_children_per_system {
        return Err(Error::new(
            ErrorCode::BoundExceeded,
            source,
            0,
            "child count exceeds max_children_per_system",
        ));
    }
    let mut children = Vec::with_capacity(child_indexes.len());
    for child_index in child_indexes {
        let child = document.elements.get(child_index).ok_or_else(|| {
            Error::new(
                ErrorCode::InvalidReference,
                source,
                0,
                "child index is invalid",
            )
        })?;
        let definition_index = reference(child.attribute("child"), source)?.ok_or_else(|| {
            Error::new(
                ErrorCode::MissingDefinition,
                source,
                0,
                "child definition is null",
            )
        })?;
        let definition = document.elements.get(definition_index).ok_or_else(|| {
            Error::new(
                ErrorCode::InvalidReference,
                source,
                0,
                "child definition index is invalid",
            )
        })?;
        let delay_seconds = match child.attribute("delay") {
            Some(Value::Float(value)) if *value >= 0.0 => *value,
            None => 0.0,
            _ => {
                return Err(Error::new(
                    ErrorCode::InvalidValue,
                    source,
                    0,
                    "child delay is not a finite non-negative float",
                ));
            }
        };
        children.push(ChildDeclaration {
            element_uuid: child.uuid,
            definition_uuid: definition.uuid,
            definition_name: definition.name.clone(),
            name_lookup: match definition.attribute("preventNameBasedLookup") {
                Some(Value::Bool(value)) => !*value,
                None => true,
                _ => {
                    return Err(Error::new(
                        ErrorCode::InvalidType,
                        source,
                        0,
                        "child preventNameBasedLookup is not a bool",
                    ));
                }
            },
            delay_seconds,
        });
    }
    Ok(Definition {
        name: element.name.clone(),
        uuid: element.uuid,
        name_lookup,
        source: source.to_owned(),
        material,
        attributes,
        functions,
        children,
    })
}

fn references(value: Option<&Value>, source: &str) -> Result<Vec<usize>, Error> {
    match value {
        None => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| match value {
                Value::Element(Some(index)) => Ok(*index),
                _ => Err(Error::new(
                    ErrorCode::InvalidType,
                    source,
                    0,
                    "element array contains a null or non-reference value",
                )),
            })
            .collect(),
        _ => Err(Error::new(
            ErrorCode::InvalidType,
            source,
            0,
            "expected an element-reference array",
        )),
    }
}

fn reference(value: Option<&Value>, source: &str) -> Result<Option<usize>, Error> {
    match value {
        Some(Value::Element(index)) => Ok(*index),
        _ => Err(Error::new(
            ErrorCode::InvalidType,
            source,
            0,
            "expected an element reference",
        )),
    }
}

fn normalize_material(value: &str, source: &str) -> Result<String, Error> {
    let value = value.replace('\\', "/").to_ascii_lowercase();
    if value.is_empty()
        || value.starts_with('/')
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
        || value.chars().any(char::is_control)
    {
        return Err(Error::new(
            ErrorCode::InvalidString,
            source,
            0,
            "material logical path is invalid",
        ));
    }
    Ok(value)
}

fn validate_logical_path(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 1_024
        || value.starts_with('/')
        || value.contains('\\')
        || value
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == "..")
        || value.chars().any(char::is_control)
    {
        return Err(Error::new(
            ErrorCode::InvalidString,
            "particle-registry",
            0,
            "PCF logical path is invalid",
        ));
    }
    Ok(())
}

fn validate_limits(limits: RegistryLimits) -> Result<(), Error> {
    let values = [
        limits.max_pcf_bytes,
        limits.max_sources,
        limits.max_strings,
        limits.max_string_bytes,
        limits.max_elements_per_source,
        limits.max_attributes_per_element,
        limits.max_array_items,
        limits.max_total_values,
        limits.max_definitions,
        limits.max_definition_depth,
        limits.max_functions_per_category,
        limits.max_children_per_system,
    ];
    if values.contains(&0) {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "particle-registry",
            0,
            "registry limits must be positive",
        ));
    }
    Ok(())
}

fn supported(category: FunctionCategory, identity: &str) -> bool {
    let names: &[&str] = match category {
        FunctionCategory::Renderer => &["render_animated_sprites", "render_sprite_trail"],
        FunctionCategory::Operator => &[
            "Alpha Fade and Decay",
            "Alpha Fade Out Random",
            "Color Fade",
            "Lifespan Decay",
            "Movement Basic",
            "Movement Follow CP",
            "Movement Lock to Control Point",
            "Oscillate Scalar",
            "Oscillate Vector",
            "Radius Scale",
            "Remap Distance to Control Point to Scalar",
            "Remap Distance to Control Point to Vector",
            "Rotation Basic",
            "Rotation Spin Roll",
        ],
        FunctionCategory::Initializer => &[
            "Alpha Random",
            "Assign target CP",
            "Color Random",
            "Lifetime From Control Point Life Time",
            "Lifetime Pre-Age Noise",
            "Lifetime Random",
            "Move Particles Between 2 Control Points",
            "Position Modify Offset Random",
            "Position Within Box Random",
            "Position Within Sphere Random",
            "Radius Random",
            "Remap Control Point to Vector",
            "Remap Initial Scalar",
            "Remap Scalar to Vector",
            "Rotation Random",
            "Rotation Speed Random",
            "Rotation Yaw Flip Random",
            "Sequence Random",
            "Trail Length Random",
            "Velocity Noise",
        ],
        FunctionCategory::Emitter => &["emit_continuously", "emit_instantaneously"],
        FunctionCategory::Force => &["random force"],
        FunctionCategory::Constraint => &["Collision via traces"],
    };
    names
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(identity))
}

fn validate_function(function: &Function, definition: &Definition) -> Result<(), Error> {
    for (name, value) in &function.parameters {
        let name = name.to_ascii_lowercase();
        if !accepted_parameter(function, &name) {
            return Err(Error::new(
                ErrorCode::UnsupportedFunction,
                &definition.source,
                0,
                format!(
                    "function {} parameter {} is not in the target executable schema",
                    function.identity, name
                ),
            ));
        }
        let valid = if [
            "animation_fit_lifetime",
            "use animation rate as fps",
            "ease_in_and_out",
            "ease in and out",
            "randomly distribute to highest supplied control point",
            "bias in local system",
            "use parent particles for emission scaling",
            "randomly_flip_direction",
            "offset proportional to radius 0/1",
            "offset in local space 0/1",
            "proportional 0/1",
            "start/end proportional",
            "absolute oscillation",
            "lock rotation",
            "update particle life time",
            "ensure line of sight",
            "only active within specified distance",
            "only active within specified input range",
            "output is scalar of initial random range",
            "use local system",
            "brush only",
            "kill particle on collision",
            "use bounding box",
            "use local system",
            "output is scalar of initial random range",
            "only active within specified input range",
            "apply velocity in local space (0/1)",
            "offset position",
            "accelerate position",
            "invert absolute value",
        ]
        .contains(&name.as_str())
            || (name == "absolute value"
                && function
                    .identity
                    .eq_ignore_ascii_case("Lifetime Pre-Age Noise"))
        {
            matches!(value, Value::Bool(_))
        } else if [
            "orientation_type",
            "visibility proxy input control point number",
            "orientation control point",
            "max constraint passes",
            "create in model",
            "control_point_number",
            "control point number",
            "control point",
            "starting control point",
            "maximum end control point",
            "end control point",
            "input control point number",
            "local space cp",
            "input field",
            "output field",
            "tint control point",
            "alpha_min",
            "alpha_max",
            "sequence_min",
            "sequence_max",
            "spin_rate_min",
            "spin_rate_degrees",
            "oscillation field",
            "maximum emission per frame",
            "num_to_emit",
            "num_to_emit_minimum",
            "emission count scale control point",
            "emission count scale control point field",
            "collision mode",
            "input field",
            "output field",
        ]
        .contains(&name.as_str())
        {
            matches!(value, Value::Int(_))
        } else if [
            "gravity",
            "distance_bias",
            "distance_bias_absolute_value",
            "speed_in_local_coordinate_system_min",
            "speed_in_local_coordinate_system_max",
            "offset min",
            "offset max",
            "min",
            "max",
            "min force",
            "max force",
            "control point offset for fast collisions",
            "spatial coordinate offset",
            "invert abs value",
        ]
        .contains(&name.as_str())
            || (name == "absolute value"
                && !function
                    .identity
                    .eq_ignore_ascii_case("Lifetime Pre-Age Noise"))
            || ([
                "oscillation rate min",
                "oscillation rate max",
                "oscillation frequency min",
                "oscillation frequency max",
            ]
            .contains(&name.as_str())
                && function.identity.eq_ignore_ascii_case("Oscillate Vector"))
            || (["output minimum", "output maximum"].contains(&name.as_str())
                && (function
                    .identity
                    .eq_ignore_ascii_case("Remap Scalar to Vector")
                    || function
                        .identity
                        .eq_ignore_ascii_case("Remap Distance to Control Point to Vector")
                    || function.identity.eq_ignore_ascii_case("Velocity Noise")
                    || function
                        .identity
                        .eq_ignore_ascii_case("Remap Control Point to Vector")))
            || (["input minimum", "input maximum"].contains(&name.as_str())
                && function
                    .identity
                    .eq_ignore_ascii_case("Remap Control Point to Vector"))
        {
            matches!(value, Value::Vector3(_))
        } else if [
            "color1",
            "color2",
            "color_fade",
            "tint clamp min",
            "tint clamp max",
        ]
        .contains(&name.as_str())
        {
            matches!(value, Value::Color(_))
        } else if name == "collision group" || name == "los collision group" {
            matches!(value, Value::String(_))
        } else {
            matches!(value, Value::Float(_))
        };
        if !valid {
            return Err(Error::new(
                ErrorCode::InvalidType,
                &definition.source,
                0,
                format!(
                    "function {} parameter {} has the wrong DMX type",
                    function.identity, name
                ),
            ));
        }
    }
    let unsupported = if function.category == FunctionCategory::Renderer {
        int_parameter(function, "Visibility Proxy Input Control Point Number", -1) >= 0
            || (function
                .identity
                .eq_ignore_ascii_case("render_animated_sprites")
                && (int_parameter(function, "orientation_type", 0) != 0
                    || int_parameter(function, "orientation control point", -1) >= 0))
    } else if function
        .identity
        .eq_ignore_ascii_case("Position Within Sphere Random")
    {
        int_parameter(function, "create in model", 0) != 0
            || bool_parameter(
                function,
                "randomly distribute to highest supplied Control Point",
                false,
            )
    } else if function
        .identity
        .eq_ignore_ascii_case("Movement Lock to Control Point")
    {
        float_parameter(function, "distance fade range", 0.0) != 0.0
    } else if function.identity.eq_ignore_ascii_case("emit_continuously") {
        float_parameter(function, "scale emission to used control points", 0.0) != 0.0
            || bool_parameter(function, "use parent particles for emission scaling", false)
    } else if function
        .identity
        .eq_ignore_ascii_case("emit_instantaneously")
    {
        int_parameter(function, "emission count scale control point", -1) >= 0
    } else if function
        .identity
        .eq_ignore_ascii_case("Collision via traces")
    {
        !matches!(int_parameter(function, "collision mode", 0), 0..=3)
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Initial Scalar")
    {
        int_parameter(function, "input field", 8) != 8
            || !matches!(int_parameter(function, "output field", 3), 1 | 3 | 7)
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Scalar to Vector")
    {
        int_parameter(function, "input field", 8) != 8
            || int_parameter(function, "output field", 0) != 0
    } else {
        false
    };
    if unsupported {
        return Err(Error::new(
            ErrorCode::UnsupportedFunction,
            &definition.source,
            0,
            format!(
                "function {} uses a parameter combination outside the exact projectile closure",
                function.identity
            ),
        ));
    }
    Ok(())
}

fn int_parameter(function: &Function, name: &str, default: i32) -> i32 {
    match function.parameter(name) {
        Some(Value::Int(value)) => *value,
        _ => default,
    }
}

fn float_parameter(function: &Function, name: &str, default: f32) -> f32 {
    match function.parameter(name) {
        Some(Value::Float(value)) => *value,
        _ => default,
    }
}

fn bool_parameter(function: &Function, name: &str, default: bool) -> bool {
    match function.parameter(name) {
        Some(Value::Bool(value)) => *value,
        _ => default,
    }
}

fn accepted_parameter(function: &Function, name: &str) -> bool {
    if [
        "operator start fadein",
        "operator end fadein",
        "operator start fadeout",
        "operator end fadeout",
        "operator fade oscillate",
    ]
    .contains(&name)
    {
        return true;
    }
    if function.category == FunctionCategory::Renderer
        && [
            "visibility proxy input control point number",
            "visibility proxy radius",
            "visibility input minimum",
            "visibility input maximum",
            "visibility alpha scale minimum",
            "visibility alpha scale maximum",
            "visibility radius scale minimum",
            "visibility radius scale maximum",
            "visibility camera depth bias",
        ]
        .contains(&name)
    {
        return true;
    }
    let names: &[&str] = if function
        .identity
        .eq_ignore_ascii_case("render_animated_sprites")
    {
        &[
            "animation rate",
            "animation_fit_lifetime",
            "orientation_type",
            "orientation control point",
            "second sequence animation rate",
            "use animation rate as fps",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("render_sprite_trail")
    {
        &[
            "animation rate",
            "length fade in time",
            "max length",
            "min length",
        ]
    } else if function.identity.eq_ignore_ascii_case("Movement Basic") {
        &["gravity", "drag", "max constraint passes"]
    } else if function
        .identity
        .eq_ignore_ascii_case("Alpha Fade and Decay")
    {
        &[
            "start_alpha",
            "end_alpha",
            "start_fade_in_time",
            "end_fade_in_time",
            "start_fade_out_time",
            "end_fade_out_time",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Alpha Fade Out Random")
    {
        &[
            "fade out time min",
            "fade out time max",
            "fade out time exponent",
            "proportional 0/1",
            "ease in and out",
            "fade bias",
        ]
    } else if function.identity.eq_ignore_ascii_case("Lifespan Decay") {
        &[]
    } else if function.identity.eq_ignore_ascii_case("Radius Scale") {
        &[
            "start_time",
            "end_time",
            "radius_start_scale",
            "radius_end_scale",
            "ease_in_and_out",
            "scale_bias",
        ]
    } else if function.identity.eq_ignore_ascii_case("Color Fade") {
        &[
            "color_fade",
            "fade_start_time",
            "fade_end_time",
            "ease_in_and_out",
        ]
    } else if function.identity.eq_ignore_ascii_case("Rotation Basic") {
        &[]
    } else if function.identity.eq_ignore_ascii_case("Rotation Spin Roll") {
        &["spin_rate_degrees", "spin_stop_time", "spin_rate_min"]
    } else if function
        .identity
        .eq_ignore_ascii_case("Movement Lock to Control Point")
    {
        &[
            "control_point_number",
            "start_fadeout_min",
            "start_fadeout_max",
            "start_fadeout_exponent",
            "end_fadeout_min",
            "end_fadeout_max",
            "end_fadeout_exponent",
            "distance fade range",
            "lock rotation",
        ]
    } else if function.identity.eq_ignore_ascii_case("Movement Follow CP") {
        &[
            "starting control point",
            "maximum end control point",
            "catch up speed",
            "lerp to cp radius speed",
            "update particle life time",
        ]
    } else if function.identity.eq_ignore_ascii_case("Oscillate Vector") {
        &[
            "oscillation field",
            "oscillation rate min",
            "oscillation rate max",
            "oscillation frequency min",
            "oscillation frequency max",
            "proportional 0/1",
            "start time min",
            "start time max",
            "end time min",
            "end time max",
            "start/end proportional",
            "oscillation multiplier",
            "oscillation start phase",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Distance to Control Point to Scalar")
    {
        &[
            "distance minimum",
            "distance maximum",
            "output field",
            "output minimum",
            "output maximum",
            "control point",
            "ensure line of sight",
            "los collision group",
            "maximum trace length",
            "los failure scalar",
            "output is scalar of initial random range",
            "only active within specified distance",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Distance to Control Point to Vector")
    {
        &[
            "distance minimum",
            "distance maximum",
            "output field",
            "output minimum",
            "output maximum",
            "control point",
            "local space cp",
            "only active within specified distance",
        ]
    } else if function.identity.eq_ignore_ascii_case("Oscillate Scalar") {
        &[
            "oscillation field",
            "oscillation rate min",
            "oscillation rate max",
            "oscillation rate exponent",
            "oscillation frequency min",
            "oscillation frequency max",
            "oscillation frequency exponent",
            "proportional 0/1",
            "start time min",
            "start time max",
            "start time exponent",
            "end time min",
            "end time max",
            "end time exponent",
            "start/end proportional",
            "oscillation multiplier",
            "oscillation start phase",
            "absolute oscillation",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Initial Scalar")
    {
        &[
            "emitter lifetime start time (seconds)",
            "emitter lifetime end time (seconds)",
            "input field",
            "input minimum",
            "input maximum",
            "output field",
            "output minimum",
            "output maximum",
            "output is scalar of initial random range",
            "only active within specified input range",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Scalar to Vector")
    {
        &[
            "emitter lifetime start time (seconds)",
            "emitter lifetime end time (seconds)",
            "input field",
            "input minimum",
            "input maximum",
            "output field",
            "output minimum",
            "output maximum",
            "output is scalar of initial random range",
            "use local system",
            "control_point_number",
        ]
    } else if function.identity.eq_ignore_ascii_case("Assign target CP")
        || function
            .identity
            .eq_ignore_ascii_case("Lifetime From Control Point Life Time")
    {
        &["starting control point", "maximum end control point"]
    } else if function.identity.eq_ignore_ascii_case("Lifetime Random") {
        &["lifetime_min", "lifetime_max", "lifetime_random_exponent"]
    } else if function.identity.eq_ignore_ascii_case("Radius Random") {
        &["radius_min", "radius_max", "radius_random_exponent"]
    } else if function.identity.eq_ignore_ascii_case("Alpha Random") {
        &["alpha_min", "alpha_max", "alpha_random_exponent"]
    } else if function.identity.eq_ignore_ascii_case("Color Random") {
        &[
            "color1",
            "color2",
            "tint_perc",
            "tint control point",
            "tint clamp min",
            "tint clamp max",
            "tint update movement threshold",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Move Particles Between 2 Control Points")
    {
        &[
            "minimum speed",
            "maximum speed",
            "end spread",
            "start offset",
            "end control point",
        ]
    } else if function.identity.eq_ignore_ascii_case("Velocity Noise") {
        &[
            "control point number",
            "time noise coordinate scale",
            "spatial noise coordinate scale",
            "time coordinate offset",
            "spatial coordinate offset",
            "absolute value",
            "invert abs value",
            "output minimum",
            "output maximum",
            "apply velocity in local space (0/1)",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Lifetime Pre-Age Noise")
    {
        &[
            "time noise coordinate scale",
            "spatial noise coordinate scale",
            "time coordinate offset",
            "spatial coordinate offset",
            "absolute value",
            "invert absolute value",
            "start age minimum",
            "start age maximum",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Control Point to Vector")
    {
        &[
            "emitter lifetime start time (seconds)",
            "emitter lifetime end time (seconds)",
            "input control point number",
            "input minimum",
            "input maximum",
            "output field",
            "output minimum",
            "output maximum",
            "output is scalar of initial random range",
            "offset position",
            "accelerate position",
            "local space cp",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Rotation Yaw Flip Random")
    {
        &["flip percentage"]
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Initial Scalar")
    {
        &[
            "emitter lifetime start time (seconds)",
            "emitter lifetime end time (seconds)",
            "input field",
            "input minimum",
            "input maximum",
            "output field",
            "output minimum",
            "output maximum",
            "output is scalar of initial random range",
            "only active within specified input range",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Remap Scalar to Vector")
    {
        &[
            "emitter lifetime start time (seconds)",
            "emitter lifetime end time (seconds)",
            "input field",
            "input minimum",
            "input maximum",
            "output field",
            "output minimum",
            "output maximum",
            "output is scalar of initial random range",
            "use local system",
            "control_point_number",
        ]
    } else if function.identity.eq_ignore_ascii_case("Rotation Random") {
        &[
            "rotation_initial",
            "rotation_offset_min",
            "rotation_offset_max",
            "rotation_random_exponent",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Rotation Speed Random")
    {
        &[
            "rotation_speed_constant",
            "rotation_speed_random_min",
            "rotation_speed_random_max",
            "rotation_speed_random_exponent",
            "randomly_flip_direction",
        ]
    } else if function.identity.eq_ignore_ascii_case("Sequence Random") {
        &["sequence_min", "sequence_max"]
    } else if function
        .identity
        .eq_ignore_ascii_case("Trail Length Random")
    {
        &["length_min", "length_max", "length_random_exponent"]
    } else if function
        .identity
        .eq_ignore_ascii_case("Position Within Box Random")
    {
        &["min", "max", "control point number"]
    } else if function
        .identity
        .eq_ignore_ascii_case("Position Within Sphere Random")
    {
        &[
            "distance_min",
            "distance_max",
            "distance_bias",
            "distance_bias_absolute_value",
            "bias in local system",
            "control_point_number",
            "speed_min",
            "speed_max",
            "speed_random_exponent",
            "speed_in_local_coordinate_system_min",
            "speed_in_local_coordinate_system_max",
            "create in model",
            "randomly distribute to highest supplied control point",
            "randomly distribution growth time",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("Position Modify Offset Random")
    {
        &[
            "control_point_number",
            "offset min",
            "offset max",
            "offset in local space 0/1",
            "offset proportional to radius 0/1",
        ]
    } else if function.identity.eq_ignore_ascii_case("emit_continuously") {
        &[
            "emission_start_time",
            "emission_rate",
            "emission_duration",
            "scale emission to used control points",
            "use parent particles for emission scaling",
        ]
    } else if function
        .identity
        .eq_ignore_ascii_case("emit_instantaneously")
    {
        &[
            "emission_start_time",
            "num_to_emit_minimum",
            "num_to_emit",
            "maximum emission per frame",
            "emission count scale control point",
            "emission count scale control point field",
        ]
    } else if function.identity.eq_ignore_ascii_case("random force") {
        &["min force", "max force"]
    } else if function
        .identity
        .eq_ignore_ascii_case("Collision via traces")
    {
        &[
            "collision mode",
            "amount of bounce",
            "amount of slide",
            "radius scale",
            "brush only",
            "collision group",
            "control point offset for fast collisions",
            "control point movement distance tolerance",
            "kill particle on collision",
            "trace accuracy tolerance",
        ]
    } else {
        &[]
    };
    names.contains(&name)
}
