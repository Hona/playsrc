use crate::Proxy;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariableRef {
    pub name: Vec<u8>,
    pub component: Option<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FloatInput {
    Constant(f32),
    Variable(VariableRef),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProxyOperation {
    AnimatedTexture {
        texture: VariableRef,
        frame: VariableRef,
        frame_rate: f32,
        wrap: bool,
    },
    Sine {
        result: VariableRef,
        period: FloatInput,
        maximum: FloatInput,
        minimum: FloatInput,
        time_offset: FloatInput,
    },
    Equals {
        source: VariableRef,
        result: VariableRef,
    },
    Multiply {
        first: VariableRef,
        second: VariableRef,
        result: VariableRef,
    },
    LessOrEqual {
        first: VariableRef,
        second: VariableRef,
        less_equal: VariableRef,
        greater: VariableRef,
        result: VariableRef,
    },
    SelectFirstIfNonZero {
        first: VariableRef,
        second: VariableRef,
        result: VariableRef,
    },
    TextureTransform {
        result: VariableRef,
        center: Option<VariableRef>,
        scale: Option<VariableRef>,
        rotate: Option<VariableRef>,
        translate: Option<VariableRef>,
    },
    TextureScroll {
        result: VariableRef,
        rate: FloatInput,
        angle: FloatInput,
        scale: FloatInput,
    },
    WaterLod,
    Invisibility {
        player_tint: bool,
    },
    ModelGlowColor {
        result: VariableRef,
    },
    YellowLevel {
        result: VariableRef,
    },
    AnimatedWeaponSheen,
    WeaponSkin,
    ScalarModelInput {
        input: ScalarModelInput,
        result: VariableRef,
    },
    VectorModelInput {
        input: VectorModelInput,
        result: VariableRef,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarModelInput {
    Invulnerability,
    Burn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorModelInput {
    ItemTint,
    StickybombGlow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyDisposition {
    Handled,
    Malformed,
    Unsupported,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProxyProgramEntry {
    pub name: Vec<u8>,
    pub disposition: ProxyDisposition,
    pub operation: Option<ProxyOperation>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProxyProgram {
    pub entries: Vec<ProxyProgramEntry>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProxyValue {
    Int(i32),
    Float(f32),
    Vector { values: [f32; 4], size: u8 },
    Matrix([f32; 16]),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProxyEvaluationContext {
    pub time: f32,
    pub frame_time: f32,
    pub water_lod: Option<[f32; 2]>,
    pub texture_frames: BTreeMap<Vec<u8>, u32>,
    pub model_inputs: ModelProxyInputs,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InvisibilityInput {
    pub factor: f32,
    pub player_tint: Option<[f32; 3]>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponSheenInput {
    pub frame: i32,
    pub tint: [f32; 4],
    pub mask_scale: [f32; 2],
    pub mask_offset: [f32; 2],
    pub mask_direction: i32,
    pub shader_index: i32,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelProxyInputs {
    pub invisibility: Option<InvisibilityInput>,
    pub model_glow_color: Option<[f32; 3]>,
    pub yellow_level: Option<[f32; 3]>,
    pub weapon_sheen: Option<WeaponSheenInput>,
    pub weapon_skin_base_texture: Option<Option<String>>,
    pub invulnerability_level: Option<f32>,
    pub burn_level: Option<f32>,
    pub item_tint: Option<[f32; 3]>,
    pub stickybomb_glow: Option<[f32; 3]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyEvaluationErrorCode {
    NonFinite,
    MissingVariable,
    InvalidVariable,
    InvalidTextureFrames,
    MissingWaterLod,
    MissingModelInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyEvaluationError {
    pub code: ProxyEvaluationErrorCode,
    pub operation: usize,
    pub variable: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyTraceStep {
    pub operation: usize,
    pub disposition: ProxyDisposition,
    pub writes: Vec<VariableRef>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedProxyState {
    pub variables: BTreeMap<Vec<u8>, ProxyValue>,
    pub trace: Vec<ProxyTraceStep>,
    pub effects: Vec<ModelProxyEffect>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelProxyEffect {
    WeaponSkinBaseTexture(Option<String>),
}

pub(crate) fn compile_proxy_program(proxies: &[Proxy]) -> ProxyProgram {
    let mut entries = Vec::with_capacity(proxies.len());
    for proxy in proxies {
        let operation = compile_operation(proxy);
        let (disposition, operation) = match operation {
            Ok(Some(operation)) => (ProxyDisposition::Handled, Some(operation)),
            Ok(None) => (ProxyDisposition::Unsupported, None),
            Err(()) => (ProxyDisposition::Malformed, None),
        };
        entries.push(ProxyProgramEntry {
            name: proxy.name.clone(),
            disposition,
            operation,
        });
    }
    ProxyProgram { entries }
}

fn compile_operation(proxy: &Proxy) -> Result<Option<ProxyOperation>, ()> {
    if proxy.name.eq_ignore_ascii_case(b"AnimatedTexture") {
        Ok(Some(ProxyOperation::AnimatedTexture {
            texture: required_variable(proxy, b"animatedtexturevar")?,
            frame: required_variable(proxy, b"animatedtextureframenumvar")?,
            frame_rate: float_argument(proxy, b"animatedtextureframerate", 15.0)?,
            wrap: integer_argument(proxy, b"animationnowrap", 0) == 0,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"Sine") {
        Ok(Some(ProxyOperation::Sine {
            result: required_variable(proxy, b"resultvar")?,
            period: float_input(proxy, b"sineperiod", 1.0)?,
            maximum: float_input(proxy, b"sinemax", 1.0)?,
            minimum: float_input(proxy, b"sinemin", 0.0)?,
            time_offset: float_input(proxy, b"timeoffset", 0.0)?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"Equals") {
        Ok(Some(ProxyOperation::Equals {
            source: required_variable(proxy, b"srcvar1")?,
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"Multiply") {
        Ok(Some(ProxyOperation::Multiply {
            first: required_variable(proxy, b"srcvar1")?,
            second: required_variable(proxy, b"srcvar2")?,
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"LessOrEqual") {
        Ok(Some(ProxyOperation::LessOrEqual {
            first: required_variable(proxy, b"srcvar1")?,
            second: required_variable(proxy, b"srcvar2")?,
            less_equal: required_variable(proxy, b"lessequalvar")?,
            greater: required_variable(proxy, b"greatervar")?,
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"SelectFirstIfNonZero") {
        Ok(Some(ProxyOperation::SelectFirstIfNonZero {
            first: required_variable(proxy, b"srcvar1")?,
            second: required_variable(proxy, b"srcvar2")?,
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"TextureTransform") {
        Ok(Some(ProxyOperation::TextureTransform {
            result: required_variable(proxy, b"resultvar")?,
            center: optional_variable(proxy, b"centervar")?,
            scale: optional_variable(proxy, b"scalevar")?,
            rotate: optional_variable(proxy, b"rotatevar")?,
            translate: optional_variable(proxy, b"translatevar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"TextureScroll") {
        Ok(Some(ProxyOperation::TextureScroll {
            result: required_variable(proxy, b"texturescrollvar")?,
            rate: float_input(proxy, b"texturescrollrate", 1.0)?,
            angle: float_input(proxy, b"texturescrollangle", 0.0)?,
            scale: float_input(proxy, b"texturescale", 1.0)?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"WaterLOD") {
        Ok(Some(ProxyOperation::WaterLod))
    } else if proxy.name.eq_ignore_ascii_case(b"invis")
        || proxy.name.eq_ignore_ascii_case(b"weapon_invis")
    {
        Ok(Some(ProxyOperation::Invisibility { player_tint: false }))
    } else if proxy.name.eq_ignore_ascii_case(b"spy_invis") {
        Ok(Some(ProxyOperation::Invisibility { player_tint: true }))
    } else if proxy.name.eq_ignore_ascii_case(b"ModelGlowColor") {
        Ok(Some(ProxyOperation::ModelGlowColor {
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"YellowLevel") {
        Ok(Some(ProxyOperation::YellowLevel {
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"AnimatedWeaponSheen") {
        required_variable(proxy, b"animatedtexturevar")?;
        required_variable(proxy, b"animatedtextureframenumvar")?;
        Ok(Some(ProxyOperation::AnimatedWeaponSheen))
    } else if proxy.name.eq_ignore_ascii_case(b"WeaponSkin") {
        Ok(Some(ProxyOperation::WeaponSkin))
    } else if proxy.name.eq_ignore_ascii_case(b"InvulnLevel") {
        Ok(Some(ProxyOperation::ScalarModelInput {
            input: ScalarModelInput::Invulnerability,
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"BurnLevel") {
        Ok(Some(ProxyOperation::ScalarModelInput {
            input: ScalarModelInput::Burn,
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"ItemTintColor") {
        Ok(Some(ProxyOperation::VectorModelInput {
            input: VectorModelInput::ItemTint,
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else if proxy.name.eq_ignore_ascii_case(b"StickybombGlowColor") {
        Ok(Some(ProxyOperation::VectorModelInput {
            input: VectorModelInput::StickybombGlow,
            result: required_variable(proxy, b"resultvar")?,
        }))
    } else {
        Ok(None)
    }
}

pub fn evaluate_proxy_program(
    program: &ProxyProgram,
    initial_variables: &BTreeMap<Vec<u8>, ProxyValue>,
    context: &ProxyEvaluationContext,
) -> Result<EvaluatedProxyState, ProxyEvaluationError> {
    if !context.time.is_finite()
        || !context.frame_time.is_finite()
        || context.frame_time < 0.0
        || context
            .water_lod
            .is_some_and(|values| values.iter().any(|value| !value.is_finite()))
        || !model_inputs_are_finite(&context.model_inputs)
    {
        return Err(evaluation_error(
            ProxyEvaluationErrorCode::NonFinite,
            0,
            None,
        ));
    }
    if let Some((name, _)) = initial_variables
        .iter()
        .find(|(_, value)| !value_is_finite(value))
    {
        return Err(evaluation_error(
            ProxyEvaluationErrorCode::NonFinite,
            0,
            Some(name.clone()),
        ));
    }
    let mut variables = initial_variables.clone();
    let mut trace = Vec::with_capacity(program.entries.len());
    let mut effects = Vec::new();
    for (index, entry) in program.entries.iter().enumerate() {
        let Some(operation) = &entry.operation else {
            trace.push(ProxyTraceStep {
                operation: index,
                disposition: entry.disposition,
                writes: Vec::new(),
            });
            continue;
        };
        let writes = match operation {
            ProxyOperation::AnimatedTexture {
                texture,
                frame,
                frame_rate,
                wrap,
            } => {
                let frame_count = context
                    .texture_frames
                    .get(&texture.name)
                    .copied()
                    .ok_or_else(|| {
                        evaluation_error(
                            ProxyEvaluationErrorCode::MissingVariable,
                            index,
                            Some(texture.name.clone()),
                        )
                    })?;
                if frame_count == 0 {
                    return Err(evaluation_error(
                        ProxyEvaluationErrorCode::InvalidTextureFrames,
                        index,
                        Some(texture.name.clone()),
                    ));
                }
                let elapsed = context.time.max(0.0);
                let previous = (context.time - context.frame_time).max(0.0);
                let exact = *frame_rate * elapsed;
                let previous_exact = *frame_rate * previous;
                if !exact.is_finite() || !previous_exact.is_finite() {
                    return Err(evaluation_error(
                        ProxyEvaluationErrorCode::NonFinite,
                        index,
                        None,
                    ));
                }
                let mut selected = (exact.trunc() as i64) % i64::from(frame_count);
                let old = (previous_exact.trunc() as i64) % i64::from(frame_count);
                if !wrap && old > selected {
                    selected = i64::from(frame_count - 1);
                }
                set_value(
                    &mut variables,
                    frame,
                    ProxyValue::Int(selected as i32),
                    index,
                )?;
                vec![frame.clone()]
            }
            ProxyOperation::Sine {
                result,
                period,
                maximum,
                minimum,
                time_offset,
            } => {
                let mut period = read_float(period, &variables, index)?;
                if period == 0.0 {
                    period = 1.0;
                }
                let maximum = read_float(maximum, &variables, index)?;
                let minimum = read_float(minimum, &variables, index)?;
                let time_offset = read_float(time_offset, &variables, index)?;
                let unit = ((std::f32::consts::TAU * (context.time - time_offset) / period).sin()
                    * 0.5)
                    + 0.5;
                let value = (maximum - minimum) * unit + minimum;
                if !value.is_finite() {
                    return Err(evaluation_error(
                        ProxyEvaluationErrorCode::NonFinite,
                        index,
                        None,
                    ));
                }
                set_float(&mut variables, result, value, index)?;
                vec![result.clone()]
            }
            ProxyOperation::Equals { source, result } => {
                let value = read_value(source, &variables, index)?;
                match value {
                    ProxyValue::Int(value) => {
                        set_value(&mut variables, result, ProxyValue::Int(value), index)?
                    }
                    ProxyValue::Float(value) => set_float(&mut variables, result, value, index)?,
                    ProxyValue::Vector { values, size } => {
                        set_value(
                            &mut variables,
                            result,
                            ProxyValue::Vector { values, size },
                            index,
                        )?;
                    }
                    ProxyValue::Matrix(_) => {
                        return Err(evaluation_error(
                            ProxyEvaluationErrorCode::InvalidVariable,
                            index,
                            Some(source.name.clone()),
                        ));
                    }
                }
                vec![result.clone()]
            }
            ProxyOperation::Multiply {
                first,
                second,
                result,
            } => {
                let first_value = read_value(first, &variables, index)?;
                let second_value = read_value(second, &variables, index)?;
                let result_value = multiply_values(
                    first_value,
                    second_value,
                    variables.get(&result.name),
                    index,
                    &result.name,
                )?;
                set_value(&mut variables, result, result_value, index)?;
                vec![result.clone()]
            }
            ProxyOperation::LessOrEqual {
                first,
                second,
                less_equal,
                greater,
                result,
            } => {
                let first_value = read_value(first, &variables, index)?;
                let second_value = read_value(second, &variables, index)?;
                let selected = if scalar_component(first_value).ok_or_else(|| {
                    evaluation_error(
                        ProxyEvaluationErrorCode::InvalidVariable,
                        index,
                        Some(first.name.clone()),
                    )
                })? <= scalar_component(second_value).ok_or_else(|| {
                    evaluation_error(
                        ProxyEvaluationErrorCode::InvalidVariable,
                        index,
                        Some(second.name.clone()),
                    )
                })? {
                    read_value(less_equal, &variables, index)?
                } else {
                    read_value(greater, &variables, index)?
                };
                let selected =
                    coerce_result(selected, variables.get(&result.name), index, &result.name)?;
                set_value(&mut variables, result, selected, index)?;
                vec![result.clone()]
            }
            ProxyOperation::SelectFirstIfNonZero {
                first,
                second,
                result,
            } => {
                let first_value = read_value(first, &variables, index)?;
                let nonzero = match &first_value {
                    ProxyValue::Int(value) => *value != 0,
                    ProxyValue::Float(value) => *value != 0.0,
                    ProxyValue::Vector { values, size } => values[..usize::from(*size)]
                        .iter()
                        .any(|value| *value != 0.0),
                    ProxyValue::Matrix(_) => {
                        return Err(evaluation_error(
                            ProxyEvaluationErrorCode::InvalidVariable,
                            index,
                            Some(first.name.clone()),
                        ));
                    }
                };
                let selected = if nonzero {
                    first_value
                } else {
                    read_value(second, &variables, index)?
                };
                let selected =
                    coerce_result(selected, variables.get(&result.name), index, &result.name)?;
                set_value(&mut variables, result, selected, index)?;
                vec![result.clone()]
            }
            ProxyOperation::TextureTransform {
                result,
                center,
                scale,
                rotate,
                translate,
            } => {
                let center = read_optional_vec2(center, &variables, index, [0.5, 0.5])?;
                let scale = read_optional_vec2(scale, &variables, index, [1.0, 1.0])?;
                let angle = read_optional_float(rotate, &variables, index, 0.0)?.to_radians();
                let translation = read_optional_vec2(translate, &variables, index, [0.0, 0.0])?;
                let matrix = multiply(
                    translation_matrix(translation[0], translation[1]),
                    multiply(
                        translation_matrix(center[0], center[1]),
                        multiply(
                            rotation_matrix(angle),
                            multiply(
                                scale_matrix(scale[0], scale[1]),
                                translation_matrix(-center[0], -center[1]),
                            ),
                        ),
                    ),
                );
                set_value(&mut variables, result, ProxyValue::Matrix(matrix), index)?;
                vec![result.clone()]
            }
            ProxyOperation::TextureScroll {
                result,
                rate,
                angle,
                scale,
            } => {
                let rate = read_float(rate, &variables, index)?;
                let angle = read_float(angle, &variables, index)?.to_radians();
                let scale = read_float(scale, &variables, index)?;
                let s = wrap_offset(context.time * angle.cos() * rate);
                let t = wrap_offset(context.time * angle.sin() * rate);
                if [s, t, scale].iter().any(|value| !value.is_finite()) {
                    return Err(evaluation_error(
                        ProxyEvaluationErrorCode::NonFinite,
                        index,
                        None,
                    ));
                }
                set_value(
                    &mut variables,
                    result,
                    ProxyValue::Matrix([
                        scale, 0.0, 0.0, s, 0.0, scale, 0.0, t, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ]),
                    index,
                )?;
                vec![result.clone()]
            }
            ProxyOperation::WaterLod => {
                let [start, end] = context.water_lod.ok_or_else(|| {
                    evaluation_error(ProxyEvaluationErrorCode::MissingWaterLod, index, None)
                })?;
                let start_ref = VariableRef {
                    name: b"$cheapwaterstartdistance".to_vec(),
                    component: None,
                };
                let end_ref = VariableRef {
                    name: b"$cheapwaterenddistance".to_vec(),
                    component: None,
                };
                set_float(&mut variables, &start_ref, start, index)?;
                set_float(&mut variables, &end_ref, end, index)?;
                vec![start_ref, end_ref]
            }
            ProxyOperation::Invisibility { player_tint } => {
                let input = context.model_inputs.invisibility.ok_or_else(|| {
                    evaluation_error(ProxyEvaluationErrorCode::MissingModelInput, index, None)
                })?;
                let cloak = variable(b"$cloakfactor").expect("fixed cloak variable");
                set_float(&mut variables, &cloak, input.factor, index)?;
                let mut writes = vec![cloak];
                if *player_tint {
                    let tint = input.player_tint.ok_or_else(|| {
                        evaluation_error(ProxyEvaluationErrorCode::MissingModelInput, index, None)
                    })?;
                    let target = variable(b"$cloakcolortint").expect("fixed cloak tint variable");
                    set_value(&mut variables, &target, vector_value(tint), index)?;
                    writes.push(target);
                }
                writes
            }
            ProxyOperation::ModelGlowColor { result } => {
                let value = context.model_inputs.model_glow_color.ok_or_else(|| {
                    evaluation_error(ProxyEvaluationErrorCode::MissingModelInput, index, None)
                })?;
                set_value(&mut variables, result, vector_value(value), index)?;
                vec![result.clone()]
            }
            ProxyOperation::YellowLevel { result } => {
                let value = context.model_inputs.yellow_level.ok_or_else(|| {
                    evaluation_error(ProxyEvaluationErrorCode::MissingModelInput, index, None)
                })?;
                set_value(&mut variables, result, vector_value(value), index)?;
                vec![result.clone()]
            }
            ProxyOperation::AnimatedWeaponSheen => {
                let input = context.model_inputs.weapon_sheen.ok_or_else(|| {
                    evaluation_error(ProxyEvaluationErrorCode::MissingModelInput, index, None)
                })?;
                let frame = variable(b"$sheenmapmaskframe").expect("fixed sheen frame variable");
                let tint = variable(b"$sheenmaptint").expect("fixed sheen tint variable");
                let shader = variable(b"$sheenindex").expect("fixed sheen index variable");
                if !input.enabled {
                    set_value(&mut variables, &frame, ProxyValue::Int(0), index)?;
                    set_value(
                        &mut variables,
                        &tint,
                        ProxyValue::Vector {
                            values: [0.0; 4],
                            size: 3,
                        },
                        index,
                    )?;
                    set_value(&mut variables, &shader, ProxyValue::Int(0), index)?;
                    vec![tint, frame, shader]
                } else {
                    let scale_x =
                        variable(b"$sheenmapmaskscalex").expect("fixed sheen scale variable");
                    let scale_y =
                        variable(b"$sheenmapmaskscaley").expect("fixed sheen scale variable");
                    let offset_x =
                        variable(b"$sheenmapmaskoffsetx").expect("fixed sheen offset variable");
                    let offset_y =
                        variable(b"$sheenmapmaskoffsety").expect("fixed sheen offset variable");
                    let direction = variable(b"$sheenmapmaskdirection")
                        .expect("fixed sheen direction variable");
                    set_value(&mut variables, &frame, ProxyValue::Int(input.frame), index)?;
                    set_value(
                        &mut variables,
                        &tint,
                        ProxyValue::Vector {
                            values: input.tint,
                            size: 4,
                        },
                        index,
                    )?;
                    set_float(&mut variables, &scale_x, input.mask_scale[0], index)?;
                    set_float(&mut variables, &scale_y, input.mask_scale[1], index)?;
                    set_float(&mut variables, &offset_x, input.mask_offset[0], index)?;
                    set_float(&mut variables, &offset_y, input.mask_offset[1], index)?;
                    set_value(
                        &mut variables,
                        &direction,
                        ProxyValue::Int(input.mask_direction),
                        index,
                    )?;
                    set_value(
                        &mut variables,
                        &shader,
                        ProxyValue::Int(input.shader_index),
                        index,
                    )?;
                    vec![
                        tint, frame, scale_x, scale_y, offset_x, offset_y, direction, shader,
                    ]
                }
            }
            ProxyOperation::WeaponSkin => {
                let texture = context
                    .model_inputs
                    .weapon_skin_base_texture
                    .clone()
                    .ok_or_else(|| {
                        evaluation_error(ProxyEvaluationErrorCode::MissingModelInput, index, None)
                    })?;
                effects.push(ModelProxyEffect::WeaponSkinBaseTexture(texture));
                Vec::new()
            }
            ProxyOperation::ScalarModelInput { input, result } => {
                let value = match input {
                    ScalarModelInput::Invulnerability => context.model_inputs.invulnerability_level,
                    ScalarModelInput::Burn => context.model_inputs.burn_level,
                }
                .ok_or_else(|| {
                    evaluation_error(ProxyEvaluationErrorCode::MissingModelInput, index, None)
                })?;
                set_float(&mut variables, result, value, index)?;
                vec![result.clone()]
            }
            ProxyOperation::VectorModelInput { input, result } => {
                let value = match input {
                    VectorModelInput::ItemTint => context.model_inputs.item_tint,
                    VectorModelInput::StickybombGlow => context.model_inputs.stickybomb_glow,
                }
                .ok_or_else(|| {
                    evaluation_error(ProxyEvaluationErrorCode::MissingModelInput, index, None)
                })?;
                set_value(&mut variables, result, vector_value(value), index)?;
                vec![result.clone()]
            }
        };
        trace.push(ProxyTraceStep {
            operation: index,
            disposition: ProxyDisposition::Handled,
            writes,
        });
    }
    Ok(EvaluatedProxyState {
        variables,
        trace,
        effects,
    })
}

fn vector_value(values: [f32; 3]) -> ProxyValue {
    ProxyValue::Vector {
        values: [values[0], values[1], values[2], 0.0],
        size: 3,
    }
}

fn multiply_values(
    first: ProxyValue,
    second: ProxyValue,
    current_result: Option<&ProxyValue>,
    operation: usize,
    result: &[u8],
) -> Result<ProxyValue, ProxyEvaluationError> {
    let selected = current_result.unwrap_or(&first);
    match selected {
        ProxyValue::Vector { size, .. } => {
            let size = *size;
            let first = vector_components(first, size).ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            })?;
            let second = vector_components(second, size).ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            })?;
            let mut values = [0.0; 4];
            for component in 0..usize::from(size) {
                values[component] = first[component] * second[component];
            }
            Ok(ProxyValue::Vector { values, size })
        }
        ProxyValue::Float(_) => Ok(ProxyValue::Float(
            scalar_component(first).ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            })? * scalar_component(second).ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            })?,
        )),
        ProxyValue::Int(_) => Ok(ProxyValue::Float(
            scalar_component(first).ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            })? * scalar_component(second).ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            })?,
        )),
        ProxyValue::Matrix(_) => Err(evaluation_error(
            ProxyEvaluationErrorCode::InvalidVariable,
            operation,
            Some(result.to_vec()),
        )),
    }
}

fn coerce_result(
    selected: ProxyValue,
    current_result: Option<&ProxyValue>,
    operation: usize,
    result: &[u8],
) -> Result<ProxyValue, ProxyEvaluationError> {
    match current_result {
        Some(ProxyValue::Vector { size, .. }) => vector_components(selected, *size)
            .map(|values| ProxyValue::Vector {
                values,
                size: *size,
            })
            .ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            }),
        Some(ProxyValue::Float(_)) => scalar_component(selected)
            .map(ProxyValue::Float)
            .ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            }),
        Some(ProxyValue::Int(_)) => scalar_component(selected)
            .map(ProxyValue::Float)
            .ok_or_else(|| {
                evaluation_error(
                    ProxyEvaluationErrorCode::InvalidVariable,
                    operation,
                    Some(result.to_vec()),
                )
            }),
        Some(ProxyValue::Matrix(_)) => Err(evaluation_error(
            ProxyEvaluationErrorCode::InvalidVariable,
            operation,
            Some(result.to_vec()),
        )),
        None => Ok(selected),
    }
}

fn vector_components(value: ProxyValue, size: u8) -> Option<[f32; 4]> {
    match value {
        ProxyValue::Vector {
            values,
            size: source_size,
        } if source_size >= size => Some(values),
        ProxyValue::Int(value) => Some([value as f32; 4]),
        ProxyValue::Float(value) => Some([value; 4]),
        _ => None,
    }
}

fn scalar_component(value: ProxyValue) -> Option<f32> {
    match value {
        ProxyValue::Int(value) => Some(value as f32),
        ProxyValue::Float(value) => Some(value),
        ProxyValue::Vector { values, .. } => Some(values[0]),
        ProxyValue::Matrix(_) => None,
    }
}

fn argument<'a>(proxy: &'a Proxy, name: &[u8]) -> Option<&'a [u8]> {
    proxy
        .scalar_arguments
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_slice())
}

fn required_variable(proxy: &Proxy, name: &[u8]) -> Result<VariableRef, ()> {
    argument(proxy, name)
        .filter(|value| !value.is_empty())
        .ok_or(())
        .and_then(|value| variable(value).ok_or(()))
}

fn optional_variable(proxy: &Proxy, name: &[u8]) -> Result<Option<VariableRef>, ()> {
    argument(proxy, name)
        .filter(|value| !value.is_empty())
        .map(|value| variable(value).ok_or(()))
        .transpose()
}

fn variable(value: &[u8]) -> Option<VariableRef> {
    let mut name = value;
    let mut component = None;
    if value.last() == Some(&b']') {
        let open = value.iter().rposition(|byte| *byte == b'[')?;
        let index = std::str::from_utf8(&value[open + 1..value.len() - 1])
            .ok()?
            .parse::<u8>()
            .ok()?;
        if index >= 4 {
            return None;
        }
        name = &value[..open];
        component = Some(index);
    }
    (!name.is_empty()).then(|| VariableRef {
        name: lower(name),
        component,
    })
}

fn float_argument(proxy: &Proxy, name: &[u8], default: f32) -> Result<f32, ()> {
    let value = argument(proxy, name)
        .map(|value| std::str::from_utf8(value).ok()?.trim().parse::<f32>().ok())
        .unwrap_or(Some(default))
        .filter(|value| value.is_finite())
        .ok_or(())?;
    Ok(value)
}

fn integer_argument(proxy: &Proxy, name: &[u8], default: i32) -> i32 {
    let Some(value) = argument(proxy, name) else {
        return default;
    };
    let Ok(value) = std::str::from_utf8(value) else {
        return 0;
    };
    let value = value.trim_start();
    let digits = value
        .strip_prefix(['-', '+'])
        .map_or(value, |digits| digits);
    let length = digits.bytes().take_while(u8::is_ascii_digit).count();
    if length == 0 {
        0
    } else {
        value[..value.len() - digits.len() + length]
            .parse()
            .unwrap_or(0)
    }
}

fn float_input(proxy: &Proxy, name: &[u8], default: f32) -> Result<FloatInput, ()> {
    let Some(value) = argument(proxy, name) else {
        return Ok(FloatInput::Constant(default));
    };
    if let Ok(text) = std::str::from_utf8(value)
        && let Ok(value) = text.trim().parse::<f32>()
        && value.is_finite()
    {
        return Ok(FloatInput::Constant(value));
    }
    variable(value).map(FloatInput::Variable).ok_or(())
}

fn read_float(
    input: &FloatInput,
    variables: &BTreeMap<Vec<u8>, ProxyValue>,
    operation: usize,
) -> Result<f32, ProxyEvaluationError> {
    match input {
        FloatInput::Constant(value) => Ok(*value),
        FloatInput::Variable(reference) => match read_value(reference, variables, operation)? {
            ProxyValue::Int(value) => Ok(value as f32),
            ProxyValue::Float(value) => Ok(value),
            ProxyValue::Vector { values, .. } => Ok(values[0]),
            ProxyValue::Matrix(_) => Err(evaluation_error(
                ProxyEvaluationErrorCode::InvalidVariable,
                operation,
                Some(reference.name.clone()),
            )),
        },
    }
}

fn read_value(
    reference: &VariableRef,
    variables: &BTreeMap<Vec<u8>, ProxyValue>,
    operation: usize,
) -> Result<ProxyValue, ProxyEvaluationError> {
    let value = variables.get(&reference.name).cloned().ok_or_else(|| {
        evaluation_error(
            ProxyEvaluationErrorCode::MissingVariable,
            operation,
            Some(reference.name.clone()),
        )
    })?;
    let Some(component) = reference.component else {
        return Ok(value);
    };
    let ProxyValue::Vector { values, size } = value else {
        return Err(evaluation_error(
            ProxyEvaluationErrorCode::InvalidVariable,
            operation,
            Some(reference.name.clone()),
        ));
    };
    if component >= size {
        return Err(evaluation_error(
            ProxyEvaluationErrorCode::InvalidVariable,
            operation,
            Some(reference.name.clone()),
        ));
    }
    Ok(ProxyValue::Float(values[component as usize]))
}

fn set_float(
    variables: &mut BTreeMap<Vec<u8>, ProxyValue>,
    reference: &VariableRef,
    value: f32,
    operation: usize,
) -> Result<(), ProxyEvaluationError> {
    if let Some(component) = reference.component {
        let Some(ProxyValue::Vector { values, size }) = variables.get_mut(&reference.name) else {
            return Err(evaluation_error(
                ProxyEvaluationErrorCode::MissingVariable,
                operation,
                Some(reference.name.clone()),
            ));
        };
        if component >= *size {
            return Err(evaluation_error(
                ProxyEvaluationErrorCode::InvalidVariable,
                operation,
                Some(reference.name.clone()),
            ));
        }
        values[component as usize] = value;
        return Ok(());
    }
    if let Some(ProxyValue::Vector { values, size }) = variables.get_mut(&reference.name) {
        values[..usize::from(*size)].fill(value);
    } else {
        variables.insert(reference.name.clone(), ProxyValue::Float(value));
    }
    Ok(())
}

fn set_value(
    variables: &mut BTreeMap<Vec<u8>, ProxyValue>,
    reference: &VariableRef,
    value: ProxyValue,
    operation: usize,
) -> Result<(), ProxyEvaluationError> {
    if reference.component.is_some() {
        return match value {
            ProxyValue::Int(value) => set_float(variables, reference, value as f32, operation),
            ProxyValue::Float(value) => set_float(variables, reference, value, operation),
            _ => Err(evaluation_error(
                ProxyEvaluationErrorCode::InvalidVariable,
                operation,
                Some(reference.name.clone()),
            )),
        };
    }
    variables.insert(reference.name.clone(), value);
    Ok(())
}

fn read_optional_float(
    reference: &Option<VariableRef>,
    variables: &BTreeMap<Vec<u8>, ProxyValue>,
    operation: usize,
    default: f32,
) -> Result<f32, ProxyEvaluationError> {
    reference.as_ref().map_or(Ok(default), |reference| {
        read_float(
            &FloatInput::Variable(reference.clone()),
            variables,
            operation,
        )
    })
}

fn read_optional_vec2(
    reference: &Option<VariableRef>,
    variables: &BTreeMap<Vec<u8>, ProxyValue>,
    operation: usize,
    default: [f32; 2],
) -> Result<[f32; 2], ProxyEvaluationError> {
    let Some(reference) = reference else {
        return Ok(default);
    };
    match read_value(reference, variables, operation)? {
        ProxyValue::Vector { values, size } if size >= 2 => Ok([values[0], values[1]]),
        _ => Err(evaluation_error(
            ProxyEvaluationErrorCode::InvalidVariable,
            operation,
            Some(reference.name.clone()),
        )),
    }
}

fn translation_matrix(x: f32, y: f32) -> [f32; 16] {
    [
        1.0, 0.0, 0.0, x, 0.0, 1.0, 0.0, y, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn scale_matrix(x: f32, y: f32) -> [f32; 16] {
    [
        x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn rotation_matrix(angle: f32) -> [f32; 16] {
    let (sine, cosine) = angle.sin_cos();
    [
        cosine, -sine, 0.0, 0.0, sine, cosine, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn multiply(left: [f32; 16], right: [f32; 16]) -> [f32; 16] {
    std::array::from_fn(|index| {
        let row = index / 4;
        let column = index % 4;
        (0..4)
            .map(|inner| left[row * 4 + inner] * right[inner * 4 + column])
            .sum()
    })
}

fn wrap_offset(mut value: f32) -> f32 {
    if value < 0.0 {
        value += 1.0 + -value.trunc();
    }
    value - value.trunc()
}

fn value_is_finite(value: &ProxyValue) -> bool {
    match value {
        ProxyValue::Int(_) => true,
        ProxyValue::Float(value) => value.is_finite(),
        ProxyValue::Vector { values, size } => {
            let size = usize::from(*size);
            size <= values.len() && values[..size].iter().all(|value| value.is_finite())
        }
        ProxyValue::Matrix(values) => values.iter().all(|value| value.is_finite()),
    }
}

fn model_inputs_are_finite(inputs: &ModelProxyInputs) -> bool {
    inputs.invisibility.is_none_or(|input| {
        input.factor.is_finite()
            && input
                .player_tint
                .is_none_or(|values| values.iter().all(|value| value.is_finite()))
    }) && inputs
        .model_glow_color
        .is_none_or(|values| values.iter().all(|value| value.is_finite()))
        && inputs
            .yellow_level
            .is_none_or(|values| values.iter().all(|value| value.is_finite()))
        && inputs.weapon_sheen.is_none_or(|input| {
            input.tint.iter().all(|value| value.is_finite())
                && input.mask_scale.iter().all(|value| value.is_finite())
                && input.mask_offset.iter().all(|value| value.is_finite())
        })
        && inputs.invulnerability_level.is_none_or(f32::is_finite)
        && inputs.burn_level.is_none_or(f32::is_finite)
        && inputs
            .item_tint
            .is_none_or(|values| values.iter().all(|value| value.is_finite()))
        && inputs
            .stickybomb_glow
            .is_none_or(|values| values.iter().all(|value| value.is_finite()))
}

fn lower(value: &[u8]) -> Vec<u8> {
    value.iter().map(u8::to_ascii_lowercase).collect()
}

fn evaluation_error(
    code: ProxyEvaluationErrorCode,
    operation: usize,
    variable: Option<Vec<u8>>,
) -> ProxyEvaluationError {
    ProxyEvaluationError {
        code,
        operation,
        variable,
    }
}
