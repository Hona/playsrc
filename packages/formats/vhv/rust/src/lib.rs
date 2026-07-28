//! Bounded parsing for canonical little-endian PC Source 1 VHV version 2 files.
//!
//! The format contract is declared by Valve Source SDK 2013 in
//! `src/public/materialsystem/hardwareverts.h` and written by
//! `src/utils/vrad/vradstaticprops.cpp`.

use std::{fmt, mem::size_of, ops::Range, sync::Arc};

use sha2::{Digest, Sha256};

pub const VERSION: i32 = 2;
pub const VERTEX_COLOR_FLAG: u32 = 0x0004;
pub const HEADER_BYTES: usize = 40;
pub const MESH_HEADER_BYTES: usize = 28;
pub const VERTEX_BYTES: usize = 4;
pub const STREAM_ALIGNMENT: usize = 512;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Profile {
    pub expected_mdl_checksum: u32,
}

impl Profile {
    pub const fn source_pc_v2_color_bgra8888(expected_mdl_checksum: u32) -> Self {
        Self {
            expected_mdl_checksum,
        }
    }

    pub const fn identity(self) -> &'static str {
        "source-pc-v2-color-bgra8888"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_input_bytes: usize,
    pub max_retained_bytes: usize,
    pub max_meshes: usize,
    pub max_total_vertices: usize,
    pub max_vertices_per_mesh: usize,
    pub max_lod: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Disposition {
    Empty,
    Populated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VertexEncoding {
    SourceVertexLightBgra8888Opaque,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceIdentity {
    pub byte_length: u64,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Header {
    pub range: Range<usize>,
    pub version: i32,
    pub checksum: u32,
    pub vertex_flags: u32,
    pub vertex_size: u32,
    pub total_vertices: u32,
    pub mesh_count: i32,
    pub reserved: [u32; 4],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mesh {
    pub ordinal: usize,
    pub header_range: Range<usize>,
    pub lod: u32,
    pub vertex_count: u32,
    pub data_range: Range<usize>,
    pub reserved: [u32; 4],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VertexColor {
    pub mesh_ordinal: usize,
    pub local_vertex: usize,
    pub range: Range<usize>,
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub alpha: u8,
    pub encoding: VertexEncoding,
}

impl VertexColor {
    pub fn encoded_bgra(&self) -> [u8; 4] {
        [self.blue, self.green, self.red, self.alpha]
    }

    /// Returns normalized encoded vertex-light channels. RGB remains in Source's
    /// vertex-light transfer space; this operation does not produce linear light.
    pub fn normalized_bgra(&self) -> [f32; 4] {
        self.encoded_bgra()
            .map(|channel| f32::from(channel) / 255.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Vhv {
    pub profile: Profile,
    pub source_identity: SourceIdentity,
    pub header: Header,
    pub header_padding_range: Range<usize>,
    pub meshes: Vec<Mesh>,
    pub trailing_padding_range: Range<usize>,
    pub disposition: Disposition,
    source: Arc<[u8]>,
}

impl Vhv {
    pub fn source_bytes(&self) -> &[u8] {
        &self.source
    }

    pub fn mesh_bytes(&self, mesh_ordinal: usize) -> Option<&[u8]> {
        self.meshes
            .get(mesh_ordinal)
            .map(|mesh| &self.source[mesh.data_range.clone()])
    }

    pub fn colors(&self, mesh_ordinal: usize) -> Option<VertexColors<'_>> {
        let mesh = self.meshes.get(mesh_ordinal)?;
        Some(VertexColors {
            source: &self.source,
            mesh_ordinal,
            data_start: mesh.data_range.start,
            next: 0,
            count: mesh.vertex_count as usize,
        })
    }
}

pub struct VertexColors<'a> {
    source: &'a [u8],
    mesh_ordinal: usize,
    data_start: usize,
    next: usize,
    count: usize,
}

impl Iterator for VertexColors<'_> {
    type Item = VertexColor;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.count {
            return None;
        }
        let local_vertex = self.next;
        let start = self.data_start + local_vertex * VERTEX_BYTES;
        self.next += 1;
        Some(VertexColor {
            mesh_ordinal: self.mesh_ordinal,
            local_vertex,
            range: start..start + VERTEX_BYTES,
            blue: self.source[start],
            green: self.source[start + 1],
            red: self.source[start + 2],
            alpha: self.source[start + 3],
            encoding: VertexEncoding::SourceVertexLightBgra8888Opaque,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.count - self.next;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for VertexColors<'_> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    Malformed,
    Unsupported,
    ChecksumMismatch,
    NonCanonical,
    Limit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidLimits,
    InputLimit,
    TruncatedHeader,
    UnsupportedVersion,
    ChecksumMismatch,
    UnsupportedVertexFlags,
    UnsupportedVertexSize,
    NegativeMeshCount,
    MeshLimit,
    TotalVertexLimit,
    RetainedBytesLimit,
    NonzeroHeaderReserved,
    TruncatedMeshTable,
    NonzeroMeshReserved,
    LodLimit,
    MeshVertexLimit,
    VertexCountOverflow,
    VertexTotalMismatch,
    NonzeroHeaderPadding,
    VertexRangeOverflow,
    TruncatedVertexRange,
    DuplicateMeshRange,
    OverlappingMeshRange,
    MeshDataGap,
    NonOpaqueAlpha,
    FileExtentOverflow,
    TruncatedFileExtent,
    TrailingBytes,
    NonzeroTrailingPadding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub classification: Classification,
    pub code: ErrorCode,
    pub range: Range<usize>,
    pub mesh_ordinal: Option<usize>,
    pub declared: Option<usize>,
    pub limit: Option<usize>,
}

impl fmt::Display for ParseError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            output,
            "{:?}/{:?} at {}..{}",
            self.classification, self.code, self.range.start, self.range.end
        )
    }
}

impl std::error::Error for ParseError {}

pub fn parse(bytes: &[u8], profile: Profile, limits: Limits) -> Result<Vhv, ParseError> {
    validate_limits(limits)?;
    if bytes.len() > limits.max_input_bytes {
        return Err(failure(
            Classification::Limit,
            ErrorCode::InputLimit,
            0..bytes.len(),
            None,
            Some(bytes.len()),
            Some(limits.max_input_bytes),
        ));
    }
    if bytes.len() < HEADER_BYTES {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::TruncatedHeader,
            bytes.len()..HEADER_BYTES,
            None,
            Some(bytes.len()),
            Some(HEADER_BYTES),
        ));
    }

    let version = i32_at(bytes, 0);
    if version != VERSION {
        return Err(simple(
            Classification::Unsupported,
            ErrorCode::UnsupportedVersion,
            0..4,
        ));
    }
    let checksum = u32_at(bytes, 4);
    if checksum != profile.expected_mdl_checksum {
        return Err(simple(
            Classification::ChecksumMismatch,
            ErrorCode::ChecksumMismatch,
            4..8,
        ));
    }
    let vertex_flags = u32_at(bytes, 8);
    if vertex_flags != VERTEX_COLOR_FLAG {
        return Err(simple(
            Classification::Unsupported,
            ErrorCode::UnsupportedVertexFlags,
            8..12,
        ));
    }
    let vertex_size = u32_at(bytes, 12);
    if vertex_size != VERTEX_BYTES as u32 {
        return Err(simple(
            Classification::Unsupported,
            ErrorCode::UnsupportedVertexSize,
            12..16,
        ));
    }
    let total_vertices = u32_at(bytes, 16);
    if total_vertices as usize > limits.max_total_vertices {
        return Err(failure(
            Classification::Limit,
            ErrorCode::TotalVertexLimit,
            16..20,
            None,
            Some(total_vertices as usize),
            Some(limits.max_total_vertices),
        ));
    }
    let mesh_count = i32_at(bytes, 20);
    if mesh_count < 0 {
        return Err(simple(
            Classification::Malformed,
            ErrorCode::NegativeMeshCount,
            20..24,
        ));
    }
    let mesh_count_usize = mesh_count as usize;
    if mesh_count_usize > limits.max_meshes {
        return Err(failure(
            Classification::Limit,
            ErrorCode::MeshLimit,
            20..24,
            None,
            Some(mesh_count_usize),
            Some(limits.max_meshes),
        ));
    }
    let reserved = [
        u32_at(bytes, 24),
        u32_at(bytes, 28),
        u32_at(bytes, 32),
        u32_at(bytes, 36),
    ];
    if let Some(index) = reserved.iter().position(|value| *value != 0) {
        let start = 24 + index * 4;
        return Err(simple(
            Classification::NonCanonical,
            ErrorCode::NonzeroHeaderReserved,
            start..start + 4,
        ));
    }

    let table_bytes = mesh_count_usize
        .checked_mul(MESH_HEADER_BYTES)
        .ok_or_else(|| {
            simple(
                Classification::Malformed,
                ErrorCode::TruncatedMeshTable,
                20..24,
            )
        })?;
    let table_end = HEADER_BYTES.checked_add(table_bytes).ok_or_else(|| {
        simple(
            Classification::Malformed,
            ErrorCode::TruncatedMeshTable,
            20..24,
        )
    })?;
    if table_end > bytes.len() {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::TruncatedMeshTable,
            bytes.len()..table_end,
            None,
            Some(table_end),
            Some(bytes.len()),
        ));
    }
    let payload_start = align_up(table_end, STREAM_ALIGNMENT).ok_or_else(|| {
        simple(
            Classification::Malformed,
            ErrorCode::FileExtentOverflow,
            20..24,
        )
    })?;
    if payload_start > bytes.len() {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::TruncatedFileExtent,
            bytes.len()..payload_start,
            None,
            Some(payload_start),
            Some(bytes.len()),
        ));
    }
    if let Some(offset) = first_nonzero(bytes, table_end..payload_start) {
        return Err(simple(
            Classification::NonCanonical,
            ErrorCode::NonzeroHeaderPadding,
            offset..offset + 1,
        ));
    }

    let retained_bytes = bytes
        .len()
        .checked_add(
            mesh_count_usize
                .checked_mul(size_of::<Mesh>())
                .ok_or_else(|| {
                    simple(Classification::Limit, ErrorCode::RetainedBytesLimit, 20..24)
                })?,
        )
        .ok_or_else(|| simple(Classification::Limit, ErrorCode::RetainedBytesLimit, 20..24))?;
    if retained_bytes > limits.max_retained_bytes {
        return Err(failure(
            Classification::Limit,
            ErrorCode::RetainedBytesLimit,
            0..bytes.len(),
            None,
            Some(retained_bytes),
            Some(limits.max_retained_bytes),
        ));
    }

    let mut meshes = Vec::with_capacity(mesh_count_usize);
    let mut vertex_sum = 0_usize;
    let mut expected_offset = payload_start;
    for ordinal in 0..mesh_count_usize {
        let header_start = HEADER_BYTES + ordinal * MESH_HEADER_BYTES;
        let header_range = header_start..header_start + MESH_HEADER_BYTES;
        let lod = u32_at(bytes, header_start);
        if lod as usize > limits.max_lod {
            return Err(failure(
                Classification::Limit,
                ErrorCode::LodLimit,
                header_start..header_start + 4,
                Some(ordinal),
                Some(lod as usize),
                Some(limits.max_lod),
            ));
        }
        let vertex_count = u32_at(bytes, header_start + 4);
        if vertex_count as usize > limits.max_vertices_per_mesh {
            return Err(failure(
                Classification::Limit,
                ErrorCode::MeshVertexLimit,
                header_start + 4..header_start + 8,
                Some(ordinal),
                Some(vertex_count as usize),
                Some(limits.max_vertices_per_mesh),
            ));
        }
        vertex_sum = vertex_sum
            .checked_add(vertex_count as usize)
            .ok_or_else(|| {
                simple_mesh(
                    Classification::Malformed,
                    ErrorCode::VertexCountOverflow,
                    header_start + 4..header_start + 8,
                    ordinal,
                )
            })?;
        if vertex_sum > limits.max_total_vertices {
            return Err(failure(
                Classification::Limit,
                ErrorCode::TotalVertexLimit,
                header_start + 4..header_start + 8,
                Some(ordinal),
                Some(vertex_sum),
                Some(limits.max_total_vertices),
            ));
        }
        let data_start = u32_at(bytes, header_start + 8) as usize;
        let mesh_reserved = [
            u32_at(bytes, header_start + 12),
            u32_at(bytes, header_start + 16),
            u32_at(bytes, header_start + 20),
            u32_at(bytes, header_start + 24),
        ];
        if let Some(index) = mesh_reserved.iter().position(|value| *value != 0) {
            let start = header_start + 12 + index * 4;
            return Err(simple_mesh(
                Classification::NonCanonical,
                ErrorCode::NonzeroMeshReserved,
                start..start + 4,
                ordinal,
            ));
        }
        let data_bytes = (vertex_count as usize)
            .checked_mul(VERTEX_BYTES)
            .ok_or_else(|| {
                simple_mesh(
                    Classification::Malformed,
                    ErrorCode::VertexRangeOverflow,
                    header_start + 4..header_start + 12,
                    ordinal,
                )
            })?;
        let data_end = data_start.checked_add(data_bytes).ok_or_else(|| {
            simple_mesh(
                Classification::Malformed,
                ErrorCode::VertexRangeOverflow,
                header_start + 4..header_start + 12,
                ordinal,
            )
        })?;
        let data_range = data_start..data_end;

        if data_start < expected_offset && !data_range.is_empty() {
            let duplicate = meshes
                .iter()
                .any(|prior: &Mesh| !prior.data_range.is_empty() && prior.data_range == data_range);
            return Err(simple_mesh(
                Classification::NonCanonical,
                if duplicate {
                    ErrorCode::DuplicateMeshRange
                } else {
                    ErrorCode::OverlappingMeshRange
                },
                header_start + 8..header_start + 12,
                ordinal,
            ));
        }
        if data_start > expected_offset {
            return Err(simple_mesh(
                Classification::NonCanonical,
                ErrorCode::MeshDataGap,
                expected_offset..data_start,
                ordinal,
            ));
        }
        if data_end > bytes.len() {
            return Err(failure(
                Classification::Malformed,
                ErrorCode::TruncatedVertexRange,
                data_start..data_end,
                Some(ordinal),
                Some(data_end),
                Some(bytes.len()),
            ));
        }
        for local_vertex in 0..vertex_count as usize {
            let alpha = data_start + local_vertex * VERTEX_BYTES + 3;
            if bytes[alpha] != u8::MAX {
                return Err(simple_mesh(
                    Classification::NonCanonical,
                    ErrorCode::NonOpaqueAlpha,
                    alpha..alpha + 1,
                    ordinal,
                ));
            }
        }

        expected_offset = data_end;
        meshes.push(Mesh {
            ordinal,
            header_range,
            lod,
            vertex_count,
            data_range,
            reserved: mesh_reserved,
        });
    }

    if vertex_sum != total_vertices as usize {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::VertexTotalMismatch,
            16..20,
            None,
            Some(vertex_sum),
            Some(total_vertices as usize),
        ));
    }
    let file_extent = align_up(expected_offset, STREAM_ALIGNMENT).ok_or_else(|| {
        simple(
            Classification::Malformed,
            ErrorCode::FileExtentOverflow,
            expected_offset..usize::MAX,
        )
    })?;
    if bytes.len() < file_extent {
        return Err(failure(
            Classification::Malformed,
            ErrorCode::TruncatedFileExtent,
            bytes.len()..file_extent,
            None,
            Some(file_extent),
            Some(bytes.len()),
        ));
    }
    if bytes.len() > file_extent {
        return Err(failure(
            Classification::NonCanonical,
            ErrorCode::TrailingBytes,
            file_extent..bytes.len(),
            None,
            Some(bytes.len()),
            Some(file_extent),
        ));
    }
    if let Some(offset) = first_nonzero(bytes, expected_offset..file_extent) {
        return Err(simple(
            Classification::NonCanonical,
            ErrorCode::NonzeroTrailingPadding,
            offset..offset + 1,
        ));
    }

    let digest: [u8; 32] = Sha256::digest(bytes).into();
    Ok(Vhv {
        profile,
        source_identity: SourceIdentity {
            byte_length: bytes.len() as u64,
            sha256: digest,
        },
        header: Header {
            range: 0..HEADER_BYTES,
            version,
            checksum,
            vertex_flags,
            vertex_size,
            total_vertices,
            mesh_count,
            reserved,
        },
        header_padding_range: table_end..payload_start,
        meshes,
        trailing_padding_range: expected_offset..file_extent,
        disposition: if total_vertices == 0 {
            Disposition::Empty
        } else {
            Disposition::Populated
        },
        source: Arc::from(bytes),
    })
}

