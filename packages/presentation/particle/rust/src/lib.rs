mod definition;
mod control_point_light;
mod dmx;
mod sheet;
mod rope;
mod visibility;
mod source_noise;
mod source_random;
mod world;

pub use definition::{
    ChildDeclaration, Definition, DefinitionLookup, Function, FunctionCategory, PcfSource,
    Registry, RegistryLimits, TargetClosure,
};
pub use dmx::{Attribute, Document, Element, Value};
pub use sheet::{
    ParticleSheet, SheetFrame, SheetSample, SheetSampleRequest, SheetSequence, sample_sheet,
};
pub use rope::{RopePoint, RopeRender};
pub use visibility::{VisibilityProxy, VisibilitySample, VisibilityView};
pub use world::{
    AdvanceRequest, Bounds, CollisionBatch, CollisionQuery, CollisionResult, ControlPoint, Event,
    EventCommand, ParticleBlendFactor, ParticleBlendState, ParticleColorSpace, ParticleMaterial,
    ParticleMaterialShader, ParticleMaterialState, ParticleWorld, Primitive, RenderItem, StopMode,
    TraceRequest, WorldLimits, encode_render_output, resolve_render_output,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InputLimit,
    MalformedHeader,
    Truncated,
    InvalidUtf8,
    InvalidString,
    InvalidType,
    InvalidReference,
    DuplicateAttribute,
    NonFinite,
    InvalidValue,
    TrailingData,
    MissingDefinition,
    MissingDependency,
    UnsupportedFunction,
    DefinitionCycle,
    BoundExceeded,
    TimeReversed,
    InvalidEvent,
    InvalidState,
    MissingQuery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode,
    pub source: String,
    pub offset: usize,
    pub detail: String,
}

impl Error {
    pub(crate) fn new(
        code: ErrorCode,
        source: impl Into<String>,
        offset: usize,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code,
            source: source.into(),
            offset,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            output,
            "{:?} in {} at {}: {}",
            self.code, self.source, self.offset, self.detail
        )
    }
}

impl std::error::Error for Error {}
