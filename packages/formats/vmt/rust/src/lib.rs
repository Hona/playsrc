use std::{collections::BTreeSet, fmt};

use playsrc_keyvalues::{
    Condition, ConditionEnvironment, EscapeMode, Limits as KeyValuesLimits, Node, ParseError,
    Scalar, Span, SyntaxDocument, Token, Value, parse_text,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_document_bytes: usize,
    pub max_aggregate_dependency_bytes: usize,
    pub max_keyvalues_depth: usize,
    pub max_nodes: usize,
    pub max_patch_depth: usize,
    pub max_dependencies: usize,
    pub max_composition_steps: usize,
    pub max_owned_bytes: usize,
    pub max_diagnostics: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_document_bytes: 4 * 1024 * 1024,
            max_aggregate_dependency_bytes: 64 * 1024 * 1024,
            max_keyvalues_depth: 64,
            max_nodes: 262_144,
            max_patch_depth: 10,
            max_dependencies: 10,
            max_composition_steps: 1_000_000,
            max_owned_bytes: 128 * 1024 * 1024,
            max_diagnostics: 128,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Material,
    Patch,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub identity: String,
    pub syntax: SyntaxDocument,
    pub role: Role,
    pub root: Node,
}

impl Document {
    pub fn shader(&self) -> &[u8] {
        &self.root.key.bytes
    }

    pub fn first(&self, key: &[u8]) -> Option<&Node> {
        self.root.first_child(key)
    }