fn validate_limits(limits: Limits) -> Result<(), ParseError> {
    let structural_mesh_max = i32::MAX as usize;
    let structural_vertex_max = u32::MAX as usize;
    let structural_lod_max = u32::MAX as usize;
    if limits.max_meshes > structural_mesh_max
        || limits.max_total_vertices > structural_vertex_max
        || limits.max_vertices_per_mesh > structural_vertex_max
        || limits.max_lod > structural_lod_max
    {
        return Err(failure(
            Classification::Limit,
            ErrorCode::InvalidLimits,
            0..0,
            None,
            None,
            None,
        ));
    }
    Ok(())
}

fn align_up(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
}

fn first_nonzero(bytes: &[u8], range: Range<usize>) -> Option<usize> {
    bytes[range.clone()]
        .iter()
        .position(|value| *value != 0)
        .map(|offset| range.start + offset)
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated range"),
    )
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated range"),
    )
}

fn simple(classification: Classification, code: ErrorCode, range: Range<usize>) -> ParseError {
    failure(classification, code, range, None, None, None)
}

fn simple_mesh(
    classification: Classification,
    code: ErrorCode,
    range: Range<usize>,
    mesh_ordinal: usize,
) -> ParseError {
    failure(classification, code, range, Some(mesh_ordinal), None, None)
}

