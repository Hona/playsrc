use std::fmt;

const FORMAT_MAX_TOKEN_BYTES: usize = 4_095;
const FORMAT_MAX_DEPTH: usize = 101;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encoding {
    SourceBytes,
    Utf16LittleEndian,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EscapeMode {
    LiteralBackslash,
    Escaped,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_token_bytes: usize,
    pub max_depth: usize,
    pub max_nodes: usize,
    pub max_directives: usize,
    pub max_diagnostics: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_input_bytes: 16 * 1024 * 1024,
            max_decoded_bytes: 16 * 1024 * 1024,
            max_token_bytes: FORMAT_MAX_TOKEN_BYTES,
            max_depth: FORMAT_MAX_DEPTH,
            max_nodes: 262_144,
            max_directives: 4_096,
            max_diagnostics: 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub bytes: Vec<u8>,
    pub quoted: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionPlacement {
    BeforeValue,
    AfterScalar,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Condition {
    pub token: Token,
    pub symbol: Vec<u8>,
    pub negated: bool,
    pub placement: ConditionPlacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectiveKind {
    Include,
    Base,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directive {
    pub kind: DirectiveKind,
    pub keyword: Token,
    pub target: Token,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScalarKind {
    Bytes,
    Integer(i32),
    Float(u32),
    Uint64(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumericValue<'a> {
    Bytes(&'a [u8]),
    Integer(i32),
    Float(f32),
    Uint64(u64),
}

impl NumericValue<'_> {
    pub fn get_int(self) -> i32 {
        match self {
            Self::Bytes(value) => decimal_i32_prefix(value),
            Self::Integer(value) => value,
            Self::Float(value) => value as i32,
            Self::Uint64(_) => 0,
        }
    }

    pub fn get_uint64(self) -> u64 {
        match self {
            Self::Bytes(value) => source_i64_prefix(value) as u64,
            Self::Integer(value) => value as u64,
            Self::Float(value) => (value as i32) as u64,
            Self::Uint64(value) => value,
        }
    }

    pub fn get_float(self) -> f32 {
        match self {
            Self::Bytes(value) => decimal_float_prefix(value) as f32,
            Self::Integer(value) => value as f32,
            Self::Float(value) => value,
            Self::Uint64(value) => value as f32,
        }
    }

    pub fn get_bool(self) -> bool {
        self.get_int() != 0
    }
}

fn decimal_i32_prefix(value: &[u8]) -> i32 {
    let mut cursor = source_whitespace_prefix(value);
    let negative = match value.get(cursor) {
        Some(b'-') => {
            cursor += 1;
            true
        }
        Some(b'+') => {
            cursor += 1;
            false
        }
        _ => false,
    };
    let limit = if negative {
        i32::MAX as u64 + 1
    } else {
        i32::MAX as u64
    };
    let mut result = 0_u64;
    let mut found = false;
    while let Some(digit) = value.get(cursor).and_then(|byte| byte.checked_sub(b'0')) {
        if digit > 9 {
            break;
        }
        found = true;
        result = result
            .saturating_mul(10)
            .saturating_add(u64::from(digit))
            .min(limit);
        cursor += 1;
    }
    if !found {
        0
    } else if negative && result == i32::MAX as u64 + 1 {
        i32::MIN
    } else if negative {
        -(result as i32)
    } else {
        result as i32
    }
}

fn source_i64_prefix(value: &[u8]) -> i64 {
    let mut cursor = 0;
    let negative = match value.first() {
        Some(b'-') => {
            cursor = 1;
            true
        }
        Some(b'+') => {
            cursor = 1;
            false
        }
        _ => false,
    };
    let radix = if value
        .get(cursor..cursor + 2)
        .is_some_and(|prefix| prefix[0] == b'0' && matches!(prefix[1], b'x' | b'X'))
    {
        cursor += 2;
        16
    } else if value.get(cursor) == Some(&b'\'') {
        return value
            .get(cursor + 1)
            .copied()
            .map_or(0, |byte| i64::from(byte) * if negative { -1 } else { 1 });
    } else {
        10
    };
    let mut result = 0_u64;
    while let Some(digit) = value.get(cursor).and_then(|byte| match *byte {
        b'0'..=b'9' => Some(*byte - b'0'),
        b'a'..=b'f' if radix == 16 => Some(*byte - b'a' + 10),
        b'A'..=b'F' if radix == 16 => Some(*byte - b'A' + 10),
        _ => None,
    }) {
        if digit >= radix {
            break;
        }
        result = result
            .wrapping_mul(u64::from(radix))
            .wrapping_add(u64::from(digit));
        cursor += 1;
    }
    let signed = result as i64;
    if negative {
        signed.wrapping_neg()
    } else {
        signed
    }
}

/// Decimal prefix conversion before an owner-specific floating-point narrowing.
pub fn decimal_float_prefix(value: &[u8]) -> f64 {
    let start = source_whitespace_prefix(value);
    let mut cursor = start;
    if matches!(value.get(cursor), Some(b'+' | b'-')) {
        cursor += 1;
    }
    let integer_start = cursor;
    while value.get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
    }
    let mut digits = cursor - integer_start;
    if value.get(cursor) == Some(&b'.') {
        cursor += 1;
        let fraction_start = cursor;
        while value.get(cursor).is_some_and(u8::is_ascii_digit) {
            cursor += 1;
        }
        digits += cursor - fraction_start;
    }
    if digits == 0 {
        return 0.0;
    }
    if matches!(value.get(cursor), Some(b'e' | b'E')) {
        let mut exponent_end = cursor + 1;
        if matches!(value.get(exponent_end), Some(b'+' | b'-')) {
            exponent_end += 1;
        }
        let exponent_start = exponent_end;
        while value.get(exponent_end).is_some_and(u8::is_ascii_digit) {
            exponent_end += 1;
        }
        if exponent_end > exponent_start {
            cursor = exponent_end;
        }
    }
    std::str::from_utf8(&value[start..cursor])
        .ok()
        .and_then(|number| number.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn source_whitespace_prefix(value: &[u8]) -> usize {
    value
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c))
        .count()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scalar {
    pub token: Token,
    pub kind: ScalarKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Scalar(Scalar),
    Object(Vec<Node>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub key: Token,
    pub value: Value,
    pub condition: Option<Condition>,
}

impl Node {
    pub fn first_child(&self, key: &[u8]) -> Option<&Node> {
        let Value::Object(children) = &self.value else {
            return None;
        };
        children
            .iter()
            .find(|child| child.key.bytes.eq_ignore_ascii_case(key))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxDocument {
    pub encoding: Encoding,
    pub roots: Vec<Node>,
    pub directives: Vec<Directive>,
    source: Vec<u8>,
}

impl SyntaxDocument {
    pub fn unmodified_bytes(&self) -> &[u8] {
        &self.source
    }

    pub fn evaluated(&self, environment: &ConditionEnvironment) -> EvaluatedDocument {
        EvaluatedDocument {
            roots: self
                .roots
                .iter()
                .filter_map(|root| evaluate_node(root, environment))
                .collect(),
            decisions: self
                .roots
                .iter()
                .flat_map(|root| condition_decisions(root, environment))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConditionEnvironment {
    values: Vec<(Vec<u8>, bool)>,
}

impl ConditionEnvironment {
    pub fn new(values: impl IntoIterator<Item = (Vec<u8>, bool)>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }

    fn get(&self, symbol: &[u8]) -> Option<bool> {
        self.values
            .iter()
            .find_map(|(name, value)| name.eq_ignore_ascii_case(symbol).then_some(*value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConditionDecision {
    pub span: Span,
    pub mapped: bool,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedDocument {
    pub roots: Vec<Node>,
    pub decisions: Vec<ConditionDecision>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InputLimit,
    DecodedLimit,
    UnsupportedEncoding,
    InvalidUtf16,
    EmbeddedNul,
    UnexpectedToken,
    UnterminatedString,
    InvalidEscape,
    TokenLimit,
    DepthLimit,
    NodeLimit,
    DirectiveLimit,
    InvalidDirective,
    InvalidCondition,
    MissingValue,
    MissingClosingBrace,
    TrailingCondition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub code: ErrorCode,
    pub span: Span,
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "{:?} at byte {} ({}:{})",
            self.code, self.span.start, self.line, self.column
        )
    }
}

impl std::error::Error for ParseError {}

#[derive(Clone, Debug)]
struct DecodedInput {
    bytes: Vec<u8>,
    source_offsets: Vec<usize>,
    encoding: Encoding,
}

#[derive(Clone, Debug)]
enum LexemeKind {
    Text(Token),
    Condition(Token),
    Open(Span),
    Close(Span),
}

#[derive(Clone, Debug)]
struct Lexeme {
    kind: LexemeKind,
}

pub fn parse_text(
    source: &[u8],
    escape_mode: EscapeMode,
    limits: Limits,
) -> Result<SyntaxDocument, ParseError> {
    if source.len() > limits.max_input_bytes {
        return Err(error(
            source,
            ErrorCode::InputLimit,
            source.len(),
            source.len(),
        ));
    }
    let decoded = decode(source, limits)?;
    let lexemes = lex(source, &decoded, escape_mode, limits)?;
    Parser {
        source,
        lexemes: &lexemes,
        cursor: 0,
        limits,
        nodes: 0,
        directives: Vec::new(),
    }
    .document(decoded.encoding)
}

fn decode(source: &[u8], limits: Limits) -> Result<DecodedInput, ParseError> {
    if source.starts_with(&[0xfe, 0xff]) || source.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(error(
            source,
            ErrorCode::UnsupportedEncoding,
            0,
            source.len().min(3),
        ));
    }
    if !source.starts_with(&[0xff, 0xfe]) {
        if let Some(offset) = source.iter().position(|byte| *byte == 0) {
            return Err(error(source, ErrorCode::EmbeddedNul, offset, offset + 1));
        }
        if source.len() > limits.max_decoded_bytes {
            return Err(error(
                source,
                ErrorCode::DecodedLimit,
                source.len(),
                source.len(),
            ));
        }
        return Ok(DecodedInput {
            bytes: source.to_vec(),
            source_offsets: (0..=source.len()).collect(),
            encoding: Encoding::SourceBytes,
        });
    }

    if !(source.len() - 2).is_multiple_of(2) {
        return Err(error(
            source,
            ErrorCode::InvalidUtf16,
            source.len() - 1,
            source.len(),
        ));
    }
    let units: Vec<u16> = source[2..]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let mut bytes = Vec::new();
    let mut source_offsets = Vec::new();
    let mut unit_index = 0;
    while unit_index < units.len() {
        let start = 2 + unit_index * 2;
        let first = units[unit_index];
        let (character, consumed) = if (0xd800..=0xdbff).contains(&first) {
            let Some(second) = units.get(unit_index + 1).copied() else {
                return Err(error(source, ErrorCode::InvalidUtf16, start, start + 2));
            };
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(error(source, ErrorCode::InvalidUtf16, start, start + 4));
            }
            let scalar =
                0x1_0000 + (((u32::from(first) - 0xd800) << 10) | (u32::from(second) - 0xdc00));
            (char::from_u32(scalar).expect("validated Unicode scalar"), 2)
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err(error(source, ErrorCode::InvalidUtf16, start, start + 2));
        } else {
            (
                char::from_u32(u32::from(first)).expect("u16 is a Unicode scalar"),
                1,
            )
        };
        if character == '\0' {
            return Err(error(
                source,
                ErrorCode::EmbeddedNul,
                start,
                start + consumed * 2,
            ));
        }
        let mut encoded = [0_u8; 4];
        let text = character.encode_utf8(&mut encoded);
        if bytes.len() + text.len() > limits.max_decoded_bytes {
            return Err(error(
                source,
                ErrorCode::DecodedLimit,
                start,
                start + consumed * 2,
            ));
        }
        bytes.extend_from_slice(text.as_bytes());
        source_offsets.extend(std::iter::repeat_n(start, text.len()));
        unit_index += consumed;
    }
    source_offsets.push(source.len());
    Ok(DecodedInput {
        bytes,
        source_offsets,
        encoding: Encoding::Utf16LittleEndian,
    })
}

fn lex(
    source: &[u8],
    input: &DecodedInput,
    escape_mode: EscapeMode,
    limits: Limits,
) -> Result<Vec<Lexeme>, ParseError> {
    let mut lexemes = Vec::new();
    let mut cursor = 0;
    while cursor < input.bytes.len() {
        match input.bytes[cursor] {
            b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c => cursor += 1,
            b'/' if input.bytes.get(cursor + 1) == Some(&b'/') => {
                cursor += 2;
                while cursor < input.bytes.len() && !matches!(input.bytes[cursor], b'\r' | b'\n') {
                    cursor += 1;
                }
            }
            b'{' => {
                lexemes.push(Lexeme {
                    kind: LexemeKind::Open(source_span(input, cursor, cursor + 1)),
                });
                cursor += 1;
            }
            b'}' => {
                lexemes.push(Lexeme {
                    kind: LexemeKind::Close(source_span(input, cursor, cursor + 1)),
                });
                cursor += 1;
            }
            b'"' => {
                let start = cursor;
                cursor += 1;
                let mut bytes = Vec::new();
                loop {
                    let Some(byte) = input.bytes.get(cursor).copied() else {
                        let span = source_span(input, start, input.bytes.len());
                        return Err(error(
                            source,
                            ErrorCode::UnterminatedString,
                            span.start,
                            span.end,
                        ));
                    };
                    if byte == b'"' {
                        cursor += 1;
                        break;
                    }
                    if byte == b'\\' && escape_mode == EscapeMode::Escaped {
                        let escape_start = cursor;
                        cursor += 1;
                        let Some(escaped) = input.bytes.get(cursor).copied() else {
                            let span = source_span(input, escape_start, cursor);
                            return Err(error(
                                source,
                                ErrorCode::InvalidEscape,
                                span.start,
                                span.end,
                            ));
                        };
                        let decoded = match escaped {
                            b'n' => b'\n',
                            b't' => b'\t',
                            b'v' => 0x0b,
                            b'b' => 0x08,
                            b'r' => b'\r',
                            b'f' => 0x0c,
                            b'a' => 0x07,
                            b'\\' => b'\\',
                            b'?' => b'?',
                            b'\'' => b'\'',
                            b'"' => b'"',
                            _ => {
                                let span = source_span(input, escape_start, cursor + 1);
                                return Err(error(
                                    source,
                                    ErrorCode::InvalidEscape,
                                    span.start,
                                    span.end,
                                ));
                            }
                        };
                        bytes.push(decoded);
                        cursor += 1;
                    } else {
                        bytes.push(byte);
                        cursor += 1;
                    }
                    check_token_limit(source, input, start, cursor, bytes.len(), limits)?;
                }
                let token = Token {
                    bytes,
                    quoted: true,
                    span: source_span(input, start, cursor),
                };
                lexemes.push(Lexeme {
                    kind: LexemeKind::Text(token),
                });
            }
            _ => {
                let start = cursor;
                while cursor < input.bytes.len()
                    && !matches!(
                        input.bytes[cursor],
                        b' ' | b'\t' | b'\r' | b'\n' | 0x0b | 0x0c | b'{' | b'}' | b'"'
                    )
                    && !(input.bytes[cursor] == b'/' && input.bytes.get(cursor + 1) == Some(&b'/'))
                {
                    cursor += 1;
                    check_token_limit(source, input, start, cursor, cursor - start, limits)?;
                }
                if start == cursor || input.bytes[cursor.min(input.bytes.len() - 1)] == b'"' {
                    let span = source_span(input, start, (cursor + 1).min(input.bytes.len()));
                    return Err(error(
                        source,
                        ErrorCode::UnexpectedToken,
                        span.start,
                        span.end,
                    ));
                }
                let token = Token {
                    bytes: input.bytes[start..cursor].to_vec(),
                    quoted: false,
                    span: source_span(input, start, cursor),
                };
                let kind = if is_condition_token(&token.bytes) {
                    LexemeKind::Condition(token)
                } else {
                    LexemeKind::Text(token)
                };
                lexemes.push(Lexeme { kind });
            }
        }
    }
    Ok(lexemes)
}

fn check_token_limit(
    source: &[u8],
    input: &DecodedInput,
    start: usize,
    end: usize,
    decoded_len: usize,
    limits: Limits,
) -> Result<(), ParseError> {
    if decoded_len > limits.max_token_bytes.min(FORMAT_MAX_TOKEN_BYTES) {
        let span = source_span(input, start, end);
        return Err(error(source, ErrorCode::TokenLimit, span.start, span.end));
    }
    Ok(())
}

fn source_span(input: &DecodedInput, start: usize, end: usize) -> Span {
    Span {
        start: input.source_offsets[start],
        end: input.source_offsets[end],
    }
}

fn is_condition_token(bytes: &[u8]) -> bool {
    let Some(inner) = bytes
        .strip_prefix(b"[")
        .and_then(|value| value.strip_suffix(b"]"))
    else {
        return false;
    };
    let symbol = inner.strip_prefix(b"!").unwrap_or(inner);
    let Some(symbol) = symbol.strip_prefix(b"$") else {
        return false;
    };
    !symbol.is_empty()
        && symbol
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
}

struct Parser<'a> {
    source: &'a [u8],
    lexemes: &'a [Lexeme],
    cursor: usize,
    limits: Limits,
    nodes: usize,
    directives: Vec<Directive>,
}

impl Parser<'_> {
    fn document(mut self, encoding: Encoding) -> Result<SyntaxDocument, ParseError> {
        let mut roots = Vec::new();
        while self.cursor < self.lexemes.len() {
            if self.is_directive() {
                self.directive()?;
                continue;
            }
            roots.push(self.node(1, true)?);
        }
        if roots.is_empty() {
            return Err(self.here(ErrorCode::UnexpectedToken));
        }
        Ok(SyntaxDocument {
            encoding,
            roots,
            directives: self.directives,
            source: self.source.to_vec(),
        })
    }

    fn is_directive(&self) -> bool {
        let Some(Lexeme {
            kind: LexemeKind::Text(token),
        }) = self.lexemes.get(self.cursor)
        else {
            return false;
        };
        !token.quoted
            && (token.bytes.eq_ignore_ascii_case(b"#include")
                || token.bytes.eq_ignore_ascii_case(b"#base"))
    }

    fn directive(&mut self) -> Result<(), ParseError> {
        if self.directives.len() >= self.limits.max_directives {
            return Err(self.here(ErrorCode::DirectiveLimit));
        }
        let keyword = self.text(ErrorCode::InvalidDirective)?;
        let target = self.text(ErrorCode::InvalidDirective)?;
        if target.bytes.is_empty() {
            return Err(at(self.source, ErrorCode::InvalidDirective, target.span));
        }
        let kind = if keyword.bytes.eq_ignore_ascii_case(b"#include") {
            DirectiveKind::Include
        } else {
            DirectiveKind::Base
        };
        self.directives.push(Directive {
            kind,
            keyword,
            target,
        });
        Ok(())
    }

    fn node(&mut self, depth: usize, root: bool) -> Result<Node, ParseError> {
        if depth > self.limits.max_depth.min(FORMAT_MAX_DEPTH) {
            return Err(self.here(ErrorCode::DepthLimit));
        }
        if self.nodes >= self.limits.max_nodes {
            return Err(self.here(ErrorCode::NodeLimit));
        }
        self.nodes += 1;
        let key = self.text(ErrorCode::UnexpectedToken)?;
        if key.bytes.is_empty() {
            return Err(at(self.source, ErrorCode::UnexpectedToken, key.span));
        }
        let mut condition = self.condition(ConditionPlacement::BeforeValue)?;
        let value = match self.lexemes.get(self.cursor) {
            Some(Lexeme {
                kind: LexemeKind::Open(_),
            }) => {
                self.cursor += 1;
                let children = self.object(depth + 1)?;
                if !root
                    && matches!(
                        self.lexemes.get(self.cursor),
                        Some(Lexeme {
                            kind: LexemeKind::Condition(_)
                        })
                    )
                {
                    return Err(self.here(ErrorCode::TrailingCondition));
                }
                Value::Object(children)
            }
            Some(Lexeme {
                kind: LexemeKind::Text(_),
            }) => {
                if root {
                    return Err(self.here(ErrorCode::UnexpectedToken));
                }
                let token = self.text(ErrorCode::MissingValue)?;
                let scalar = Scalar {
                    kind: infer_scalar(&token.bytes),
                    token,
                };
                if condition.is_none() {
                    condition = self.condition(ConditionPlacement::AfterScalar)?;
                } else if matches!(
                    self.lexemes.get(self.cursor),
                    Some(Lexeme {
                        kind: LexemeKind::Condition(_)
                    })
                ) {
                    return Err(self.here(ErrorCode::InvalidCondition));
                }
                Value::Scalar(scalar)
            }
            _ => return Err(self.here(ErrorCode::MissingValue)),
        };
        Ok(Node {
            key,
            value,
            condition,
        })
    }

    fn object(&mut self, depth: usize) -> Result<Vec<Node>, ParseError> {
        if depth > self.limits.max_depth.min(FORMAT_MAX_DEPTH) {
            return Err(self.here(ErrorCode::DepthLimit));
        }
        let mut children = Vec::new();
        loop {
            match self.lexemes.get(self.cursor) {
                Some(Lexeme {
                    kind: LexemeKind::Close(_),
                }) => {
                    self.cursor += 1;
                    return Ok(children);
                }
                None => return Err(self.here(ErrorCode::MissingClosingBrace)),
                Some(Lexeme {
                    kind: LexemeKind::Text(token),
                }) if !token.quoted
                    && (token.bytes.eq_ignore_ascii_case(b"#include")
                        || token.bytes.eq_ignore_ascii_case(b"#base")) =>
                {
                    return Err(self.here(ErrorCode::InvalidDirective));
                }
                Some(Lexeme {
                    kind: LexemeKind::Text(_),
                }) => children.push(self.node(depth, false)?),
                _ => return Err(self.here(ErrorCode::UnexpectedToken)),
            }
        }
    }

    fn text(&mut self, code: ErrorCode) -> Result<Token, ParseError> {
        let Some(Lexeme {
            kind: LexemeKind::Text(token),
        }) = self.lexemes.get(self.cursor)
        else {
            return Err(self.here(code));
        };
        self.cursor += 1;
        Ok(token.clone())
    }

    fn condition(
        &mut self,
        placement: ConditionPlacement,
    ) -> Result<Option<Condition>, ParseError> {
        let Some(Lexeme {
            kind: LexemeKind::Condition(token),
        }) = self.lexemes.get(self.cursor)
        else {
            return Ok(None);
        };
        self.cursor += 1;
        let inner = &token.bytes[1..token.bytes.len() - 1];
        let (negated, symbol) = inner
            .strip_prefix(b"!")
            .map_or((false, inner), |symbol| (true, symbol));
        Ok(Some(Condition {
            token: token.clone(),
            symbol: symbol.to_vec(),
            negated,
            placement,
        }))
    }

    fn here(&self, code: ErrorCode) -> ParseError {
        let span = match self.lexemes.get(self.cursor) {
            Some(Lexeme {
                kind: LexemeKind::Text(token) | LexemeKind::Condition(token),
            }) => token.span,
            Some(Lexeme {
                kind: LexemeKind::Open(span) | LexemeKind::Close(span),
            }) => *span,
            None => Span {
                start: self.source.len(),
                end: self.source.len(),
            },
        };
        at(self.source, code, span)
    }
}

fn infer_scalar(bytes: &[u8]) -> ScalarKind {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return ScalarKind::Bytes;
    };
    if text.len() == 18
        && text.starts_with("0x")
        && text[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return u64::from_str_radix(&text[2..], 16)
            .map(ScalarKind::Uint64)
            .unwrap_or(ScalarKind::Bytes);
    }
    if let Ok(value) = text.parse::<i32>() {
        return ScalarKind::Integer(value);
    }
    if let Ok(value) = text.parse::<f32>()
        && value.is_finite()
        && (text.contains('.') || text.contains('e') || text.contains('E'))
    {
        return ScalarKind::Float(value.to_bits());
    }
    ScalarKind::Bytes
}

fn condition_active(condition: &Condition, environment: &ConditionEnvironment) -> (bool, bool) {
    let Some(value) = environment.get(&condition.symbol) else {
        return (false, false);
    };
    (true, value ^ condition.negated)
}

fn evaluate_node(node: &Node, environment: &ConditionEnvironment) -> Option<Node> {
    if let Some(condition) = &node.condition {
        let (_, active) = condition_active(condition, environment);
        if !active {
            return None;
        }
    }
    let mut evaluated = node.clone();
    if let Value::Object(children) = &node.value {
        evaluated.value = Value::Object(
            children
                .iter()
                .filter_map(|child| evaluate_node(child, environment))
                .collect(),
        );
    }
    Some(evaluated)
}

fn condition_decisions(node: &Node, environment: &ConditionEnvironment) -> Vec<ConditionDecision> {
    let mut decisions = Vec::new();
    if let Some(condition) = &node.condition {
        let (mapped, active) = condition_active(condition, environment);
        decisions.push(ConditionDecision {
            span: condition.token.span,
            mapped,
            active,
        });
    }
    if let Value::Object(children) = &node.value {
        for child in children {
            decisions.extend(condition_decisions(child, environment));
        }
    }
    decisions
}

fn error(source: &[u8], code: ErrorCode, start: usize, end: usize) -> ParseError {
    at(source, code, Span { start, end })
}

fn at(source: &[u8], code: ErrorCode, span: Span) -> ParseError {
    let before = &source[..span.start.min(source.len())];
    let line = 1 + before.iter().filter(|byte| **byte == b'\n').count();
    let line_start = before
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |offset| offset + 1);
    ParseError {
        code,
        span,
        line,
        column: span.start.saturating_sub(line_start) + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &[u8]) -> Result<SyntaxDocument, ParseError> {
        parse_text(source, EscapeMode::LiteralBackslash, Limits::default())
    }

    #[test]
    fn preserves_roots_children_repeats_empty_values_and_source() {
        let source = br#"// comment
"Root" { "Key" "first" key "second" empty "" object {} }
Second {}
"#;
        let document = parse(source).unwrap();
        assert_eq!(document.roots.len(), 2);
        let Value::Object(children) = &document.roots[0].value else {
            panic!("root must be an object");
        };
        assert_eq!(children.len(), 4);
        assert_eq!(document.roots[0].first_child(b"KEY"), Some(&children[0]));
        assert!(
            matches!(children[2].value, Value::Scalar(Scalar { ref token, .. }) if token.bytes.is_empty())
        );
        assert!(matches!(children[3].value, Value::Object(ref values) if values.is_empty()));
        assert_eq!(document.unmodified_bytes(), source);
    }

    #[test]
    fn distinguishes_literal_and_escaped_strings() {
        let source = br#"root { value "a\nb\\d" }"#;
        let literal = parse_text(source, EscapeMode::LiteralBackslash, Limits::default()).unwrap();
        let escaped = parse_text(source, EscapeMode::Escaped, Limits::default()).unwrap();
        let scalar = |document: &SyntaxDocument| {
            let Value::Object(children) = &document.roots[0].value else {
                panic!()
            };
            let Value::Scalar(value) = &children[0].value else {
                panic!()
            };
            value.token.bytes.clone()
        };
        assert_eq!(scalar(&literal), br#"a\nb\\d"#.to_vec());
        assert_eq!(scalar(&escaped), b"a\nb\\d");
    }

    #[test]
    fn rejects_unknown_escape() {
        let error = parse_text(
            br#"root { value "\x" }"#,
            EscapeMode::Escaped,
            Limits::default(),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidEscape);
    }

    #[test]
    fn decodes_utf16_little_endian_and_retains_original_spans() {
        let mut source = vec![0xff, 0xfe];
        for unit in "root { key \"value\" }".encode_utf16() {
            source.extend_from_slice(&unit.to_le_bytes());
        }
        let document = parse(&source).unwrap();
        assert_eq!(document.encoding, Encoding::Utf16LittleEndian);
        assert_eq!(document.roots[0].key.span, Span { start: 2, end: 10 });
        assert_eq!(document.unmodified_bytes(), source);
    }

    #[test]
    fn rejects_other_boms_nul_odd_utf16_and_surrogates() {
        for (source, code) in [
            (
                &[0xef, 0xbb, 0xbf, b'a'][..],
                ErrorCode::UnsupportedEncoding,
            ),
            (&[0xfe, 0xff, 0, b'a'][..], ErrorCode::UnsupportedEncoding),
            (&[b'a', 0][..], ErrorCode::EmbeddedNul),
            (&[0xff, 0xfe, b'a'][..], ErrorCode::InvalidUtf16),
            (&[0xff, 0xfe, 0x00, 0xd8][..], ErrorCode::InvalidUtf16),
        ] {
            assert_eq!(parse(source).unwrap_err().code, code);
        }
    }

    #[test]
    fn infers_complete_scalar_types_without_coercing_other_bytes() {
        let document = parse(
            br#"root { int -2147483648 over 2147483648 float 1.25 exp 1e2 plain 1 hex 0x0123456789ABCDEF badhex 0xZ123456789ABCDEF inf inf }"#,
        )
        .unwrap();
        let Value::Object(children) = &document.roots[0].value else {
            panic!()
        };
        assert_eq!(scalar_kind(&children[0]), ScalarKind::Integer(i32::MIN));
        assert_eq!(scalar_kind(&children[1]), ScalarKind::Bytes);
        assert_eq!(
            scalar_kind(&children[2]),
            ScalarKind::Float(1.25_f32.to_bits())
        );
        assert_eq!(
            scalar_kind(&children[3]),
            ScalarKind::Float(100_f32.to_bits())
        );
        assert_eq!(scalar_kind(&children[4]), ScalarKind::Integer(1));
        assert_eq!(
            scalar_kind(&children[5]),
            ScalarKind::Uint64(0x0123_4567_89ab_cdef)
        );
        assert_eq!(scalar_kind(&children[6]), ScalarKind::Bytes);
        assert_eq!(scalar_kind(&children[7]), ScalarKind::Bytes);
    }

    #[test]
    fn source_numeric_accessors_preserve_prefix_default_and_native_type_contracts() {
        for (source, expected) in [
            (&b"  +42suffix"[..], 42),
            (&b"-17.5"[..], -17),
            (&b"1e3"[..], 1),
            (&b"descriptive text"[..], 0),
            (&b""[..], 0),
            (&b"2147483648"[..], i32::MAX),
            (&b"-2147483649"[..], i32::MIN),
        ] {
            assert_eq!(NumericValue::Bytes(source).get_int(), expected);
        }
        assert!(!NumericValue::Bytes(b"Allow entities that match criteria").get_bool());
        assert!(NumericValue::Bytes(b" -2 trailing").get_bool());
        assert_eq!(NumericValue::Integer(-12).get_int(), -12);
        assert_eq!(NumericValue::Float(12.75).get_int(), 12);
        assert_eq!(NumericValue::Uint64(u64::MAX).get_int(), 0);

        assert_eq!(NumericValue::Bytes(b"15tail").get_float(), 15.0);
        assert_eq!(NumericValue::Bytes(b" -1.25e2suffix").get_float(), -125.0);
        assert_eq!(NumericValue::Bytes(b"1e suffix").get_float(), 1.0);
        assert_eq!(NumericValue::Bytes(b"text").get_float(), 0.0);
        assert!(NumericValue::Bytes(b"1e9999").get_float().is_infinite());
        assert_eq!(NumericValue::Integer(7).get_float(), 7.0);
        assert_eq!(NumericValue::Float(2.5).get_float(), 2.5);

        assert_eq!(NumericValue::Bytes(b"123tail").get_uint64(), 123);
        assert_eq!(NumericValue::Bytes(b"0x10tail").get_uint64(), 16);
        assert_eq!(NumericValue::Bytes(b"'Arest").get_uint64(), 65);
        assert_eq!(NumericValue::Bytes(b" 12").get_uint64(), 0);
        assert_eq!(NumericValue::Integer(-1).get_uint64(), u64::MAX);
        assert_eq!(NumericValue::Float(3.75).get_uint64(), 3);
        assert_eq!(NumericValue::Uint64(u64::MAX).get_uint64(), u64::MAX);
    }

    #[test]
    fn preserves_and_evaluates_conditions_without_mutating_syntax() {
        let document = parse(
            br#"enabled [$WIN32] { yes 1 no 0 [$LINUX] unknown 2 [!$MISSING] } disabled [!$WIN32] {}"#,
        )
        .unwrap();
        let environment =
            ConditionEnvironment::new([(b"$win32".to_vec(), true), (b"$linux".to_vec(), false)]);
        let evaluated = document.evaluated(&environment);
        assert_eq!(evaluated.roots.len(), 1);
        let Value::Object(children) = &evaluated.roots[0].value else {
            panic!()
        };
        assert_eq!(children.len(), 1);
        assert_eq!(document.roots.len(), 2);
        assert_eq!(evaluated.decisions.len(), 4);
        assert!(!evaluated.decisions[2].mapped);
        assert!(!evaluated.decisions[2].active);
    }

    #[test]
    fn records_ordered_top_level_directives() {
        let document = parse(br#"#base "base.txt" root {} #include child.txt second {}"#).unwrap();
        assert_eq!(document.directives.len(), 2);
        assert_eq!(document.directives[0].kind, DirectiveKind::Base);
        assert_eq!(document.directives[1].target.bytes, b"child.txt");
        assert_eq!(document.roots.len(), 2);
    }

    #[test]
    fn rejects_directives_inside_objects() {
        assert_eq!(
            parse(br#"root { #include "child.txt" }"#).unwrap_err().code,
            ErrorCode::InvalidDirective
        );
    }

    #[test]
    fn rejects_structural_failures_at_exact_locations() {
        let cases = [
            (&br#"root { key }"#[..], ErrorCode::MissingValue),
            (&br#"root { key value"#[..], ErrorCode::MissingClosingBrace),
            (
                &br#"root { child {} [$WIN32] }"#[..],
                ErrorCode::TrailingCondition,
            ),
            (&br#"[$WIN32] root {}"#[..], ErrorCode::UnexpectedToken),
            (&br#"root scalar"#[..], ErrorCode::UnexpectedToken),
        ];
        for (source, expected) in cases {
            let error = parse(source).unwrap_err();
            assert_eq!(
                error.code,
                expected,
                "source: {}",
                String::from_utf8_lossy(source)
            );
            assert!(error.span.start <= error.span.end);
            assert!(error.line >= 1 && error.column >= 1);
        }
    }

    #[test]
    fn enforces_input_decoded_token_depth_node_and_directive_limits() {
        let limits = Limits {
            max_input_bytes: 1,
            ..Limits::default()
        };
        assert_eq!(
            parse_text(b"root {}", EscapeMode::LiteralBackslash, limits)
                .unwrap_err()
                .code,
            ErrorCode::InputLimit
        );

        let limits = Limits {
            max_token_bytes: 3,
            ..Limits::default()
        };
        assert_eq!(
            parse_text(b"root {}", EscapeMode::LiteralBackslash, limits)
                .unwrap_err()
                .code,
            ErrorCode::TokenLimit
        );

        let limits = Limits {
            max_depth: 1,
            ..Limits::default()
        };
        assert_eq!(
            parse_text(b"root { child {} }", EscapeMode::LiteralBackslash, limits)
                .unwrap_err()
                .code,
            ErrorCode::DepthLimit
        );

        let limits = Limits {
            max_nodes: 1,
            ..Limits::default()
        };
        assert_eq!(
            parse_text(b"root { key value }", EscapeMode::LiteralBackslash, limits)
                .unwrap_err()
                .code,
            ErrorCode::NodeLimit
        );

        let limits = Limits {
            max_directives: 0,
            ..Limits::default()
        };
        assert_eq!(
            parse_text(b"#include x root {}", EscapeMode::LiteralBackslash, limits)
                .unwrap_err()
                .code,
            ErrorCode::DirectiveLimit
        );
    }

    #[test]
    fn accepts_format_token_and_depth_boundaries() {
        let token = vec![b'a'; FORMAT_MAX_TOKEN_BYTES];
        let mut source = b"root { key \"".to_vec();
        source.extend_from_slice(&token);
        source.extend_from_slice(b"\" }");
        assert!(parse(&source).is_ok());

        let mut too_long = b"root { key \"".to_vec();
        too_long.extend(std::iter::repeat_n(b'a', FORMAT_MAX_TOKEN_BYTES + 1));
        too_long.extend_from_slice(b"\" }");
        assert_eq!(parse(&too_long).unwrap_err().code, ErrorCode::TokenLimit);
    }

    fn scalar_kind(node: &Node) -> ScalarKind {
        let Value::Scalar(value) = &node.value else {
            panic!()
        };
        value.kind.clone()
    }
}
