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
    pub delay_seconds: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    pub name: String,
    pub uuid: [u8; 16],
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
            self.visit(child.definition_uuid, depth + 1, visiting, visited, ordered)?;
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
            let canonical = definition.name.to_ascii_lowercase();
            let next_index = self.definitions.len();
            if let Some(previous) = self.by_name.insert(canonical.clone(), next_index) {
                self.shadowed_names.push((
                    canonical,
                    self.definitions[previous].uuid,
                    definition.uuid,
                ));
            }
            self.by_uuid.insert(definition.uuid, next_index);
            self.definitions.push(definition);
        }
        Ok(())
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
            delay_seconds,
        });
    }
    Ok(Definition {
        name: element.name.clone(),
        uuid: element.uuid,
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
            "Color Fade",
            "Movement Basic",
            "Movement Lock to Control Point",
            "Oscillate Scalar",
            "Radius Scale",
            "Rotation Basic",
            "Rotation Spin Roll",
        ],
        FunctionCategory::Initializer => &[
            "Alpha Random",
            "Color Random",
            "Lifetime Random",
            "Position Modify Offset Random",
            "Position Within Box Random",
            "Position Within Sphere Random",
            "Radius Random",
            "Rotation Random",
            "Rotation Speed Random",
            "Sequence Random",
            "Trail Length Random",
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
        let valid = if [
            "animation_fit_lifetime",
            "use animation rate as fps",
            "ease_in_and_out",
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
            "brush only",
            "kill particle on collision",
            "use bounding box",
        ]
        .contains(&name.as_str())
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
        ]
        .contains(&name.as_str())
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
        } else if name == "collision group" {
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
    Ok(())
}