fn failure(
    classification: Classification,
    code: ErrorCode,
    range: Range<usize>,
    mesh_ordinal: Option<usize>,
    declared: Option<usize>,
    limit: Option<usize>,
) -> ParseError {
    ParseError {
        classification,
        code,
        range,
        mesh_ordinal,
        declared,
        limit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECKSUM: u32 = 0x89ab_cdef;

    fn limits() -> Limits {
        Limits {
            max_input_bytes: 8 * 1024,
            max_retained_bytes: 16 * 1024,
            max_meshes: 16,
            max_total_vertices: 64,
            max_vertices_per_mesh: 32,
            max_lod: 7,
        }
    }

    fn fixture(meshes: &[(u32, &[[u8; 4]])]) -> Vec<u8> {
        let table_end = HEADER_BYTES + meshes.len() * MESH_HEADER_BYTES;
        let payload_start = align_up(table_end, STREAM_ALIGNMENT).unwrap();
        let total_vertices: usize = meshes.iter().map(|(_, colors)| colors.len()).sum();
        let data_end = payload_start + total_vertices * VERTEX_BYTES;
        let file_end = align_up(data_end, STREAM_ALIGNMENT).unwrap();
        let mut bytes = vec![0_u8; file_end];
        bytes[0..4].copy_from_slice(&VERSION.to_le_bytes());
        bytes[4..8].copy_from_slice(&CHECKSUM.to_le_bytes());
        bytes[8..12].copy_from_slice(&VERTEX_COLOR_FLAG.to_le_bytes());
        bytes[12..16].copy_from_slice(&(VERTEX_BYTES as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&(total_vertices as u32).to_le_bytes());
        bytes[20..24].copy_from_slice(&(meshes.len() as i32).to_le_bytes());
        let mut data_cursor = payload_start;
        for (ordinal, (lod, colors)) in meshes.iter().enumerate() {
            let header = HEADER_BYTES + ordinal * MESH_HEADER_BYTES;
            bytes[header..header + 4].copy_from_slice(&lod.to_le_bytes());
            bytes[header + 4..header + 8].copy_from_slice(&(colors.len() as u32).to_le_bytes());
            bytes[header + 8..header + 12].copy_from_slice(&(data_cursor as u32).to_le_bytes());
            for color in *colors {
                bytes[data_cursor..data_cursor + 4].copy_from_slice(color);
                data_cursor += 4;
            }
        }
        bytes
    }

    fn parse_fixture(bytes: &[u8]) -> Result<Vhv, ParseError> {
        parse(
            bytes,
            Profile::source_pc_v2_color_bgra8888(CHECKSUM),
            limits(),
        )
    }

    fn assert_code(bytes: &[u8], code: ErrorCode) {
        assert_eq!(parse_fixture(bytes).unwrap_err().code, code);
    }

    #[test]
    fn parses_golden_multilod_stream_and_typed_colors() {
        let first = [[0, 1, 2, 255], [3, 4, 5, 255]];
        let second = [[255, 128, 64, 255]];
        let bytes = fixture(&[(0, &first), (3, &second)]);
        let vhv = parse_fixture(&bytes).unwrap();
        assert_eq!(vhv.profile.identity(), "source-pc-v2-color-bgra8888");
        assert_eq!(vhv.header.range, 0..40);
        assert_eq!(vhv.header.total_vertices, 3);
        assert_eq!(vhv.header.mesh_count, 2);
        assert_eq!(vhv.header_padding_range, 96..512);
        assert_eq!(vhv.meshes[0].header_range, 40..68);
        assert_eq!(vhv.meshes[0].data_range, 512..520);
        assert_eq!(vhv.meshes[1].data_range, 520..524);
        assert_eq!(vhv.trailing_padding_range, 524..1024);
        assert_eq!(vhv.disposition, Disposition::Populated);
        let colors: Vec<_> = vhv.colors(0).unwrap().collect();
        assert_eq!(colors[0].encoded_bgra(), [0, 1, 2, 255]);
        assert_eq!(colors[0].range, 512..516);
        assert_eq!(colors[1].local_vertex, 1);
        assert_eq!(
            vhv.colors(1).unwrap().next().unwrap().normalized_bgra(),
            [1.0, 128.0 / 255.0, 64.0 / 255.0, 1.0]
        );
        assert_eq!(vhv.mesh_bytes(0), Some(&bytes[512..520]));
    }

    #[test]
    fn accepts_canonical_empty_document() {
        let bytes = fixture(&[]);
        let vhv = parse_fixture(&bytes).unwrap();
        assert_eq!(bytes.len(), 512);
        assert_eq!(vhv.disposition, Disposition::Empty);
        assert!(vhv.meshes.is_empty());
        assert_eq!(vhv.header_padding_range, 40..512);
        assert_eq!(vhv.trailing_padding_range, 512..512);
    }

    #[test]
    fn repeated_parse_is_equal_and_does_not_mutate_input() {
        let colors = [[7, 8, 9, 255]];
        let bytes = fixture(&[(0, &colors)]);
        let before = bytes.clone();
        let first = parse_fixture(&bytes).unwrap();
        let second = parse_fixture(&bytes).unwrap();
        assert_eq!(first, second);
        assert_eq!(bytes, before);
        assert_eq!(first.source_bytes(), bytes);
        assert_eq!(first.source_identity.byte_length, bytes.len() as u64);
        assert_eq!(first.source_identity.sha256, Sha256::digest(&bytes)[..]);
    }

    #[test]
    fn classifies_header_and_profile_mutations() {
        let colors = [[1, 2, 3, 255]];
        let original = fixture(&[(0, &colors)]);
        assert_code(&original[..39], ErrorCode::TruncatedHeader);

        let mut bytes = original.clone();
        bytes[0..4].copy_from_slice(&3_i32.to_le_bytes());
        assert_code(&bytes, ErrorCode::UnsupportedVersion);
        let mut bytes = original.clone();
        bytes[4] ^= 1;
        assert_code(&bytes, ErrorCode::ChecksumMismatch);
        let mut bytes = original.clone();
        bytes[8..12].copy_from_slice(&0_u32.to_le_bytes());
        assert_code(&bytes, ErrorCode::UnsupportedVertexFlags);
        let mut bytes = original.clone();
        bytes[12..16].copy_from_slice(&8_u32.to_le_bytes());
        assert_code(&bytes, ErrorCode::UnsupportedVertexSize);
        let mut bytes = original.clone();
        bytes[20..24].copy_from_slice(&(-1_i32).to_le_bytes());
        assert_code(&bytes, ErrorCode::NegativeMeshCount);
        let mut bytes = original.clone();
        bytes[24] = 1;
        assert_code(&bytes, ErrorCode::NonzeroHeaderReserved);
    }

    #[test]
    fn classifies_mesh_count_range_and_total_mutations() {
        let first = [[1, 2, 3, 255], [4, 5, 6, 255]];
        let second = [[7, 8, 9, 255], [10, 11, 12, 255]];
        let original = fixture(&[(0, &first), (1, &second)]);

        let mut bytes = original.clone();
        bytes[40 + 12] = 1;
        assert_code(&bytes, ErrorCode::NonzeroMeshReserved);
        let mut bytes = original.clone();
        bytes[40..44].copy_from_slice(&8_u32.to_le_bytes());
        assert_code(&bytes, ErrorCode::LodLimit);
        let mut bytes = original.clone();
        bytes[40 + 4..40 + 8].copy_from_slice(&33_u32.to_le_bytes());
        assert_code(&bytes, ErrorCode::MeshVertexLimit);
        let mut bytes = original.clone();
        bytes[16..20].copy_from_slice(&5_u32.to_le_bytes());
        assert_code(&bytes, ErrorCode::VertexTotalMismatch);
        let mut bytes = original.clone();
        bytes[40 + 8..40 + 12].copy_from_slice(&516_u32.to_le_bytes());
        assert_code(&bytes, ErrorCode::MeshDataGap);
        let mut bytes = original.clone();
        bytes[68 + 8..68 + 12].copy_from_slice(&512_u32.to_le_bytes());
        assert_code(&bytes, ErrorCode::DuplicateMeshRange);
        let mut bytes = original.clone();
        bytes[68 + 8..68 + 12].copy_from_slice(&516_u32.to_le_bytes());
        assert_code(&bytes, ErrorCode::OverlappingMeshRange);
        let mut bytes = original.clone();
        bytes[40 + 4..40 + 8].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut broad = limits();
        broad.max_vertices_per_mesh = u32::MAX as usize;
        broad.max_total_vertices = u32::MAX as usize;
        assert_eq!(
            parse(
                &bytes,
                Profile::source_pc_v2_color_bgra8888(CHECKSUM),
                broad
            )
            .unwrap_err()
            .code,
            ErrorCode::TruncatedVertexRange
        );
    }

    #[test]
    fn classifies_padding_alpha_extent_and_trailing_mutations() {
        let colors = [[1, 2, 3, 255]];
        let original = fixture(&[(0, &colors)]);
        let mut bytes = original.clone();
        bytes[68] = 1;
        assert_code(&bytes, ErrorCode::NonzeroHeaderPadding);
        let mut bytes = original.clone();
        bytes[515] = 254;
        assert_code(&bytes, ErrorCode::NonOpaqueAlpha);
        let mut bytes = original.clone();
        bytes[516] = 1;
        assert_code(&bytes, ErrorCode::NonzeroTrailingPadding);
        let mut bytes = original.clone();
        bytes.pop();
        assert_code(&bytes, ErrorCode::TruncatedFileExtent);
        let mut bytes = original.clone();
        bytes.extend_from_slice(&[0; STREAM_ALIGNMENT]);
        assert_code(&bytes, ErrorCode::TrailingBytes);
    }

    #[test]
    fn enforces_every_caller_limit_at_boundaries() {
        let colors = [[1, 2, 3, 255], [4, 5, 6, 255]];
        let bytes = fixture(&[(2, &colors)]);
        let profile = Profile::source_pc_v2_color_bgra8888(CHECKSUM);

        let mut exact = limits();
        exact.max_input_bytes = bytes.len();
        exact.max_meshes = 1;
        exact.max_total_vertices = 2;
        exact.max_vertices_per_mesh = 2;
        exact.max_lod = 2;
        exact.max_retained_bytes = bytes.len() + size_of::<Mesh>();
        parse(&bytes, profile, exact).unwrap();

        let cases = [
            (
                Limits {
                    max_input_bytes: bytes.len() - 1,
                    ..exact
                },
                ErrorCode::InputLimit,
            ),
            (
                Limits {
                    max_meshes: 0,
                    ..exact
                },
                ErrorCode::MeshLimit,
            ),
            (
                Limits {
                    max_total_vertices: 1,
                    ..exact
                },
                ErrorCode::TotalVertexLimit,
            ),
            (
                Limits {
                    max_vertices_per_mesh: 1,
                    ..exact
                },
                ErrorCode::MeshVertexLimit,
            ),
            (
                Limits {
                    max_lod: 1,
                    ..exact
                },
                ErrorCode::LodLimit,
            ),
            (
                Limits {
                    max_retained_bytes: exact.max_retained_bytes - 1,
                    ..exact
                },
                ErrorCode::RetainedBytesLimit,
            ),
        ];
        for (limits, code) in cases {
            assert_eq!(parse(&bytes, profile, limits).unwrap_err().code, code);
        }

        if usize::BITS > 32 {
            let invalid = Limits {
                max_lod: u32::MAX as usize + 1,
                ..exact
            };
            assert_eq!(
                parse(&bytes, profile, invalid).unwrap_err().code,
                ErrorCode::InvalidLimits
            );
        }
    }

    #[test]
    fn truncated_mesh_table_and_contradictory_empty_counts_fail() {
        let empty = fixture(&[]);
        let mut table = empty[..60].to_vec();
        table[20..24].copy_from_slice(&1_i32.to_le_bytes());
        assert_code(&table, ErrorCode::TruncatedMeshTable);

        let mut contradictory = empty;
        contradictory[16..20].copy_from_slice(&1_u32.to_le_bytes());
        assert_code(&contradictory, ErrorCode::VertexTotalMismatch);
    }
}
