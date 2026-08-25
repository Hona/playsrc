use std::{fmt, ops::Range};

const POINT_SAMPLE_FLAG: u32 = 0x0000_0001;
const TRILINEAR_FLAG: u32 = 0x0000_0002;
const CLAMP_S_FLAG: u32 = 0x0000_0004;
const CLAMP_T_FLAG: u32 = 0x0000_0008;
const ANISOTROPIC_FLAG: u32 = 0x0000_0010;
const SRGB_FLAG: u32 = 0x0000_0040;
const NORMAL_FLAG: u32 = 0x0000_0080;
const NO_MIP_FLAG: u32 = 0x0000_0100;
const NO_LOD_FLAG: u32 = 0x0000_0200;
const ALL_MIPS_FLAG: u32 = 0x0000_0400;
const ONE_BIT_ALPHA_FLAG: u32 = 0x0000_1000;
const EIGHT_BIT_ALPHA_FLAG: u32 = 0x0000_2000;
const ENVMAP_FLAG: u32 = 0x0000_4000;
const CLAMP_U_FLAG: u32 = 0x0200_0000;
const SS_BUMP_FLAG: u32 = 0x0800_0000;
const BORDER_FLAG: u32 = 0x2000_0000;
const VERSION_7_3_FLAG_MASK: u32 = !0xd178_0400;
const INLINE_RESOURCE_FLAG: u8 = 0x02;
const LOW_IMAGE_TAG: [u8; 3] = [0x01, 0, 0];
const HIGH_IMAGE_TAG: [u8; 3] = [0x30, 0, 0];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dialect {
    Source2013Pc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_encoded_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_resource_bytes: usize,
    pub max_subresources: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_encoded_bytes: 256 * 1024 * 1024,
            max_decoded_bytes: 256 * 1024 * 1024,
            max_resource_bytes: 64 * 1024 * 1024,
            max_subresources: 65_536,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageFormat {
    Rgba8888,
    Abgr8888,
    Rgb888,
    Bgr888,
    Rgb565,
    I8,
    Ia88,
    P8,
    A8,
    Rgb888BlueScreen,
    Bgr888BlueScreen,
    Argb8888,
    Bgra8888,
    Dxt1,
    Dxt3,
    Dxt5,
    Bgrx8888,
    Bgr565,
    Bgrx5551,
    Bgra4444,
    Dxt1OneBitAlpha,
    Bgra5551,
    Uv88,
    Uvwq8888,
    Rgba16F,
    Rgba16,
    Uvlx8888,
    R32F,
    Rgb32F,
    Rgba32F,
    Unsupported(i32),
    Unknown(i32),
}

impl ImageFormat {
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Rgba8888,
            1 => Self::Abgr8888,
            2 => Self::Rgb888,
            3 => Self::Bgr888,
            4 => Self::Rgb565,
            5 => Self::I8,
            6 => Self::Ia88,
            7 => Self::P8,
            8 => Self::A8,
            9 => Self::Rgb888BlueScreen,
            10 => Self::Bgr888BlueScreen,
            11 => Self::Argb8888,
            12 => Self::Bgra8888,
            13 => Self::Dxt1,
            14 => Self::Dxt3,
            15 => Self::Dxt5,
            16 => Self::Bgrx8888,
            17 => Self::Bgr565,
            18 => Self::Bgrx5551,
            19 => Self::Bgra4444,
            20 => Self::Dxt1OneBitAlpha,
            21 => Self::Bgra5551,
            22 => Self::Uv88,
            23 => Self::Uvwq8888,
            24 => Self::Rgba16F,
            25 => Self::Rgba16,
            26 => Self::Uvlx8888,
            27 => Self::R32F,
            28 => Self::Rgb32F,
            29 => Self::Rgba32F,
            30..=38 => Self::Unsupported(code),
            _ => Self::Unknown(code),
        }
    }

    pub fn code(self) -> i32 {
        match self {
            Self::Rgba8888 => 0,
            Self::Abgr8888 => 1,
            Self::Rgb888 => 2,
            Self::Bgr888 => 3,
            Self::Rgb565 => 4,
            Self::I8 => 5,
            Self::Ia88 => 6,
            Self::P8 => 7,
            Self::A8 => 8,
            Self::Rgb888BlueScreen => 9,
            Self::Bgr888BlueScreen => 10,
            Self::Argb8888 => 11,
            Self::Bgra8888 => 12,
            Self::Dxt1 => 13,
            Self::Dxt3 => 14,
            Self::Dxt5 => 15,
            Self::Bgrx8888 => 16,
            Self::Bgr565 => 17,
            Self::Bgrx5551 => 18,
            Self::Bgra4444 => 19,
            Self::Dxt1OneBitAlpha => 20,
            Self::Bgra5551 => 21,
            Self::Uv88 => 22,
            Self::Uvwq8888 => 23,
            Self::Rgba16F => 24,
            Self::Rgba16 => 25,
            Self::Uvlx8888 => 26,
            Self::R32F => 27,
            Self::Rgb32F => 28,
            Self::Rgba32F => 29,
            Self::Unsupported(code) | Self::Unknown(code) => code,
        }
    }

    fn storage(self) -> Option<Storage> {
        match self {
            Self::Rgba8888
            | Self::Abgr8888
            | Self::Argb8888
            | Self::Bgra8888
            | Self::Bgrx8888
            | Self::Uvwq8888
            | Self::Uvlx8888
            | Self::R32F => Some(Storage::Pixel(4)),
            Self::Rgb888 | Self::Bgr888 | Self::Rgb888BlueScreen | Self::Bgr888BlueScreen => {
                Some(Storage::Pixel(3))
            }
            Self::Rgb565
            | Self::Ia88
            | Self::Bgr565
            | Self::Bgrx5551
            | Self::Bgra4444
            | Self::Bgra5551
            | Self::Uv88 => Some(Storage::Pixel(2)),
            Self::I8 | Self::P8 | Self::A8 => Some(Storage::Pixel(1)),
            Self::Dxt1 | Self::Dxt1OneBitAlpha => Some(Storage::Block(8)),
            Self::Dxt3 | Self::Dxt5 => Some(Storage::Block(16)),
            Self::Rgba16F | Self::Rgba16 => Some(Storage::Pixel(8)),
            Self::Rgb32F => Some(Storage::Pixel(12)),
            Self::Rgba32F => Some(Storage::Pixel(16)),
            Self::Unsupported(_) | Self::Unknown(_) => None,
        }
    }

    fn decodable(self) -> bool {
        matches!(
            self,
            Self::Rgba8888
                | Self::Bgr888
                | Self::Bgra8888
                | Self::Dxt1
                | Self::Dxt1OneBitAlpha
                | Self::Dxt5
                | Self::Rgba16F
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Storage {
    Pixel(usize),
    Block(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Face {
    Right,
    Left,
    Back,
    Front,
    Up,
    Down,
    Sphere,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubresourceIdentity {
    LowResolution,
    HighResolution {
        mip: u8,
        frame: u16,
        face: Face,
        slice: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Subresource {
    pub identity: SubresourceIdentity,
    pub width: u32,
    pub height: u32,
    pub format: ImageFormat,
    pub encoded_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceData {
    Inline(u32),
    External { range: Range<usize>, bytes: Vec<u8> },
    Image { offset: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resource {
    pub tag: [u8; 3],
    pub flags: u8,
    pub data: ResourceData,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    pub dialect: Dialect,
    pub version: (u32, u32),
    pub header_size: usize,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub frame_count: u16,
    pub start_frame: u16,
    pub faces: Vec<Face>,
    pub mip_count: u8,
    pub raw_flags: u32,
    pub effective_flags: u32,
    pub alpha_flags: AlphaFlags,
    pub reflectivity_bits: [u32; 3],
    pub bump_scale_bits: u32,
    pub high_format: ImageFormat,
    pub low_format: ImageFormat,
    pub low_width: u32,
    pub low_height: u32,
    pub resources: Vec<Resource>,
    pub subresources: Vec<Subresource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelLayout {
    Rgb,
    Rgba,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarEncoding {
    U8,
    F16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorEncoding {
    Linear,
    Srgb,
    NotColor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaEncoding {
    None,
    Opaque,
    A1,
    A8,
    A16F,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowOrder {
    TopToBottom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlphaFlags {
    pub one_bit: bool,
    pub eight_bit: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WrapMode {
    Repeat,
    Clamp,
    Border,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MinFilter {
    Nearest,
    Linear,
    LinearMipmapNearest,
    LinearMipmapLinear,
    Anisotropic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MagFilter {
    Nearest,
    Linear,
    Anisotropic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SamplingEnvironment {
    pub shader_model: u16,
    pub force_anisotropy: u8,
    pub maximum_anisotropy: u8,
    pub force_trilinear: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SamplingState {
    pub wrap_s: WrapMode,
    pub wrap_t: WrapMode,
    pub wrap_u: WrapMode,
    pub min_filter: MinFilter,
    pub mag_filter: MagFilter,
    pub mipmapped: bool,
    pub no_lod: bool,
    pub all_mips: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plane {
    pub identity: SubresourceIdentity,
    pub width: u32,
    pub height: u32,
    pub row_stride: usize,
    pub row_order: RowOrder,
    pub channel_layout: ChannelLayout,
    pub scalar_encoding: ScalarEncoding,
    pub color_encoding: ColorEncoding,
    pub alpha_encoding: AlphaEncoding,
    pub samples: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Classification {
    Malformed,
    Unsupported,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorCode {
    InvalidSignature,
    UnsupportedVersion,
    TruncatedHeader,
    InvalidHeaderSize,
    InvalidDimensions,
    InvalidCubemap,
    InvalidFrameCount,
    InvalidMipCount,
    InvalidFormat,
    UnknownFormat,
    UnsupportedFormat,
    InvalidThumbnail,
    ResourceLimit,
    ResourceOrder,
    DuplicateResource,
    InvalidResourceFlags,
    MissingImageResource,
    InvalidResourceRange,
    OverlappingResource,
    TruncatedImage,
    SelectorOutOfRange,
    ArithmeticOverflow,
    AllocationLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    pub classification: Classification,
    pub code: ErrorCode,
    pub offset: Option<usize>,
    pub selector: Option<SubresourceIdentity>,
    pub value: Option<i64>,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "{:?}: {:?}", self.classification, self.code)
    }
}

impl std::error::Error for Error {}

pub fn inspect(bytes: &[u8], dialect: Dialect, limits: Limits) -> Result<Metadata, Error> {
    if bytes.len() > limits.max_encoded_bytes {
        return Err(malformed(ErrorCode::AllocationLimit, None));
    }
    if bytes.get(0..4) != Some(b"VTF\0") {
        return Err(malformed(ErrorCode::InvalidSignature, Some(0)));
    }
    if bytes.len() < 64 {
        return Err(malformed(ErrorCode::TruncatedHeader, Some(bytes.len())));
    }
    let major = u32_at(bytes, 4);
    let minor = u32_at(bytes, 8);
    if major != 7 || minor > 5 {
        return Err(unsupported(
            ErrorCode::UnsupportedVersion,
            Some(4),
            major as i64,
        ));
    }
    let resource_count = if minor >= 3 {
        if bytes.len() < 80 {
            return Err(malformed(ErrorCode::TruncatedHeader, Some(bytes.len())));
        }
        u32_at(bytes, 68) as usize
    } else {
        0
    };
    let expected_header = match minor {
        0 | 1 => 64,
        2 => 80,
        _ => 80_usize
            .checked_add(
                resource_count
                    .checked_mul(8)
                    .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, Some(68)))?,
            )
            .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, Some(68)))?,
    };
    let header_size = u32_at(bytes, 12) as usize;
    if header_size != expected_header || header_size > bytes.len() {
        return Err(malformed(ErrorCode::InvalidHeaderSize, Some(12)));
    }
    let width = u16_at(bytes, 16) as u32;
    let height = u16_at(bytes, 18) as u32;
    let raw_flags = u32_at(bytes, 20);
    let effective_flags = if minor <= 3 {
        raw_flags & VERSION_7_3_FLAG_MASK
    } else {
        raw_flags
    };
    let frame_count = u16_at(bytes, 24);
    let start_frame = u16_at(bytes, 26);
    let depth = if minor >= 2 {
        u16_at(bytes, 63) as u32
    } else {
        1
    };
    if width == 0 || height == 0 || depth == 0 {
        return Err(malformed(ErrorCode::InvalidDimensions, Some(16)));
    }
    if frame_count == 0 {
        return Err(malformed(ErrorCode::InvalidFrameCount, Some(24)));
    }
    let mip_count = bytes[56];
    let maximum_mips = 32 - width.max(height).max(depth).leading_zeros();
    if mip_count == 0 || u32::from(mip_count) > maximum_mips {
        return Err(malformed(ErrorCode::InvalidMipCount, Some(56)));
    }
    let faces = if effective_flags & ENVMAP_FLAG == 0 {
        vec![Face::Right]
    } else {
        if width != height || depth != 1 {
            return Err(malformed(ErrorCode::InvalidCubemap, Some(16)));
        }
        let mut faces = vec![
            Face::Right,
            Face::Left,
            Face::Back,
            Face::Front,
            Face::Up,
            Face::Down,
        ];
        if minor >= 1 {
            faces.push(Face::Sphere);
        }
        faces
    };
    let high_format = ImageFormat::from_code(i32_at(bytes, 52));
    let low_format = ImageFormat::from_code(i32_at(bytes, 57));
    let low_width = bytes[61] as u32;
    let low_height = bytes[62] as u32;
    if (low_width == 0) != (low_height == 0) {
        return Err(malformed(ErrorCode::InvalidThumbnail, Some(61)));
    }
    let low_length = if low_width == 0 {
        0
    } else {
        image_size(low_width, low_height, low_format)?
    };
    let high_length = all_high_bytes(
        width,
        height,
        depth,
        frame_count,
        faces.len(),
        mip_count,
        high_format,
    )?;
    let mut resources = Vec::new();
    let (low_offset, high_offset) = if minor < 3 {
        let low_offset = header_size;
        let high_offset = low_offset
            .checked_add(low_length)
            .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
        (low_offset, high_offset)
    } else {
        if resource_count > 32 {
            return Err(malformed(ErrorCode::ResourceLimit, Some(68)));
        }
        let mut previous = None;
        let mut low = None;
        let mut high = None;
        for index in 0..resource_count {
            let offset = 80 + index * 8;
            let tag: [u8; 3] = bytes[offset..offset + 3]
                .try_into()
                .expect("resource header");
            let flags = bytes[offset + 3];
            let key = u32::from_le_bytes([tag[0], tag[1], tag[2], 0]);
            if previous.is_some_and(|value| value >= key) {
                return Err(malformed(ErrorCode::ResourceOrder, Some(offset)));
            }
            previous = Some(key);
            if flags & !INLINE_RESOURCE_FLAG != 0 {
                return Err(malformed(ErrorCode::InvalidResourceFlags, Some(offset + 3)));
            }
            let value = u32_at(bytes, offset + 4);
            let inline = flags & INLINE_RESOURCE_FLAG != 0;
            let data = if inline {
                ResourceData::Inline(value)
            } else {
                let data_offset = value as usize;
                if data_offset < header_size || data_offset > bytes.len() {
                    return Err(malformed(ErrorCode::InvalidResourceRange, Some(offset + 4)));
                }
                if tag == LOW_IMAGE_TAG {
                    if low.replace(data_offset).is_some() {
                        return Err(malformed(ErrorCode::DuplicateResource, Some(offset)));
                    }
                    ResourceData::Image {
                        offset: data_offset,
                    }
                } else if tag == HIGH_IMAGE_TAG {
                    if high.replace(data_offset).is_some() {
                        return Err(malformed(ErrorCode::DuplicateResource, Some(offset)));
                    }
                    ResourceData::Image {
                        offset: data_offset,
                    }
                } else {
                    let length = read_external_length(bytes, data_offset, limits)?;
                    let start = data_offset + 4;
                    let end = start.checked_add(length).ok_or_else(|| {
                        malformed(ErrorCode::ArithmeticOverflow, Some(data_offset))
                    })?;
                    ResourceData::External {
                        range: start..end,
                        bytes: bytes[start..end].to_vec(),
                    }
                }
            };
            if inline && (tag == LOW_IMAGE_TAG || tag == HIGH_IMAGE_TAG) {
                return Err(malformed(ErrorCode::InvalidResourceFlags, Some(offset + 3)));
            }
            resources.push(Resource { tag, flags, data });
        }
        let high = high.ok_or_else(|| malformed(ErrorCode::MissingImageResource, Some(80)))?;
        let low = if low_length == 0 {
            low.unwrap_or(header_size)
        } else {
            low.ok_or_else(|| malformed(ErrorCode::MissingImageResource, Some(80)))?
        };
        (low, high)
    };
    validate_range(bytes, high_offset, high_length, ErrorCode::TruncatedImage)?;
    if low_length > 0 {
        validate_range(bytes, low_offset, low_length, ErrorCode::TruncatedImage)?;
        if ranges_overlap(
            &(low_offset..low_offset + low_length),
            &(high_offset..high_offset + high_length),
        ) {
            return Err(malformed(ErrorCode::OverlappingResource, None));
        }
    }
    let mut subresources = Vec::new();
    if low_length > 0 {
        subresources.push(Subresource {
            identity: SubresourceIdentity::LowResolution,
            width: low_width,
            height: low_height,
            format: low_format,
            encoded_range: low_offset..low_offset + low_length,
        });
    }
    let mut cursor = high_offset;
    for mip in (0..mip_count).rev() {
        let mip_width = mip_dimension(width, mip);
        let mip_height = mip_dimension(height, mip);
        let slices = mip_dimension(depth, mip) as u16;
        let length = image_size(mip_width, mip_height, high_format)?;
        for frame in 0..frame_count {
            for &face in &faces {
                for slice in 0..slices {
                    let end = cursor
                        .checked_add(length)
                        .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
                    subresources.push(Subresource {
                        identity: SubresourceIdentity::HighResolution {
                            mip,
                            frame,
                            face,
                            slice,
                        },
                        width: mip_width,
                        height: mip_height,
                        format: high_format,
                        encoded_range: cursor..end,
                    });
                    cursor = end;
                    if subresources.len() > limits.max_subresources {
                        return Err(malformed(ErrorCode::ResourceLimit, None));
                    }
                }
            }
        }
    }
    let mut occupied = Vec::with_capacity(resources.len() + 2);
    occupied.push(high_offset..high_offset + high_length);
    if low_length > 0 {
        occupied.push(low_offset..low_offset + low_length);
    }
    for resource in &resources {
        if let ResourceData::External { range, .. } = &resource.data {
            occupied.push(range.start.saturating_sub(4)..range.end);
        }
    }
    for left in 0..occupied.len() {
        for right in left + 1..occupied.len() {
            if ranges_overlap(&occupied[left], &occupied[right]) {
                return Err(malformed(ErrorCode::OverlappingResource, None));
            }
        }
    }
    Ok(Metadata {
        dialect,
        version: (major, minor),
        header_size,
        width,
        height,
        depth,
        frame_count,
        start_frame,
        faces,
        mip_count,
        raw_flags,
        effective_flags,
        alpha_flags: AlphaFlags {
            one_bit: effective_flags & ONE_BIT_ALPHA_FLAG != 0,
            eight_bit: effective_flags & EIGHT_BIT_ALPHA_FLAG != 0,
        },
        reflectivity_bits: [u32_at(bytes, 32), u32_at(bytes, 36), u32_at(bytes, 40)],
        bump_scale_bits: u32_at(bytes, 48),
        high_format,
        low_format,
        low_width,
        low_height,
        resources,
        subresources,
    })
}

#[derive(Debug)]
pub struct Decoder<'source> {
    bytes: &'source [u8],
    metadata: Metadata,
    limits: Limits,
}

impl<'source> Decoder<'source> {
    pub fn new(bytes: &'source [u8], dialect: Dialect, limits: Limits) -> Result<Self, Error> {
        Ok(Self {
            bytes,
            metadata: inspect(bytes, dialect, limits)?,
            limits,
        })
    }

    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub fn decode(&self, selector: SubresourceIdentity) -> Result<Plane, Error> {
        decode_inspected(self.bytes, &self.metadata, selector, self.limits)
    }

    pub fn decode_high_resolution(&self) -> Result<Vec<Plane>, Error> {
        let count = self
            .metadata
            .subresources
            .iter()
            .filter(|subresource| {
                matches!(
                    subresource.identity,
                    SubresourceIdentity::HighResolution { .. }
                )
            })
            .count();
        let mut planes = Vec::with_capacity(count);
        let mut decoded_bytes = 0_usize;
        for subresource in &self.metadata.subresources {
            if !matches!(
                subresource.identity,
                SubresourceIdentity::HighResolution { .. }
            ) {
                continue;
            }
            if let Some(length) = decoded_subresource_bytes(subresource)? {
                decoded_bytes = decoded_bytes
                    .checked_add(length)
                    .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
                if decoded_bytes > self.limits.max_decoded_bytes {
                    return Err(malformed(ErrorCode::AllocationLimit, None));
                }
            }
            planes.push(self.decode(subresource.identity)?);
        }
        Ok(planes)
    }
}

pub fn decode(
    bytes: &[u8],
    dialect: Dialect,
    selector: SubresourceIdentity,
    limits: Limits,
) -> Result<Plane, Error> {
    Decoder::new(bytes, dialect, limits)?.decode(selector)
}

fn decoded_subresource_bytes(subresource: &Subresource) -> Result<Option<usize>, Error> {
    let components = match subresource.format {
        ImageFormat::Bgr888 => 3,
        ImageFormat::Rgba8888
        | ImageFormat::Bgra8888
        | ImageFormat::Dxt1
        | ImageFormat::Dxt1OneBitAlpha
        | ImageFormat::Dxt5 => 4,
        ImageFormat::Rgba16F => 8,
        _ => return Ok(None),
    };
    usize::try_from(subresource.width)
        .ok()
        .and_then(|width| {
            usize::try_from(subresource.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
                .and_then(|pixels| pixels.checked_mul(components))
        })
        .map(Some)
        .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))
}

fn decode_inspected(
    bytes: &[u8],
    metadata: &Metadata,
    selector: SubresourceIdentity,
    limits: Limits,
) -> Result<Plane, Error> {
    let subresource = metadata
        .subresources
        .iter()
        .find(|resource| resource.identity == selector)
        .ok_or(Error {
            classification: Classification::Malformed,
            code: ErrorCode::SelectorOutOfRange,
            offset: None,
            selector: Some(selector),
            value: None,
        })?;
    let format = subresource.format;
    if !format.decodable() {
        let (classification, code) = match format {
            ImageFormat::Unknown(_) => (Classification::Unknown, ErrorCode::UnknownFormat),
            _ => (Classification::Unsupported, ErrorCode::UnsupportedFormat),
        };
        return Err(Error {
            classification,
            code,
            offset: Some(subresource.encoded_range.start),
            selector: Some(selector),
            value: Some(format.code() as i64),
        });
    }
    let encoded = &bytes[subresource.encoded_range.clone()];
    let pixels = usize::try_from(subresource.width)
        .ok()
        .and_then(|width| {
            usize::try_from(subresource.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
    let (channel_layout, scalar_encoding, row_stride, alpha_encoding, samples) = match format {
        ImageFormat::Rgba8888 => {
            let length = pixels
                .checked_mul(4)
                .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
            if length > limits.max_decoded_bytes {
                return Err(malformed(ErrorCode::AllocationLimit, None));
            }
            (
                ChannelLayout::Rgba,
                ScalarEncoding::U8,
                subresource.width as usize * 4,
                AlphaEncoding::A8,
                encoded.to_vec(),
            )
        }
        ImageFormat::Bgr888 => {
            let length = pixels
                .checked_mul(3)
                .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
            if length > limits.max_decoded_bytes {
                return Err(malformed(ErrorCode::AllocationLimit, None));
            }
            let mut samples = vec![0; length];
            for (source, output) in encoded.chunks_exact(3).zip(samples.chunks_exact_mut(3)) {
                output.copy_from_slice(&[source[2], source[1], source[0]]);
            }
            (
                ChannelLayout::Rgb,
                ScalarEncoding::U8,
                subresource.width as usize * 3,
                AlphaEncoding::None,
                samples,
            )
        }
        ImageFormat::Bgra8888 => {
            let length = pixels
                .checked_mul(4)
                .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
            if length > limits.max_decoded_bytes {
                return Err(malformed(ErrorCode::AllocationLimit, None));
            }
            let mut samples = vec![0; length];
            for (source, output) in encoded.chunks_exact(4).zip(samples.chunks_exact_mut(4)) {
                output.copy_from_slice(&[source[2], source[1], source[0], source[3]]);
            }
            (
                ChannelLayout::Rgba,
                ScalarEncoding::U8,
                subresource.width as usize * 4,
                AlphaEncoding::A8,
                samples,
            )
        }
        ImageFormat::Dxt1 | ImageFormat::Dxt1OneBitAlpha | ImageFormat::Dxt5 => {
            let length = pixels
                .checked_mul(4)
                .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
            if length > limits.max_decoded_bytes {
                return Err(malformed(ErrorCode::AllocationLimit, None));
            }
            let mut samples = vec![0; length];
            decode_blocks(
                format,
                encoded,
                subresource.width,
                subresource.height,
                &mut samples,
            );
            (
                ChannelLayout::Rgba,
                ScalarEncoding::U8,
                subresource.width as usize * 4,
                match format {
                    ImageFormat::Dxt1 => AlphaEncoding::Opaque,
                    ImageFormat::Dxt1OneBitAlpha => AlphaEncoding::A1,
                    ImageFormat::Dxt5 => AlphaEncoding::A8,
                    _ => unreachable!(),
                },
                samples,
            )
        }
        ImageFormat::Rgba16F => {
            if encoded.len() > limits.max_decoded_bytes {
                return Err(malformed(ErrorCode::AllocationLimit, None));
            }
            (
                ChannelLayout::Rgba,
                ScalarEncoding::F16,
                subresource.width as usize * 8,
                AlphaEncoding::A16F,
                encoded.to_vec(),
            )
        }
        _ => unreachable!("decodable format set"),
    };
    Ok(Plane {
        identity: selector,
        width: subresource.width,
        height: subresource.height,
        row_stride,
        row_order: RowOrder::TopToBottom,
        channel_layout,
        scalar_encoding,
        color_encoding: if metadata.effective_flags & (NORMAL_FLAG | SS_BUMP_FLAG) != 0 {
            ColorEncoding::NotColor
        } else if metadata.effective_flags & SRGB_FLAG != 0 {
            ColorEncoding::Srgb
        } else {
            ColorEncoding::Linear
        },
        alpha_encoding,
        samples,
    })
}

pub fn sampling_state(metadata: &Metadata, environment: SamplingEnvironment) -> SamplingState {
    let flags = metadata.effective_flags;
    let wrap = |flag| {
        if flags & BORDER_FLAG != 0 {
            WrapMode::Border
        } else if flags & flag != 0 {
            WrapMode::Clamp
        } else {
            WrapMode::Repeat
        }
    };
    let point = flags & POINT_SAMPLE_FLAG != 0;
    let mipmapped = !point && flags & NO_MIP_FLAG == 0;
    let (min_filter, mag_filter) = if point {
        (MinFilter::Nearest, MagFilter::Nearest)
    } else if !mipmapped {
        (MinFilter::Linear, MagFilter::Linear)
    } else {
        let supports_anisotropy =
            environment.shader_model >= 80 && environment.maximum_anisotropy > 1;
        let forced_anisotropy = supports_anisotropy && environment.force_anisotropy > 1;
        let texture_anisotropy = supports_anisotropy && flags & ANISOTROPIC_FLAG != 0;
        if forced_anisotropy || texture_anisotropy {
            (MinFilter::Anisotropic, MagFilter::Anisotropic)
        } else if environment.force_trilinear || flags & TRILINEAR_FLAG != 0 {
            (MinFilter::LinearMipmapLinear, MagFilter::Linear)
        } else {
            (MinFilter::LinearMipmapNearest, MagFilter::Linear)
        }
    };
    SamplingState {
        wrap_s: wrap(CLAMP_S_FLAG),
        wrap_t: wrap(CLAMP_T_FLAG),
        wrap_u: wrap(CLAMP_U_FLAG),
        min_filter,
        mag_filter,
        mipmapped,
        no_lod: flags & NO_LOD_FLAG != 0,
        all_mips: flags & ALL_MIPS_FLAG != 0,
    }
}

fn read_external_length(bytes: &[u8], offset: usize, limits: Limits) -> Result<usize, Error> {
    if offset.checked_add(4).is_none_or(|end| end > bytes.len()) {
        return Err(malformed(ErrorCode::InvalidResourceRange, Some(offset)));
    }
    let length = u32_at(bytes, offset) as usize;
    if length > limits.max_resource_bytes {
        return Err(malformed(ErrorCode::ResourceLimit, Some(offset)));
    }
    let end = offset
        .checked_add(4)
        .and_then(|start| start.checked_add(length))
        .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, Some(offset)))?;
    if end > bytes.len() {
        return Err(malformed(ErrorCode::InvalidResourceRange, Some(offset)));
    }
    Ok(length)
}

fn all_high_bytes(
    width: u32,
    height: u32,
    depth: u32,
    frames: u16,
    faces: usize,
    mips: u8,
    format: ImageFormat,
) -> Result<usize, Error> {
    let mut total = 0_usize;
    for mip in 0..mips {
        let image = image_size(
            mip_dimension(width, mip),
            mip_dimension(height, mip),
            format,
        )?;
        let slices = mip_dimension(depth, mip) as usize;
        let level = image
            .checked_mul(slices)
            .and_then(|value| value.checked_mul(frames as usize))
            .and_then(|value| value.checked_mul(faces))
            .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
        total = total
            .checked_add(level)
            .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None))?;
    }
    Ok(total)
}

fn image_size(width: u32, height: u32, format: ImageFormat) -> Result<usize, Error> {
    let storage = format.storage().ok_or_else(|| match format {
        ImageFormat::Unknown(code) => unknown(ErrorCode::UnknownFormat, code as i64),
        _ => unsupported(ErrorCode::UnsupportedFormat, None, format.code() as i64),
    })?;
    match storage {
        Storage::Pixel(bytes) => (width as usize)
            .checked_mul(height as usize)
            .and_then(|value| value.checked_mul(bytes))
            .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None)),
        Storage::Block(bytes) => (width.div_ceil(4) as usize)
            .checked_mul(height.div_ceil(4) as usize)
            .and_then(|value| value.checked_mul(bytes))
            .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, None)),
    }
}

fn mip_dimension(value: u32, mip: u8) -> u32 {
    value.checked_shr(mip as u32).unwrap_or(0).max(1)
}

fn validate_range(
    bytes: &[u8],
    offset: usize,
    length: usize,
    code: ErrorCode,
) -> Result<(), Error> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| malformed(ErrorCode::ArithmeticOverflow, Some(offset)))?;
    if end > bytes.len() {
        return Err(malformed(code, Some(offset)));
    }
    Ok(())
}

fn ranges_overlap(left: &Range<usize>, right: &Range<usize>) -> bool {
    !left.is_empty() && !right.is_empty() && left.start < right.end && right.start < left.end
}

fn expand_565(value: u16) -> [u8; 4] {
    let red = ((value >> 11) & 0x1f) as u8;
    let green = ((value >> 5) & 0x3f) as u8;
    let blue = (value & 0x1f) as u8;
    [
        (red << 3) | (red >> 2),
        (green << 2) | (green >> 4),
        (blue << 3) | (blue >> 2),
        255,
    ]
}

fn color_palette(block: &[u8], one_bit_alpha: bool) -> [[u8; 4]; 4] {
    let encoded_first = u16::from_le_bytes([block[0], block[1]]);
    let encoded_second = u16::from_le_bytes([block[2], block[3]]);
    let first = expand_565(encoded_first);
    let second = expand_565(encoded_second);
    let mut colors = [first, second, [0; 4], [0; 4]];
    if encoded_first > encoded_second || !one_bit_alpha {
        for channel in 0..3 {
            colors[2][channel] =
                ((2 * u16::from(first[channel]) + u16::from(second[channel])) / 3) as u8;
            colors[3][channel] =
                ((u16::from(first[channel]) + 2 * u16::from(second[channel])) / 3) as u8;
        }
        colors[2][3] = 255;
        colors[3][3] = 255;
    } else {
        for channel in 0..3 {
            colors[2][channel] =
                ((u16::from(first[channel]) + u16::from(second[channel])) / 2) as u8;
        }
        colors[2][3] = 255;
    }
    colors
}

fn alpha_palette(block: &[u8]) -> [u8; 8] {
    let first = block[0];
    let second = block[1];
    let mut values = [first, second, 0, 0, 0, 0, 0, 0];
    if first > second {
        for (index, value) in values.iter_mut().enumerate().skip(2) {
            *value = (((8 - index) as u16 * u16::from(first)
                + (index - 1) as u16 * u16::from(second))
                / 7) as u8;
        }
    } else {
        for (index, value) in values.iter_mut().enumerate().take(6).skip(2) {
            *value = (((6 - index) as u16 * u16::from(first)
                + (index - 1) as u16 * u16::from(second))
                / 5) as u8;
        }
        values[6] = 0;
        values[7] = 255;
    }
    values
}

fn decode_blocks(format: ImageFormat, encoded: &[u8], width: u32, height: u32, output: &mut [u8]) {
    let block_bytes = if format == ImageFormat::Dxt5 { 16 } else { 8 };
    let blocks_wide = width.div_ceil(4) as usize;
    for (block_index, block) in encoded.chunks_exact(block_bytes).enumerate() {
        let block_x = block_index % blocks_wide;
        let block_y = block_index / blocks_wide;
        let (color, color_indexes, alpha_indexes, alphas) = if format == ImageFormat::Dxt5 {
            (
                &block[8..16],
                u32::from_le_bytes(block[12..16].try_into().expect("BC3 color indexes")),
                u64::from_le_bytes([
                    block[2], block[3], block[4], block[5], block[6], block[7], 0, 0,
                ]),
                Some(alpha_palette(block)),
            )
        } else {
            (
                &block[0..8],
                u32::from_le_bytes(block[4..8].try_into().expect("BC1 color indexes")),
                0,
                None,
            )
        };
        let colors = color_palette(color, format == ImageFormat::Dxt1OneBitAlpha);
        for texel in 0..16 {
            let x = block_x * 4 + texel % 4;
            let y = block_y * 4 + texel / 4;
            if x >= width as usize || y >= height as usize {
                continue;
            }
            let color_index = ((color_indexes >> (texel * 2)) & 3) as usize;
            let mut sample = colors[color_index];
            if let Some(alphas) = alphas {
                sample[3] = alphas[((alpha_indexes >> (texel * 3)) & 7) as usize];
            }
            let start = (y * width as usize + x) * 4;
            output[start..start + 4].copy_from_slice(&sample);
        }
    }
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        bytes[offset..offset + 2]
            .try_into()
            .expect("validated VTF field"),
    )
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated VTF field"),
    )
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated VTF field"),
    )
}

fn malformed(code: ErrorCode, offset: Option<usize>) -> Error {
    Error {
        classification: Classification::Malformed,
        code,
        offset,
        selector: None,
        value: None,
    }
}

fn unsupported(code: ErrorCode, offset: Option<usize>, value: i64) -> Error {
    Error {
        classification: Classification::Unsupported,
        code,
        offset,
        selector: None,
        value: Some(value),
    }
}

fn unknown(code: ErrorCode, value: i64) -> Error {
    Error {
        classification: Classification::Unknown,
        code,
        offset: None,
        selector: None,
        value: Some(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Header {
        minor: u32,
        width: u16,
        height: u16,
        depth: u16,
        frames: u16,
        start_frame: u16,
        flags: u32,
        high_format: i32,
        mips: u8,
        low_format: i32,
        low_dimensions: (u8, u8),
    }

    fn header(input: Header, resource_count: usize) -> Vec<u8> {
        let size = match input.minor {
            0 | 1 => 64,
            2 => 80,
            _ => 80 + resource_count * 8,
        };
        let mut bytes = vec![0; size];
        bytes[..4].copy_from_slice(b"VTF\0");
        bytes[4..8].copy_from_slice(&7_u32.to_le_bytes());
        bytes[8..12].copy_from_slice(&input.minor.to_le_bytes());
        bytes[12..16].copy_from_slice(&(size as u32).to_le_bytes());
        bytes[16..18].copy_from_slice(&input.width.to_le_bytes());
        bytes[18..20].copy_from_slice(&input.height.to_le_bytes());
        bytes[20..24].copy_from_slice(&input.flags.to_le_bytes());
        bytes[24..26].copy_from_slice(&input.frames.to_le_bytes());
        bytes[26..28].copy_from_slice(&input.start_frame.to_le_bytes());
        bytes[32..36].copy_from_slice(&0x3f80_0000_u32.to_le_bytes());
        bytes[36..40].copy_from_slice(&0x8000_0000_u32.to_le_bytes());
        bytes[40..44].copy_from_slice(&0x7fc0_1234_u32.to_le_bytes());
        bytes[48..52].copy_from_slice(&0x4000_0000_u32.to_le_bytes());
        bytes[52..56].copy_from_slice(&input.high_format.to_le_bytes());
        bytes[56] = input.mips;
        bytes[57..61].copy_from_slice(&input.low_format.to_le_bytes());
        bytes[61] = input.low_dimensions.0;
        bytes[62] = input.low_dimensions.1;
        if input.minor >= 2 {
            bytes[63..65].copy_from_slice(&input.depth.to_le_bytes());
        }
        if input.minor >= 3 {
            bytes[68..72].copy_from_slice(&(resource_count as u32).to_le_bytes());
        }
        bytes
    }

    fn resource(bytes: &mut [u8], index: usize, tag: [u8; 3], flags: u8, value: u32) {
        let offset = 80 + index * 8;
        bytes[offset..offset + 3].copy_from_slice(&tag);
        bytes[offset + 3] = flags;
        bytes[offset + 4..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn ordinary(minor: u32, format: i32, width: u16, height: u16) -> Header {
        Header {
            minor,
            width,
            height,
            depth: 1,
            frames: 1,
            start_frame: 0,
            flags: 0,
            high_format: format,
            mips: 1,
            low_format: -1,
            low_dimensions: (0, 0),
        }
    }

    #[test]
    fn parses_sequential_v71_and_decodes_stored_bgr_rows() {
        let mut bytes = header(ordinary(1, 3, 2, 1), 0);
        bytes.extend_from_slice(&[30, 20, 10, 60, 50, 40]);
        let metadata = inspect(&bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        assert_eq!(metadata.header_size, 64);
        assert_eq!(
            metadata.reflectivity_bits,
            [0x3f80_0000, 0x8000_0000, 0x7fc0_1234]
        );
        assert_eq!(metadata.subresources.len(), 1);
        let identity = SubresourceIdentity::HighResolution {
            mip: 0,
            frame: 0,
            face: Face::Right,
            slice: 0,
        };
        let plane = decode(&bytes, Dialect::Source2013Pc, identity, Limits::default()).unwrap();
        assert_eq!(plane.samples, [10, 20, 30, 40, 50, 60]);
        assert_eq!(plane.row_stride, 6);
        assert_eq!(plane.channel_layout, ChannelLayout::Rgb);
        assert_eq!(plane.row_order, RowOrder::TopToBottom);
    }

    #[test]
    fn prepared_decoder_retains_every_authored_frame_and_mip_in_disk_order() {
        let mut input = ordinary(1, 3, 2, 2);
        input.frames = 2;
        input.mips = 2;
        let mut bytes = header(input, 0);
        bytes.extend_from_slice(&[3, 2, 1, 6, 5, 4]);
        bytes.extend((10_u8..22).chain(30_u8..42));

        let decoder = Decoder::new(&bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        assert_eq!(decoder.metadata().frame_count, 2);
        assert_eq!(decoder.metadata().mip_count, 2);

        let planes = decoder.decode_high_resolution().unwrap();
        assert_eq!(
            planes
                .iter()
                .map(|plane| plane.identity)
                .collect::<Vec<_>>(),
            [
                SubresourceIdentity::HighResolution {
                    mip: 1,
                    frame: 0,
                    face: Face::Right,
                    slice: 0,
                },
                SubresourceIdentity::HighResolution {
                    mip: 1,
                    frame: 1,
                    face: Face::Right,
                    slice: 0,
                },
                SubresourceIdentity::HighResolution {
                    mip: 0,
                    frame: 0,
                    face: Face::Right,
                    slice: 0,
                },
                SubresourceIdentity::HighResolution {
                    mip: 0,
                    frame: 1,
                    face: Face::Right,
                    slice: 0,
                },
            ],
        );
        assert_eq!(planes[0].samples, [1, 2, 3]);
        assert_eq!(planes[1].samples, [4, 5, 6]);
        assert_eq!(planes[2].samples[..3], [12, 11, 10]);
        assert_eq!(planes[3].samples[..3], [32, 31, 30]);

        let limited = Decoder::new(
            &bytes,
            Dialect::Source2013Pc,
            Limits {
                max_decoded_bytes: 25,
                ..Limits::default()
            },
        )
        .unwrap();
        assert_eq!(
            limited.decode_high_resolution().unwrap_err().code,
            ErrorCode::AllocationLimit,
        );
        assert_eq!(
            limited
                .decode(SubresourceIdentity::HighResolution {
                    mip: 0,
                    frame: 1,
                    face: Face::Right,
                    slice: 0,
                })
                .unwrap()
                .samples
                .len(),
            12,
        );
    }

    #[test]
    fn decodes_stored_rgba_rows_without_channel_reordering() {
        let mut bytes = header(ordinary(1, 0, 2, 1), 0);
        bytes.extend_from_slice(&[10, 20, 30, 40, 50, 60, 70, 80]);
        let plane = decode(
            &bytes,
            Dialect::Source2013Pc,
            SubresourceIdentity::HighResolution {
                mip: 0,
                frame: 0,
                face: Face::Right,
                slice: 0,
            },
            Limits::default(),
        )
        .unwrap();
        assert_eq!(plane.samples, [10, 20, 30, 40, 50, 60, 70, 80]);
        assert_eq!(plane.row_stride, 8);
        assert_eq!(plane.channel_layout, ChannelLayout::Rgba);
        assert_eq!(plane.alpha_encoding, AlphaEncoding::A8);
    }

    #[test]
    fn parses_v73_image_resources_thumbnail_and_bc1_edge_crop() {
        let mut input = ordinary(3, 13, 3, 2);
        input.low_format = 13;
        input.low_dimensions = (4, 4);
        let mut bytes = header(input, 2);
        let low_offset = bytes.len();
        bytes.extend_from_slice(&[0, 0xf8, 0xe0, 0x07, 0, 0, 0, 0]);
        let high_offset = bytes.len();
        bytes.extend_from_slice(&[0, 0xf8, 0xe0, 0x07, 0, 0, 0, 0]);
        resource(&mut bytes, 0, LOW_IMAGE_TAG, 0, low_offset as u32);
        resource(&mut bytes, 1, HIGH_IMAGE_TAG, 0, high_offset as u32);
        let metadata = inspect(&bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        assert_eq!(metadata.header_size, 96);
        assert_eq!(metadata.resources.len(), 2);
        assert_eq!(metadata.subresources.len(), 2);
        let low = decode(
            &bytes,
            Dialect::Source2013Pc,
            SubresourceIdentity::LowResolution,
            Limits::default(),
        )
        .unwrap();
        assert_eq!(low.width, 4);
        assert_eq!(low.height, 4);
        assert_eq!(low.samples.len(), 64);
        let high = decode(
            &bytes,
            Dialect::Source2013Pc,
            SubresourceIdentity::HighResolution {
                mip: 0,
                frame: 0,
                face: Face::Right,
                slice: 0,
            },
            Limits::default(),
        )
        .unwrap();
        assert_eq!(high.samples.len(), 3 * 2 * 4);
        assert!(
            high.samples
                .chunks_exact(4)
                .all(|sample| sample == [255, 0, 0, 255])
        );
    }

    #[test]
    fn decodes_bc3_alpha_and_preserves_srgb_label() {
        let mut input = ordinary(4, 15, 4, 4);
        input.flags = SRGB_FLAG;
        let mut bytes = header(input, 1);
        let high_offset = bytes.len();
        bytes.extend_from_slice(&[255, 0, 0, 0, 0, 0, 0, 0, 0, 0xf8, 0xe0, 0x07, 0, 0, 0, 0]);
        resource(&mut bytes, 0, HIGH_IMAGE_TAG, 0, high_offset as u32);
        let plane = decode(
            &bytes,
            Dialect::Source2013Pc,
            SubresourceIdentity::HighResolution {
                mip: 0,
                frame: 0,
                face: Face::Right,
                slice: 0,
            },
            Limits::default(),
        )
        .unwrap();
        assert_eq!(plane.alpha_encoding, AlphaEncoding::A8);
        assert_eq!(plane.color_encoding, ColorEncoding::Srgb);
        assert!(
            plane
                .samples
                .chunks_exact(4)
                .all(|sample| sample == [255, 0, 0, 255])
        );
    }

    #[test]
    fn decodes_bgra_rows_from_top_to_bottom_without_changing_alpha() {
        let mut input = ordinary(1, 12, 2, 2);
        input.flags = SRGB_FLAG | EIGHT_BIT_ALPHA_FLAG | CLAMP_S_FLAG | CLAMP_T_FLAG;
        let mut bytes = header(input, 0);
        bytes.extend_from_slice(&[
            3, 2, 1, 4, 7, 6, 5, 8, // stored top row
            11, 10, 9, 12, 15, 14, 13, 16, // stored bottom row
        ]);
        let metadata = inspect(&bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        assert_eq!(
            metadata.alpha_flags,
            AlphaFlags {
                one_bit: false,
                eight_bit: true
            }
        );
        let plane = decode(
            &bytes,
            Dialect::Source2013Pc,
            SubresourceIdentity::HighResolution {
                mip: 0,
                frame: 0,
                face: Face::Right,
                slice: 0,
            },
            Limits::default(),
        )
        .unwrap();
        assert_eq!(plane.row_order, RowOrder::TopToBottom);
        assert_eq!(plane.scalar_encoding, ScalarEncoding::U8);
        assert_eq!(plane.alpha_encoding, AlphaEncoding::A8);
        assert_eq!(plane.color_encoding, ColorEncoding::Srgb);
        assert_eq!(
            plane.samples,
            [
                1, 2, 3, 4, 5, 6, 7, 8, // top row remains first
                9, 10, 11, 12, 13, 14, 15, 16
            ]
        );
    }

    #[test]
    fn retains_native_little_endian_rgba16f_samples() {
        let mut input = ordinary(1, 24, 1, 1);
        input.flags = EIGHT_BIT_ALPHA_FLAG;
        let mut bytes = header(input, 0);
        let samples = [0x00, 0x3c, 0x00, 0x38, 0x00, 0x00, 0x00, 0x3c];
        bytes.extend_from_slice(&samples);
        let plane = decode(
            &bytes,
            Dialect::Source2013Pc,
            SubresourceIdentity::HighResolution {
                mip: 0,
                frame: 0,
                face: Face::Right,
                slice: 0,
            },
            Limits::default(),
        )
        .unwrap();
        assert_eq!(plane.channel_layout, ChannelLayout::Rgba);
        assert_eq!(plane.scalar_encoding, ScalarEncoding::F16);
        assert_eq!(plane.alpha_encoding, AlphaEncoding::A16F);
        assert_eq!(plane.color_encoding, ColorEncoding::Linear);
        assert_eq!(plane.row_stride, 8);
        assert_eq!(plane.samples, samples);
    }

    #[test]
    fn resolves_source_sampling_precedence_from_explicit_environment() {
        let mut input = ordinary(3, 15, 4, 4);
        input.flags = EIGHT_BIT_ALPHA_FLAG | CLAMP_S_FLAG | CLAMP_T_FLAG | NO_LOD_FLAG;
        let mut bytes = header(input, 1);
        let image_offset = bytes.len();
        bytes.extend_from_slice(&[0; 16]);
        resource(&mut bytes, 0, HIGH_IMAGE_TAG, 0, image_offset as u32);
        let metadata = inspect(&bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        let default_sampling = sampling_state(
            &metadata,
            SamplingEnvironment {
                shader_model: 90,
                force_anisotropy: 1,
                maximum_anisotropy: 16,
                force_trilinear: false,
            },
        );
        assert_eq!(default_sampling.wrap_s, WrapMode::Clamp);
        assert_eq!(default_sampling.wrap_t, WrapMode::Clamp);
        assert_eq!(default_sampling.wrap_u, WrapMode::Repeat);
        assert_eq!(default_sampling.min_filter, MinFilter::LinearMipmapNearest);
        assert_eq!(default_sampling.mag_filter, MagFilter::Linear);
        assert!(default_sampling.mipmapped);
        assert!(default_sampling.no_lod);

        let forced = sampling_state(
            &metadata,
            SamplingEnvironment {
                shader_model: 90,
                force_anisotropy: 8,
                maximum_anisotropy: 16,
                force_trilinear: true,
            },
        );
        assert_eq!(forced.min_filter, MinFilter::Anisotropic);
        assert_eq!(forced.mag_filter, MagFilter::Anisotropic);

        let mut point_input = ordinary(1, 12, 1, 1);
        point_input.flags = POINT_SAMPLE_FLAG | BORDER_FLAG | NO_MIP_FLAG | TRILINEAR_FLAG;
        let mut point_bytes = header(point_input, 0);
        point_bytes.extend_from_slice(&[0; 4]);
        let point_metadata =
            inspect(&point_bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        let point = sampling_state(
            &point_metadata,
            SamplingEnvironment {
                shader_model: 90,
                force_anisotropy: 16,
                maximum_anisotropy: 16,
                force_trilinear: true,
            },
        );
        assert_eq!(point.wrap_s, WrapMode::Border);
        assert_eq!(point.wrap_t, WrapMode::Border);
        assert_eq!(point.wrap_u, WrapMode::Border);
        assert_eq!(point.min_filter, MinFilter::Nearest);
        assert_eq!(point.mag_filter, MagFilter::Nearest);
        assert!(!point.mipmapped);
    }

    #[test]
    fn labels_normal_and_ssbump_payloads_as_non_color() {
        for flag in [NORMAL_FLAG, SS_BUMP_FLAG] {
            let mut input = ordinary(1, 3, 1, 1);
            input.flags = flag | SRGB_FLAG;
            let mut bytes = header(input, 0);
            bytes.extend_from_slice(&[3, 2, 1]);
            let plane = decode(
                &bytes,
                Dialect::Source2013Pc,
                SubresourceIdentity::HighResolution {
                    mip: 0,
                    frame: 0,
                    face: Face::Right,
                    slice: 0,
                },
                Limits::default(),
            )
            .unwrap();
            assert_eq!(plane.color_encoding, ColorEncoding::NotColor);
        }
    }

    #[test]
    fn enumerates_source2013_sphere_faces_frames_and_smallest_mip_first() {
        let mut input = ordinary(4, 3, 2, 2);
        input.flags = ENVMAP_FLAG;
        input.frames = 2;
        input.mips = 2;
        let mut bytes = header(input, 1);
        let high_offset = bytes.len();
        let image_count = 7 * 2 * 2;
        bytes.resize(bytes.len() + 7 * 2 * (3 + 12), 1);
        resource(&mut bytes, 0, HIGH_IMAGE_TAG, 0, high_offset as u32);
        let metadata = inspect(&bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        assert_eq!(metadata.faces.len(), 7);
        assert_eq!(metadata.faces[6], Face::Sphere);
        assert_eq!(metadata.subresources.len(), image_count);
        assert_eq!(
            metadata.subresources[0].identity,
            SubresourceIdentity::HighResolution {
                mip: 1,
                frame: 0,
                face: Face::Right,
                slice: 0,
            }
        );
        assert_eq!(metadata.subresources[0].width, 1);
    }

    #[test]
    fn source2013_cubemap_topology_uses_version_not_start_frame() {
        let mut v70 = ordinary(0, 3, 1, 1);
        v70.flags = ENVMAP_FLAG;
        v70.start_frame = 0;
        let mut v70_bytes = header(v70, 0);
        v70_bytes.extend_from_slice(&[0; 6 * 3]);
        let v70_metadata = inspect(&v70_bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        assert_eq!(v70_metadata.faces.len(), 6);

        let mut v71 = ordinary(1, 3, 1, 1);
        v71.flags = ENVMAP_FLAG;
        v71.start_frame = u16::MAX;
        let mut v71_bytes = header(v71, 0);
        v71_bytes.extend_from_slice(&[0; 7 * 3]);
        let v71_metadata = inspect(&v71_bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        assert_eq!(v71_metadata.faces.len(), 7);
        assert_eq!(v71_metadata.faces[6], Face::Sphere);
    }

    #[test]
    fn retains_custom_resources_and_classifies_format_and_selector_failures() {
        let mut bytes = header(ordinary(4, 13, 4, 4), 2);
        let custom_offset = bytes.len();
        bytes.extend_from_slice(&3_u32.to_le_bytes());
        bytes.extend_from_slice(b"abc");
        let high_offset = bytes.len();
        bytes.extend_from_slice(&[0; 8]);
        resource(&mut bytes, 0, HIGH_IMAGE_TAG, 0, high_offset as u32);
        resource(&mut bytes, 1, *b"CRC", 0, custom_offset as u32);
        let metadata = inspect(&bytes, Dialect::Source2013Pc, Limits::default()).unwrap();
        assert_eq!(
            metadata.resources[1].data,
            ResourceData::External {
                range: custom_offset + 4..custom_offset + 7,
                bytes: b"abc".to_vec(),
            }
        );
        let invalid_selector = SubresourceIdentity::HighResolution {
            mip: 1,
            frame: 0,
            face: Face::Right,
            slice: 0,
        };
        assert_eq!(
            decode(
                &bytes,
                Dialect::Source2013Pc,
                invalid_selector,
                Limits::default()
            )
            .unwrap_err()
            .code,
            ErrorCode::SelectorOutOfRange
        );

        let mut unsupported_bytes = header(ordinary(1, 7, 1, 1), 0);
        unsupported_bytes.push(0);
        assert_eq!(
            decode(
                &unsupported_bytes,
                Dialect::Source2013Pc,
                SubresourceIdentity::HighResolution {
                    mip: 0,
                    frame: 0,
                    face: Face::Right,
                    slice: 0,
                },
                Limits::default()
            )
            .unwrap_err()
            .classification,
            Classification::Unsupported
        );
    }

    #[test]
    fn rejects_header_resource_range_and_allocation_boundaries() {
        assert_eq!(
            inspect(&[], Dialect::Source2013Pc, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidSignature
        );
        let mut bad_header = header(ordinary(2, 3, 1, 1), 0);
        bad_header[12..16].copy_from_slice(&64_u32.to_le_bytes());
        assert_eq!(
            inspect(&bad_header, Dialect::Source2013Pc, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::InvalidHeaderSize
        );
        let mut unsorted = header(ordinary(4, 13, 4, 4), 2);
        let high_offset = unsorted.len();
        unsorted.extend_from_slice(&[0; 8]);
        resource(&mut unsorted, 0, HIGH_IMAGE_TAG, 0, high_offset as u32);
        resource(&mut unsorted, 1, LOW_IMAGE_TAG, 0, high_offset as u32);
        assert_eq!(
            inspect(&unsorted, Dialect::Source2013Pc, Limits::default())
                .unwrap_err()
                .code,
            ErrorCode::ResourceOrder
        );
        let bytes = header(ordinary(1, 3, 1, 1), 0);
        assert_eq!(
            inspect(
                &bytes,
                Dialect::Source2013Pc,
                Limits {
                    max_encoded_bytes: bytes.len() - 1,
                    ..Limits::default()
                }
            )
            .unwrap_err()
            .code,
            ErrorCode::AllocationLimit
        );
    }
}