    pub fn active_proxies(&self) -> Option<&[Node]> {
        let node = self.first(b"Proxies")?;
        let Value::Object(children) = &node.value else {
            return None;
        };
        Some(children)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyRequest {
    pub parent_identity: String,
    pub target_token: Vec<u8>,
    pub include_span: Span,
    pub dependency_chain: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyResponse {
    pub parent_identity: String,
    pub target_token: Vec<u8>,
    pub canonical_identity: String,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Base,
    Insert,
    Replace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Origin {
    pub source_identity: String,
    pub source_span: Span,
    pub operation: Operation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EffectiveValue {
    Scalar(Scalar),
    Object(Vec<EffectiveNode>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveNode {
    pub key: Token,
    pub value: EffectiveValue,
    pub condition: Option<Condition>,
    pub origin: Origin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceKind {
    Dependency,
    InertMember,
    InsertUpdated,
    InsertAppended,
    ReplaceUpdated,
    ReplaceInert,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceStep {
    pub kind: TraceKind,
    pub source_identity: String,
    pub key_path: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectiveDocument {
    pub root: EffectiveNode,
    pub source_documents: Vec<Document>,
    pub trace: Vec<TraceStep>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Composition {
    Needs(Vec<DependencyRequest>),
    Complete(Box<EffectiveDocument>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidLimits,
    InvalidIdentity,
    KeyValues,
    RootCount,
    RootKind,
    UnsupportedDirective,
    UnmappedCondition,
    PatchInclude,
    PatchMemberKind,
    MissingDependency,
    DependencyCycle,
    DependencyLimit,
    AggregateByteLimit,
    CompositionLimit,
    OwnedByteLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub identity: String,
    pub span: Option<Span>,
    pub dependency_chain: Vec<String>,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "{:?} in {}", self.code, self.identity)
    }
}

impl std::error::Error for Error {}

pub fn parse(bytes: &[u8], identity: impl Into<String>, limits: Limits) -> Result<Document, Error> {
    validate_limits(limits)?;
    let identity = identity.into();
    if identity.is_empty() {
        return Err(failure(ErrorCode::InvalidIdentity, &identity, None));
    }
    let syntax = parse_text(
        bytes,
        EscapeMode::LiteralBackslash,
        KeyValuesLimits {
            max_input_bytes: limits.max_document_bytes,
            max_decoded_bytes: limits.max_document_bytes,
            max_depth: limits.max_keyvalues_depth,
            max_nodes: limits.max_nodes,
            max_diagnostics: limits.max_diagnostics,
            ..KeyValuesLimits::default()
        },
    )
    .map_err(|error| keyvalues_error(&identity, error))?;
    if syntax.roots.len() != 1 {
        return Err(failure(ErrorCode::RootCount, &identity, None));
    }
    let root = syntax.roots[0].clone();
    if !matches!(root.value, Value::Object(_)) {
        return Err(failure(ErrorCode::RootKind, &identity, Some(root.key.span)));
    }
    let role = if root.key.bytes.eq_ignore_ascii_case(b"Patch") {
        Role::Patch
    } else {
        Role::Material
    };
    Ok(Document {
        identity,
        syntax,
        role,
        root,
    })
}

pub fn compose(
    root_bytes: &[u8],
    root_identity: impl Into<String>,
    responses: &[DependencyResponse],
    environment: &ConditionEnvironment,
    limits: Limits,
) -> Result<Composition, Error> {
    validate_limits(limits)?;
    let root_identity = root_identity.into();
    let mut current = evaluated_document(
        parse(root_bytes, root_identity.clone(), limits)?,
        environment,
    )?;
    let mut source_documents = vec![current.clone()];
    let mut visited = BTreeSet::from([current.identity.to_ascii_lowercase()]);
    let mut patches = Vec::new();
    let mut trace = Vec::new();
    let mut aggregate_bytes = root_bytes.len();

    while current.role == Role::Patch {
        if patches.len() >= limits.max_patch_depth || patches.len() >= limits.max_dependencies {
            return Err(Error {
                code: ErrorCode::DependencyLimit,
                identity: current.identity.clone(),
                span: None,
                dependency_chain: source_documents
                    .iter()
                    .map(|document| document.identity.clone())
                    .collect(),
            });
        }
        let patch = patch_view(&current, &mut trace)?;
        let request = DependencyRequest {
            parent_identity: current.identity.clone(),
            target_token: patch.include.token.bytes.clone(),
            include_span: patch.include.token.span,
            dependency_chain: source_documents
                .iter()
                .map(|document| document.identity.clone())
                .collect(),
        };
        let Some(response) = responses.iter().find(|response| {
            response.parent_identity == request.parent_identity
                && response.target_token == request.target_token
        }) else {
            return Ok(Composition::Needs(vec![request]));
        };
        if response.canonical_identity.is_empty() {
            return Err(Error {
                code: ErrorCode::InvalidIdentity,
                identity: current.identity,
                span: Some(patch.include.token.span),
                dependency_chain: request.dependency_chain,
            });
        }
        if !visited.insert(response.canonical_identity.to_ascii_lowercase()) {
            return Err(Error {
                code: ErrorCode::DependencyCycle,
                identity: response.canonical_identity.clone(),
                span: Some(patch.include.token.span),
                dependency_chain: request.dependency_chain,
            });
        }
        let Some(bytes) = &response.bytes else {
            return Err(Error {
                code: ErrorCode::MissingDependency,
                identity: response.canonical_identity.clone(),
                span: Some(patch.include.token.span),
                dependency_chain: request.dependency_chain,
            });
        };
        aggregate_bytes = aggregate_bytes
            .checked_add(bytes.len())
            .ok_or_else(|| Error {
                code: ErrorCode::AggregateByteLimit,
                identity: response.canonical_identity.clone(),
                span: None,
                dependency_chain: request.dependency_chain.clone(),
            })?;
        if aggregate_bytes > limits.max_aggregate_dependency_bytes {
            return Err(Error {
                code: ErrorCode::AggregateByteLimit,
                identity: response.canonical_identity.clone(),
                span: None,
                dependency_chain: request.dependency_chain,
            });
        }
        trace.push(TraceStep {
            kind: TraceKind::Dependency,
            source_identity: current.identity.clone(),
            key_path: vec![patch.include.token.bytes.clone()],
        });
        patches.push((current, patch));
        current = evaluated_document(
            parse(bytes, response.canonical_identity.clone(), limits)?,
            environment,
        )?;
        source_documents.push(current.clone());
    }

    if !current.syntax.directives.is_empty() {
        return Err(failure(
            ErrorCode::UnsupportedDirective,
            &current.identity,
            Some(current.syntax.directives[0].keyword.span),
        ));
    }
    let mut insert = Vec::new();
    let mut replace = Vec::new();
    let mut steps = 0_usize;
    for (document, patch) in patches {
        if let Some(nodes) = patch.insert {
            accumulate(
                &mut insert,
                &nodes,
                &document.identity,
                Operation::Insert,
                &mut trace,
                &mut steps,
                limits,
            )?;
        }
        if let Some(nodes) = patch.replace {
            accumulate(
                &mut replace,
                &nodes,
                &document.identity,
                Operation::Replace,
                &mut trace,
                &mut steps,
                limits,
            )?;
        }
    }
    let mut root = effective(&current.root, &current.identity, Operation::Base);
    let EffectiveValue::Object(children) = &mut root.value else {
        unreachable!("ordinary VMT root is an object")
    };
    apply(
        children,
        &insert,
        false,
        &mut trace,
        &mut Vec::new(),
        &mut steps,
        limits,
    )?;
    apply(
        children,
        &replace,
        true,
        &mut trace,
        &mut Vec::new(),
        &mut steps,
        limits,
    )?;
    if owned_bytes(&root) > limits.max_owned_bytes {
        return Err(failure(ErrorCode::OwnedByteLimit, &current.identity, None));
    }
    Ok(Composition::Complete(Box::new(EffectiveDocument {
        root,
        source_documents,
        trace,
    })))
}

struct PatchView {
    include: Scalar,
    insert: Option<Vec<Node>>,
    replace: Option<Vec<Node>>,
}

fn patch_view(document: &Document, trace: &mut Vec<TraceStep>) -> Result<PatchView, Error> {
    if !document.syntax.directives.is_empty() {
        return Err(failure(
            ErrorCode::UnsupportedDirective,
            &document.identity,
            Some(document.syntax.directives[0].keyword.span),
        ));
    }
    let Value::Object(children) = &document.root.value else {
        unreachable!("validated VMT root")
    };
    let mut include = None;
    let mut insert = None;
    let mut replace = None;
    for child in children {
        if child.key.bytes.eq_ignore_ascii_case(b"include") && include.is_none() {
            let Value::Scalar(value) = &child.value else {
                return Err(failure(
                    ErrorCode::PatchInclude,
                    &document.identity,
                    Some(child.key.span),
                ));
            };
            if value.token.bytes.is_empty() {
                return Err(failure(
                    ErrorCode::PatchInclude,
                    &document.identity,
                    Some(value.token.span),
                ));
            }
            include = Some(value.clone());
        } else if child.key.bytes.eq_ignore_ascii_case(b"insert") && insert.is_none() {
            let Value::Object(nodes) = &child.value else {
                return Err(failure(
                    ErrorCode::PatchMemberKind,
                    &document.identity,
                    Some(child.key.span),
                ));
            };
            insert = Some(nodes.clone());
        } else if child.key.bytes.eq_ignore_ascii_case(b"replace") && replace.is_none() {
            let Value::Object(nodes) = &child.value else {
                return Err(failure(
                    ErrorCode::PatchMemberKind,
                    &document.identity,
                    Some(child.key.span),
                ));
            };
            replace = Some(nodes.clone());
        } else {
            trace.push(TraceStep {
                kind: TraceKind::InertMember,
                source_identity: document.identity.clone(),
                key_path: vec![child.key.bytes.clone()],
            });
        }
    }
    Ok(PatchView {
        include: include
            .ok_or_else(|| failure(ErrorCode::PatchInclude, &document.identity, None))?,
        insert,
        replace,
    })
}

fn evaluated_document(
    mut document: Document,
    environment: &ConditionEnvironment,
) -> Result<Document, Error> {
    let evaluated = document.syntax.evaluated(environment);
    if let Some(decision) = evaluated.decisions.iter().find(|decision| !decision.mapped) {
        return Err(failure(
            ErrorCode::UnmappedCondition,
            &document.identity,
            Some(decision.span),
        ));
    }
    if evaluated.roots.len() != 1 {
        return Err(failure(ErrorCode::RootCount, &document.identity, None));
    }
    document.root = evaluated.roots[0].clone();
    Ok(document)
}

fn accumulate(
    destination: &mut Vec<EffectiveNode>,
    source: &[Node],
    identity: &str,
    operation: Operation,
    trace: &mut Vec<TraceStep>,
    steps: &mut usize,
    limits: Limits,
) -> Result<(), Error> {
    let source: Vec<_> = source
        .iter()
        .map(|node| effective(node, identity, operation))
        .collect();
    apply(
        destination,
        &source,
        false,
        trace,
        &mut Vec::new(),
        steps,
        limits,
    )
}

fn apply(
    destination: &mut Vec<EffectiveNode>,
    source: &[EffectiveNode],
    replace_only: bool,
    trace: &mut Vec<TraceStep>,
    key_path: &mut Vec<Vec<u8>>,
    steps: &mut usize,
    limits: Limits,
) -> Result<(), Error> {
    for source_node in source {
        *steps = steps.checked_add(1).ok_or_else(|| {
            failure(
                ErrorCode::CompositionLimit,
                &source_node.origin.source_identity,
                Some(source_node.origin.source_span),
            )
        })?;
        if *steps > limits.max_composition_steps {
            return Err(failure(
                ErrorCode::CompositionLimit,
                &source_node.origin.source_identity,
                Some(source_node.origin.source_span),
            ));
        }
        key_path.push(source_node.key.bytes.clone());
        let matched = destination
            .iter()
            .position(|node| node.key.bytes.eq_ignore_ascii_case(&source_node.key.bytes));
        match (matched, replace_only, &source_node.value) {
            (Some(index), false, EffectiveValue::Object(source_children)) => {
                if !matches!(destination[index].value, EffectiveValue::Object(_)) {
                    destination[index] = source_node.clone();
                    destination[index].value = EffectiveValue::Object(Vec::new());
                }
                let EffectiveValue::Object(destination_children) = &mut destination[index].value
                else {
                    unreachable!()
                };
                apply(
                    destination_children,
                    source_children,
                    false,
                    trace,
                    key_path,
                    steps,
                    limits,
                )?;
                trace.push(step(TraceKind::InsertUpdated, source_node, key_path));
            }
            (Some(index), false, EffectiveValue::Scalar(_)) => {
                destination[index] = source_node.clone();
                trace.push(step(TraceKind::InsertUpdated, source_node, key_path));
            }
            (Some(index), true, EffectiveValue::Object(source_children)) => {
                if let EffectiveValue::Object(destination_children) = &mut destination[index].value
                {
                    apply(
                        destination_children,
                        source_children,
                        true,
                        trace,
                        key_path,
                        steps,
                        limits,
                    )?;
                    trace.push(step(TraceKind::ReplaceUpdated, source_node, key_path));
                } else {
                    trace.push(step(TraceKind::ReplaceInert, source_node, key_path));
                }
            }
            (Some(index), true, EffectiveValue::Scalar(_)) => {
                destination[index] = source_node.clone();
                trace.push(step(TraceKind::ReplaceUpdated, source_node, key_path));
            }
            (None, false, _) => {
                destination.push(source_node.clone());
                trace.push(step(TraceKind::InsertAppended, source_node, key_path));
            }
            (None, true, _) => {
                trace.push(step(TraceKind::ReplaceInert, source_node, key_path));
            }
        }
        key_path.pop();
    }
    Ok(())
}

fn step(kind: TraceKind, node: &EffectiveNode, key_path: &[Vec<u8>]) -> TraceStep {
    TraceStep {
        kind,
        source_identity: node.origin.source_identity.clone(),
        key_path: key_path.to_vec(),
    }
}

fn effective(node: &Node, identity: &str, operation: Operation) -> EffectiveNode {
    EffectiveNode {
        key: node.key.clone(),
        value: match &node.value {
            Value::Scalar(value) => EffectiveValue::Scalar(value.clone()),
            Value::Object(children) => EffectiveValue::Object(
                children
                    .iter()
                    .map(|child| effective(child, identity, operation))
                    .collect(),
            ),
        },
        condition: node.condition.clone(),
        origin: Origin {
            source_identity: identity.to_owned(),
            source_span: node.key.span,
            operation,
        },
    }
}

fn owned_bytes(node: &EffectiveNode) -> usize {
    node.key.bytes.len()
        + match &node.value {
            EffectiveValue::Scalar(value) => value.token.bytes.len(),
            EffectiveValue::Object(children) => children.iter().map(owned_bytes).sum(),
        }
}

fn validate_limits(limits: Limits) -> Result<(), Error> {
    if limits.max_patch_depth > 10
        || limits.max_dependencies > 10
        || limits.max_document_bytes == 0
        || limits.max_aggregate_dependency_bytes == 0
        || limits.max_keyvalues_depth == 0
        || limits.max_nodes == 0
        || limits.max_composition_steps == 0
        || limits.max_owned_bytes == 0
        || limits.max_diagnostics == 0
    {
        return Err(failure(ErrorCode::InvalidLimits, "limits", None));
    }
    Ok(())
}

fn keyvalues_error(identity: &str, error: ParseError) -> Error {
    Error {
        code: ErrorCode::KeyValues,
        identity: identity.to_owned(),
        span: Some(error.span),
        dependency_chain: Vec::new(),
    }
}

fn failure(code: ErrorCode, identity: &str, span: Option<Span>) -> Error {
    Error {
        code,
        identity: identity.to_owned(),
        span,
        dependency_chain: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment() -> ConditionEnvironment {
        ConditionEnvironment::new([(b"$WIN32".to_vec(), true)])
    }

    fn children(node: &EffectiveNode) -> &[EffectiveNode] {
        let EffectiveValue::Object(children) = &node.value else {
            panic!("expected object")
        };
        children
    }

    fn first<'a>(nodes: &'a [EffectiveNode], key: &[u8]) -> &'a EffectiveNode {
        nodes
            .iter()
            .find(|node| node.key.bytes.eq_ignore_ascii_case(key))
            .expect("effective key")
    }

    fn scalar(node: &EffectiveNode) -> &[u8] {
        let EffectiveValue::Scalar(value) = &node.value else {
            panic!("expected scalar")
        };
        &value.token.bytes
    }

    #[test]
    fn parses_one_opaque_shader_root_and_preserves_order_repeats_and_proxies() {
        let bytes = br#"
            LightmappedGeneric
            {
                "$baseTexture" "wood/wall"
                "$flag" "1" [$WIN32]
                "$flag" "2"
                "Proxies"
                {
                    "AnimatedTexture" { "rate" "2" }
                    "AnimatedTexture" {}
                }
                "proxies" { "shadowed" {} }
            }
        "#;
        let document = parse(bytes, "materials/test.vmt", Limits::default()).unwrap();
        assert_eq!(document.role, Role::Material);
        assert_eq!(document.shader(), b"LightmappedGeneric");
        let Value::Object(nodes) = &document.root.value else {
            panic!("VMT root was not an object")
        };
        assert_eq!(nodes.len(), 5);
        assert_eq!(nodes[1].condition.as_ref().unwrap().symbol, b"$WIN32");
        let proxies = document.active_proxies().unwrap();
        assert_eq!(proxies.len(), 2);
        assert_eq!(proxies[0].key.bytes, b"AnimatedTexture");
        assert_eq!(proxies[1].key.bytes, b"AnimatedTexture");
        assert_eq!(document.syntax.unmodified_bytes(), bytes);
    }

    #[test]
    fn emits_exact_batched_patch_requests_and_composes_outer_to_base() {
        let outer = br#"
            Patch
            {
                "include" "materials\inner.vmt"
                "insert"
                {
                    "$base" "outer"
                    "nested" { "a" "outer" "c" "outer" }
                    "scalar" { "child" "value" }
                }
                "replace" { "nested" { "b" "outer" "missing" "no" } }
                "unknown" "retained"
            }
        "#;
        let inner = br#"
            patch
            {
                "include" "materials/base.vmt"
                "insert" { "$base" "inner" "nested" { "a" "inner" "d" "inner" } }
                "replace" { "nested" { "b" "inner" } }
            }
        "#;
        let base = br#"
            LightmappedGeneric
            {
                "$base" "base"
                "nested" { "a" "base" "b" "base" }
                "scalar" "base"
                "repeat" "one"
                "repeat" "two"
            }
        "#;
        let Composition::Needs(first_request) = compose(
            outer,
            "materials/outer.vmt",
            &[],
            &environment(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("outer patch did not request its dependency")
        };
        assert_eq!(first_request.len(), 1);
        assert_eq!(first_request[0].target_token, b"materials\\inner.vmt");

        let inner_response = DependencyResponse {
            parent_identity: "materials/outer.vmt".to_owned(),
            target_token: b"materials\\inner.vmt".to_vec(),
            canonical_identity: "materials/inner.vmt".to_owned(),
            bytes: Some(inner.to_vec()),
        };
        let Composition::Needs(second_request) = compose(
            outer,
            "materials/outer.vmt",
            std::slice::from_ref(&inner_response),
            &environment(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("inner patch did not request its dependency")
        };
        assert_eq!(second_request[0].parent_identity, "materials/inner.vmt");
        assert_eq!(second_request[0].target_token, b"materials/base.vmt");

        let base_response = DependencyResponse {
            parent_identity: "materials/inner.vmt".to_owned(),
            target_token: b"materials/base.vmt".to_vec(),
            canonical_identity: "materials/base.vmt".to_owned(),
            bytes: Some(base.to_vec()),
        };
        let Composition::Complete(effective) = compose(
            outer,
            "materials/outer.vmt",
            &[inner_response, base_response],
            &environment(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("complete patch graph requested another dependency")
        };
        assert_eq!(effective.source_documents.len(), 3);
        assert!(
            effective
                .trace
                .iter()
                .any(|step| step.kind == TraceKind::InertMember)
        );
        let root = children(&effective.root);
        assert_eq!(scalar(first(root, b"$base")), b"inner");
        let nested = children(first(root, b"nested"));
        assert_eq!(scalar(first(nested, b"a")), b"inner");
        assert_eq!(scalar(first(nested, b"b")), b"inner");
        assert_eq!(scalar(first(nested, b"c")), b"outer");
        assert_eq!(scalar(first(nested, b"d")), b"inner");
        assert!(nested.iter().all(|node| node.key.bytes != b"missing"));
        assert_eq!(
            scalar(first(children(first(root, b"scalar")), b"child")),
            b"value"
        );
        assert_eq!(
            root.iter()
                .filter(|node| node.key.bytes.eq_ignore_ascii_case(b"repeat"))
                .count(),
            2
        );
    }

    #[test]
    fn replace_object_does_not_convert_scalar_and_replace_scalar_converts_object() {
        let patch = br#"
            Patch
            {
                "include" "materials/base.vmt"
                "replace"
                {
                    "scalar" { "child" "ignored" }
                    "object" "replaced"
                    "missing" "ignored"
                }
            }
        "#;
        let base = br#"LightmappedGeneric { "scalar" "kept" "object" { "a" "1" } }"#;
        let response = DependencyResponse {
            parent_identity: "materials/patch.vmt".to_owned(),
            target_token: b"materials/base.vmt".to_vec(),
            canonical_identity: "materials/base.vmt".to_owned(),
            bytes: Some(base.to_vec()),
        };
        let Composition::Complete(effective) = compose(
            patch,
            "materials/patch.vmt",
            &[response],
            &environment(),
            Limits::default(),
        )
        .unwrap() else {
            panic!("unexpected dependency request")
        };
        let root = children(&effective.root);
        assert_eq!(scalar(first(root, b"scalar")), b"kept");
        assert_eq!(scalar(first(root, b"object")), b"replaced");
        assert!(root.iter().all(|node| node.key.bytes != b"missing"));
    }

    #[test]
    fn reports_missing_cycles_directives_and_limits_without_partial_output() {
        let patch = br#"Patch { "include" "materials/a.vmt" }"#;
        let missing = DependencyResponse {
            parent_identity: "materials/root.vmt".to_owned(),
            target_token: b"materials/a.vmt".to_vec(),
            canonical_identity: "materials/a.vmt".to_owned(),
            bytes: None,
        };
        assert_eq!(
            compose(
                patch,
                "materials/root.vmt",
                &[missing],
                &environment(),
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::MissingDependency
        );

        let cycle = DependencyResponse {
            parent_identity: "materials/root.vmt".to_owned(),
            target_token: b"materials/a.vmt".to_vec(),
            canonical_identity: "materials/root.vmt".to_owned(),
            bytes: Some(patch.to_vec()),
        };
        assert_eq!(
            compose(
                patch,
                "materials/root.vmt",
                &[cycle],
                &environment(),
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::DependencyCycle
        );

        let directive = br#"#include "materials/base.vmt" LightmappedGeneric {}"#;
        assert_eq!(
            compose(
                directive,
                "materials/directive.vmt",
                &[],
                &environment(),
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::UnsupportedDirective
        );
        assert_eq!(
            parse(
                b"LightmappedGeneric {}",
                "materials/a.vmt",
                Limits {
                    max_patch_depth: 11,
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::InvalidLimits
        );
        assert_eq!(
            compose(
                b"LightmappedGeneric { \"$a\" \"1\" [$UNKNOWN] }",
                "materials/condition.vmt",
                &[],
                &environment(),
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::UnmappedCondition
        );
    }
}
