# VTF Roadmap

[`../../../docs/roadmap-contract.md`](../../../docs/roadmap-contract.md) defines the normative roadmap schema and denominator review gate. [`../../../TERMINOLOGY.md`](../../../TERMINOLOGY.md) defines delivery status, coverage classification, inventory, and completion terms.

## Completion Denominator

This roadmap contains exactly 37 behavior rows. The three candidate inventories contain 117 decisions: 6 version contracts, 70 dialect-qualified image-format code mappings, and 41 flag/resource contracts. They contribute 0 denominator items until checked-in generators emit them and a denominator review accepts them. The current completion denominator therefore contains 37 items and is Not accepted.

The target is little-endian PC VTF used by Team Fortress 2, Counter-Strike: Source, and legacy Source 1 Counter-Strike: Global Offensive. The `source-2013-pc` dialect covers the official Source SDK 2013 contract. The `asw-pc` dialect is the current candidate for legacy Source 1 CS:GO and remains Blocked until an exact declared CS:GO content build and authoritative dialect mapping are available.

## Inputs

- Exact bytes whose logical identity is a `.vtf` resource and whose first four bytes are `VTF\0`.
- One explicit dialect identity: `source-2013-pc` or `asw-pc`. The parser never infers a dialect from an image-format code, flag bit, version minor, path, or file size.
- One explicit selector: `LowResolution` or `HighResolution { mip, frame, face, slice }`, with every index zero-based.
- A caller-supplied allocation budget containing maximum encoded bytes, maximum decoded bytes, maximum resource bytes, and maximum subresource count. The accepted numeric defaults are Blocked.
- Valve Source SDK 2013 commit `88fa198fba3fb85d46d4c95018254693fdc3af0a`: `src/public/vtf/vtf.h`, `src/public/bitmap/imageformat.h`, `src/utils/vtf2tga/vtf2tga.cpp`, and `src/utils/vtfdiff/vtfdiff.cpp`.
- Public Alien Swarm SDK header snapshot commit `bc047d62ea6529c92d37b2544ed971cd73dc7ad7`: `src/public/vtf/vtf.h` and `src/public/bitmap/imageformat.h`.
- Public Valve Developer Community VTF contract, revision `Unknown`, and srctools VTF branch documentation at commit `5a8ed668f34693f59f26cc04ff367788ed738f8b`.
- Microsoft BC1, BC2, BC3, BC4, and BC5 block-compression contracts and IEEE 754 binary16/binary32 contracts.
- Exact VPK/directory indexes for one declared TF2, CS:S, and legacy Source 1 CS:GO content build. These inputs are Missing.

## Outputs

- A canonical metadata record containing dialect, version, header size, width, height, depth, frame count, start frame, face topology, mip count, raw flags, version-filtered flags, reflectivity as three stored-order binary32 bit patterns, bump scale as one binary32 bit pattern, high- and low-resolution format identities, low-resolution dimensions, and ordered resource entries.
- An ordered subresource descriptor sequence. High-resolution identities are `(mip, frame, face, slice)`; the low-resolution image has the sole identity `LowResolution`.
- A selected canonical decoded plane containing identity, width, height, tightly packed row stride, `StoredRowOrder`, channel layout, scalar encoding, color encoding, alpha encoding, and exact sample bytes. Channel layouts are `R`, `RG`, `RGB`, `RGBA`, `Intensity`, `IntensityAlpha`, and `Alpha`. Scalar encodings are `U8`, `U16`, `F16`, and `F32`. Color encodings are `Linear`, `Srgb`, `PwlCorrected`, `PreSrgb`, `NotColor`, and `Unspecified`. Alpha encodings are `None`, `Opaque`, `A1`, `A2`, `A4`, `A8`, `A16`, `A16F`, and `A32F`.
- Typed known-resource output and bounded opaque bytes for custom resources.
- One deterministic error containing a coverage classification, stable code, byte offset or selector identity, and offending value or required range. The finite code set is `invalid-signature`, `unsupported-dialect`, `unsupported-version`, `truncated-header`, `invalid-header-size`, `invalid-dimensions`, `invalid-cubemap`, `invalid-frame-count`, `invalid-mip-count`, `invalid-format`, `unknown-format`, `unsupported-format`, `invalid-thumbnail`, `resource-limit`, `resource-order`, `duplicate-resource`, `invalid-resource-flags`, `missing-image-resource`, `invalid-resource-range`, `overlapping-resource`, `invalid-known-resource`, `truncated-image`, `selector-out-of-range`, `arithmetic-overflow`, and `allocation-limit`.

The decoder preserves native precision. It does not force every format through RGBA8, PNG, a browser image, or a GPU texture.

## Invariants

- All integer and floating-point fields are read little-endian. Every range, product, sum, alignment, and allocation uses checked arithmetic before reading or allocating.
- Header sizes are exactly 64 bytes for 7.0/7.1, 80 bytes for 7.2, and `80 + 8 × resourceCount` bytes for 7.3 through 7.5. A resource count is in `0..=32`.
- Width, height, depth, and frame count are nonzero. Versions 7.0/7.1 imply depth 1. Cubemaps are square and have depth 1. A volume texture is not a cubemap.
- Mip `m` has dimensions `max(1, width >> m) × max(1, height >> m) × max(1, depth >> m)`. The declared mip count is in `1..=floor(log2(max(width, height, depth))) + 1`.
- High-resolution bytes are ordered from the smallest declared mip to mip 0; within one mip they are ordered by ascending frame, face, then slice.
- Face identities are `Right`, `Left`, `Back`, `Front`, `Up`, `Down`, and, only when the accepted contract requires it, `Sphere`.
- A block-compressed 2D slice uses `ceil(width / 4) × ceil(height / 4)` blocks. BC1/BC4 blocks contain 8 bytes; BC2/BC3/BC5 blocks contain 16 bytes. Partial edge blocks decode only texels inside the selected dimensions.
- Versions 7.0 through 7.2 place the optional low-resolution image immediately after the header and the high-resolution image immediately after it. Versions 7.3 through 7.5 locate both images through resource entries.
- A low-resolution image is absent when both low-resolution dimensions are zero. Its format field is preserved without reading image bytes. One zero dimension with one nonzero dimension is Malformed; two nonzero dimensions require a recognized format and complete image range.
- Resource entries are strictly ascending by their 24-bit tag, contain no duplicate tag, and use only the `0x02` inline-data flag. Image resources are never inline. External non-image resources contain a four-byte payload length followed by exactly that payload.
- Decoding changes byte/channel packing only. It never applies gamma transfer, premultiplies alpha, reconstructs a normal-map Z channel, interprets shader-compressed HDR, selects an animation frame by time, flips rows, or generates missing mip levels.
- Raw flags and unknown custom-resource tags are preserved even when their semantic effect belongs to another module. Unknown image-format codes never fall back to RGBA8888.
- Inspection does not allocate decoded image storage. Selection validates every index and the complete selected byte range before allocating its output.
- External package schemas, PNG artifacts, renderer capabilities, and product behavior never alter VTF delivery status.
- VTF parsing never returns Missing because exact input bytes are a precondition. `packages/content` classifies an unresolved logical path as Missing before invoking VTF.

## Ownership Exclusions

- VMT parsing and patch composition belong to `packages/formats/vmt`; material parameter, shader, color-transfer, compressed-HDR, alpha-test, normal-map, and animation-proxy semantics belong to `packages/world/material`.
- Logical-path lookup, provider ordering, and game/content-build selection belong to `packages/content`.
- GPU format selection, upload, sampler state, texture lifetime, frame animation presentation, cubemap sampling, and pixels belong to `packages/presentation/rendering`.
- Particle-sheet sequence use, particle frame selection, and particle rendering belong to `packages/presentation/particle`; VTF owns only the resource container and typed sheet payload.
- Reflectivity and bump-scale use belong to the consuming world/material, lighting, or rendering owner. VTF preserves their stored values.
- PNG, JPEG, BMP, PSD, PFM, and TGA container decoding does not belong to VTF. The parent format inventory currently assigns texture-source raster interchange to VTF, while this assignment excludes it. Proposed owner `packages/formats/raster-image` requires a root ownership decision.
- Texture compilation, mip generation, compression, VTF serialization, and command orchestration are outside this parsing denominator. Repeated command orchestration belongs to `tools/playsrc`.
- Content-addressed artifacts and publication state belong to `packages/asset-store`.

## Behavior Families

### `jump_beef` world-surface parity checkpoint

| Source/TF2 behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Selected VTF rows retain stored top-to-bottom order; map/decal UV V=0 addresses the first stored row and no format decoder flips rows. | `RowOrder::TopToBottom` is explicit and every channel/block decoder retains row identity. | Asymmetric BGR and BGRA top/bottom vectors plus unchanged BC block row traversal. | Ready |
| Stored wrap/filter flags resolve under one explicit PC sampling environment with Source precedence: border, per-axis clamp/repeat, point, no-mip, forced anisotropy/trilinear, texture anisotropy/trilinear, then linear mip-nearest. | `sampling_state` resolves all listed state without inferring browser/GPU capabilities. | Isolated precedence vectors plus the 13 exact clamp-S/T alpha decal metadata records. | Ready |
| Stored one-bit/eight-bit alpha flags remain distinct from decoded channel capacity; BGRA8888 and RGBA16F join existing BGR888, DXT1, and DXT5 as exact target formats. | Metadata emits both alpha facts; BGRA becomes RGBA/U8 and RGBA16F remains little-endian RGBA/F16. Normal and SSBump payloads are `NotColor`. | BGRA channel/alpha, native F16 bit, BC alpha, normal/SSBump, and allocation vectors pass. | Ready |
| Every world/decal/sky/cubemap VTF in the declared target closure has one exact identity, format, dimensions, frame/mip/face topology, flags, and decoder disposition. | The 46-row fixed inventory handles all encountered codes 3, 12, 13, 15, and 24. | [`inventories/jump-beef-world-textures.md`](inventories/jump-beef-world-textures.md), exact source-bundle SHA-256, and successful native HDR closure compilation. | Ready |

The active migration checkpoints implement the explicit `source-2013-pc` dialect for VTF 7.0 through 7.5; exact sequential/resource-directory image framing; 2D, animated, volume, and Source-2013 cubemap descriptors; selected low/high subresources; bounded opaque custom resources; top-to-bottom rows; explicit sampling state; and canonical planes for BGR888, BGRA8888, BC1/DXT1, one-bit-alpha BC1, BC3/DXT5, and RGBA16F required by the 46 `jump_beef` world/decal/sky/cubemap texture identities. Other image-format codes remain explicit `Unsupported` or `Unknown`.

| Target behavior | playsrc behavior | Evidence | Status |
|---|---|---|---|
| Every accepted PC VTF is identified by `VTF\0`, an explicit dialect, and one allowed 7.x version; another signature, dialect, major, or minor never enters a fallback parser. | The Rust parser requires `source-2013-pc`, `VTF\0`, major 7, and minor 0–5. The legacy CS:GO dialect remains unavailable. | Signature/version vectors pass; exact `jump_beef` dependencies contain three v7.1, 18 v7.3, and six v7.4 files. | Partial |
| Each version uses its exact aligned header size and field availability. | Versions 7.0/7.1 require 64 bytes, 7.2 requires 80, and 7.3–7.5 require exactly `80 + 8*resourceCount`. | Fixed v7.1/v7.2/v7.3/v7.4 and mismatched-size vectors pass; complete minor-version truncation matrix remains absent. | Partial |
| Width, height, and depth produce one validated 2D, cubemap, or volume topology. | Nonzero dimensions produce explicit face and per-mip slice topology; cubemaps require square 2D faces. | Flat and seven-face cubemap vectors plus exact dependencies pass; volume boundary matrix remains absent. | Partial |
| Frame count is nonzero and start frame is preserved without becoming playback state. | Both fields remain metadata; zero frames fail. | One-, two-, and 60-frame exact/synthetic inputs pass. | Ready |
| Declared mip count and per-mip dimensions are exact for 2D and volume textures. | Mip count is bounded by dimensions and every descriptor uses `max(1, dimension >> mip)`. | Two-mip and exact 6–11-mip inputs pass; volume vectors remain absent. | Partial |
| Reflectivity and bump scale retain their exact stored binary32 bits and stored component order. | Metadata stores all four raw bit patterns. | Signed-zero and NaN-payload synthetic fields compare exactly. | Ready |
| All 32 flag bits retain raw state and receive dialect/version-qualified meaning without applying renderer or material behavior. | The exact legacy CS:GO meanings for conflicting bits are unresolved. | Generate the flag table from pinned dialect headers; test each bit alone and in combinations across 7.3, 7.4, and 7.5. | Blocked |
| Versions 7.0 through 7.2 derive low- and high-resolution regions from the exact sequential layout. | Sequential low/high offsets use exact header and computed thumbnail length. | Absent-thumbnail v7.1 and mismatched v7.2 vectors pass; present-thumbnail and trailing-byte evidence remain incomplete. | Partial |
| Versions 7.3 through 7.5 validate the resource directory count, size, sort order, tags, duplicates, and image entries. | Count 0–32, exact header extent, strict tag order, unique image tags, and required high image are validated. | Two-resource, unsorted, and 27 exact-source directories pass. Complete count/duplicate matrix remains absent. | Partial |
| Inline and external resource entries use exact flag, offset, length-prefix, and range rules. | Inline `u32`, bounded external length-prefixed bytes, image offsets, flags, truncation, and cross-resource overlap are represented or rejected. | One external custom resource and malformed range/order vectors pass; inline custom evidence remains absent. | Partial |
| Low- and high-resolution resource presence agrees with the header dimensions and image contract. | High image is mandatory; nonzero thumbnail dimensions require low image; image resources cannot be inline. | Present/absent thumbnail and exact-source inputs pass. | Ready |
| `SHEET`, `CRC`, `LOD`, `TS0`, and `STR` resources produce their exact typed fields; malformed known payloads fail. | No parser exists. | Fixed resource payload vectors for every accepted payload version, count, size, reserved field, and truncation boundary. | Not started |
| An unrecognized 24-bit resource tag yields bounded opaque bytes or one inline `u32` without semantic invention. | Custom tags retain raw identity, flags, inline value or exact external bytes. | External custom vector passes; inline vector remains absent. | Partial |
| The optional low-resolution image decodes through its declared dimensions and format as the sole `LowResolution` subresource. | A present thumbnail has one stable identity and uses the same selected-plane decoder. | BC1 thumbnail and malformed dimensions pass. | Partial |
| High-resolution byte length and every subresource offset follow smallest-mip, frame, face, slice disk order. | Inspection emits every range in that exact nested order after validating the complete high-image extent. | Synthetic multi-mip/frame/cubemap and all 905 exact selected subresources pass. | Ready |
| A flat texture exposes one face and one slice at each mip for every frame. | Flat descriptors use `Right` as the sole stable face identity and one shrinking slice when depth is one. | Exact flat textures across 1 and 60 frames pass. | Ready |
| Animated texture frames remain independently selectable and retain ascending on-disk order; VTF supplies no frame duration. | Every frame has an independent identity and no timing field exists. `Decoder` inspects immutable bytes once, exposes the same canonical metadata, and decodes every authored high-resolution mip/frame/face/slice in disk order under one aggregate decoded-byte bound. | Two-frame/two-mip BGR source-order, exact sample bytes, aggregate-limit rejection, unchanged single-plane selection, and configured 60-frame water-normal vectors pass. | Ready |
| A cubemap exposes the accepted ordered face set and rejects non-square or volume combinations. | Source-2013 v7.0 disk data emits right, left, back, front, up, and down; v7.1 and later additionally emit the sphere fallback. | Synthetic six/seven-face framing and three exact seven-face map cubemaps pass. Broader v7.5 dialect evidence remains absent. | Partial |
| Version and dialect determine whether the sphere fallback has stored bytes; start frame remains animation metadata and never changes topology. | The explicit Source-2013 dialect uses six faces for v7.0 and seven for v7.1 onward, independent of the retained start-frame value. | Synthetic boundary vectors and three configured TF2 v7.4 cubemaps produce correctly bounded seven-face layouts. CS:S and legacy-CS:GO evidence remain absent. | Partial |
| A volume texture exposes `max(1, depth >> mip)` ascending slices per frame and mip. | Descriptor generation implements shrinking volume slices. | Depth-one exact use passes; depth 2–16 marker evidence remains absent. | Partial |
| High-resolution selection validates mip, frame, face, and slice independently and reports the offending selector component. | Selection matches one complete typed identity or returns it in `SelectorOutOfRange`. | Valid identities and one invalid mip pass; complete axis boundary matrix remains absent. | Partial |
| Selected decoding reads and allocates only the chosen plane after validating the complete file-level layout. | Inspection validates all ranges; decoding allocates only the selected output plane. | All 905 exact subresources decode independently. Bounded-reader instrumentation remains absent because current input is an immutable full object. | Partial |
| Stable byte-addressed color formats decode channel order to native-precision canonical planes. | BGR888 emits RGB/U8 and BGRA8888 emits RGBA/U8 with unchanged top-to-bottom rows; other byte-addressed formats remain explicit non-decoded identities. | Two-pixel BGR, asymmetric BGRA channel/row/alpha, four exact BGR cubemap, and six exact BGRA HDR-sky sources pass. | Partial |
| Intensity, intensity-alpha, alpha-only, and bluescreen formats preserve their distinct channel contracts and exact key transparency. | No parser exists. | Exhaustive 8-bit endpoint vectors and keyed/non-keyed blue pixels for codes 5, 6, 8, 9, and 10. | Not started |
| Packed 16-bit color formats unpack little-endian bit fields without losing source precision. | No parser exists. | Exhaustive component-endpoint vectors for RGB565, BGR565, BGRX5551, BGRA4444, and BGRA5551; compare integer channels exactly. | Not started |
| BC1 and DXT1 one-bit-alpha blocks decode both endpoint-order modes and crop edge blocks. | BC1 opaque and one-bit-alpha palettes are distinct and edge texels outside dimensions are omitted. | A 3x2 crop and opaque palette vector passes; complete alpha/palette/size matrix remains absent. | Partial |
| BC2/DXT3 blocks decode explicit four-bit alpha and BC1 color independently. | No parser exists. | Microsoft BC2 vectors covering every alpha nibble and color selector; exact RGBA8 comparison. | Not started |
| BC3/DXT5 blocks decode both alpha interpolation modes and BC1 color independently. | BC3 alpha/color palettes and packed selectors decode independently to RGBA/U8. | One endpoint-order vector and all 15 exact BC3 sources pass; complete selector matrix remains absent. | Partial |
| Dialect-qualified ATI1N/ATI2N codes decode BC4 to `R/U8` and BC5 to `RG/U8` without inventing B, A, or normal Z. | No parser exists. | BC4/BC5 vectors under both dialect code maps; compare every channel byte and code identity. | Not started |
| UV, integer-HDR, half-float, and float formats preserve native channels and precision without RGBA8 clamping. | RGBA16F emits unchanged little-endian RGBA/F16 samples; the other listed formats remain explicit non-decoded identities. | Native F16 bit vector plus the three exact HDR cubemap sources pass; UV88, UVWQ8888, UVLX8888, RGBA16, R32F, RGB32F, and RGBA32F remain required. | Partial |
| `asw-pc` codes 30 through 60 have one authoritative legacy-CS:GO mapping and one exact decoded or deterministic Unsupported result. | Public branch headers establish the candidate mapping; no authoritative declared legacy CS:GO build binds that mapping to the target. | Compare generated code identities against an exact legacy CS:GO archive index and retained target metadata/pixel captures for every encountered code. | Blocked |
| P8 and dialect depth/stencil/null formats without a canonical CPU sample contract return deterministic Unsupported; unrecognized numeric codes return Unknown. Neither result guesses byte size or pixels. | Known unsupported storage can be structurally framed but selected decode returns `Unsupported`; unknown codes return `Unknown` before inventing image size. | P8 selected decode passes; complete unsupported/unknown code matrix remains absent. | Partial |
| Canonical planes identify layout, scalar width, color encoding, alpha encoding, row stride, and exact bytes; all consumers use that output. | Selected output contains every declared field and never emits PNG. Material/Rendering consumers are not implemented. | BGR, BC1, and BC3 vectors compare complete descriptors and bytes. | Partial |
| Decoding preserves numeric samples and only labels dialect-qualified color/alpha metadata; gamma, premultiplication, HDR shader expansion, and normal reconstruction never occur. | The decoder performs only channel/block unpacking and labels sRGB from the raw flag. | Equal block samples with linear/sRGB labels and exact normal-map bytes pass without semantic reconstruction. | Ready |
| Malformed counts, sizes, offsets, products, overlaps, and truncation fail before out-of-range reads or allocations. | All fixed and variable ranges use checked arithmetic and pairwise data-range overlap checks. | Header, resource order/range, truncation, and allocation vectors pass; complete property matrix remains absent. | Partial |
| Encoded, decoded, resource, and subresource allocations obey one accepted bounded policy on 32-bit and 64-bit runtimes. | Four caller limits are enforced; defaults remain unaccepted and cross-runtime evidence is absent. | Encoded over-limit and exact-source runs pass on macOS arm64. | Partial |
| Every parser failure is exactly Malformed, Unsupported, or Unknown and reports one finite stable code plus location; Missing remains a Content result and no failure invokes another decoder. | One typed error contract separates all three classifications and contains no content lookup or fallback path. | Signature, version, format, resource, selector, and allocation failures pass; exhaustive error-table coverage remains absent. | Partial |

## Generated Inventories

No generated VTF inventory is accepted. Accepted inventory item count: 0.

| Output | Authority identity | Authority revision | Generator command | Candidate items |
|---|---|---|---|---:|
| [`inventories/versions.md`](inventories/versions.md) | Official Source SDK VTF header, public Alien Swarm VTF header snapshot, VTF public contract, and three declared-game content indexes | SDK `88fa198f...`; public snapshot `bc047d62...`; VTF page revision `Unknown`; content indexes Missing | Missing | 6 |
| [`inventories/image-formats.md`](inventories/image-formats.md) | Official Source SDK and public Alien Swarm `ImageFormat` declarations plus BC/IEEE contracts and three declared-game content indexes | SDK `88fa198f...`; public snapshot `bc047d62...`; content indexes Missing | Missing | 70 |
| [`inventories/flags-and-resources.md`](inventories/flags-and-resources.md) | Both pinned VTF headers, VTF public resource contract, and three declared-game content indexes | SDK `88fa198f...`; public snapshot `bc047d62...`; VTF page revision `Unknown`; content indexes Missing | Missing | 41 |

Current manually derived candidate count: 117. Current generated count: 0.

## Exit Criteria

VTF is Complete only when all of these predicates pass:

- All 37 behavior rows are Ready.
- The parent format-universe denominator is Accepted and assigns VTF container parsing, subresource selection, format decoding, and canonical decoded texture output to this package.
- The version, image-format, and flag/resource inventories are generated, current, Accepted, and contain exactly the authority-backed current item counts.
- Exact TF2, CS:S, and legacy Source 1 CS:GO content-build indexes are inputs to generation; every indexed VTF has one dialect, version, image format, topology, and resource classification.
- The legacy CS:GO dialect and pre-7.5 sphere-face rule are resolved by authoritative evidence; no related item remains Unknown or Blocked.
- Every Handled image-format mapping has fixed native-precision vectors. Every Unsupported mapping returns its declared deterministic result without reading an invented payload length.
- All parser operations pass exact-at-bound, one-over-bound, overflow, truncation, overlap, and selector-range evidence on every supported runtime.
- `packages/world/material`, `packages/presentation/rendering`, `packages/presentation/particle`, and declared tools consume the canonical metadata/plane interface; no duplicate decoder, PNG-only parser contract, fallback, or legacy reader remains.
- No required declared-content VTF remains Unsupported, Unknown, Missing, Malformed, Partial, or Blocked.

## Blockers

- **Parent inventory gate:** [`../ROADMAP.md`](../ROADMAP.md) marks the 62-row format inventory candidate and Not accepted. No checked-in generator or denominator review accepts its VTF assignment.
- **Declared content indexes:** `playsrc.local.json` is Missing and no checked TF2, CS:S, or legacy CS:GO archive index is available. Checked `playsrc.local.example.json`, the format-universe roadmap/inventory, and the assigned VTF tree. Filesystem discovery was not attempted.
- **Legacy CS:GO dialect:** the official Source SDK 2013 header ends at VTF 7.4; the public Alien Swarm header and public secondary parser establish a candidate 7.5 code/flag mapping, but no authoritative declared legacy CS:GO build binds it to the target.
- **Cross-dialect sphere topology:** the Source-2013 PC contract is fixed at six stored v7.0 faces and seven stored v7.1+ faces. Exact CS:S and legacy-CS:GO samples remain required before applying that topology to another accepted dialect.
- **Raster ownership:** [`../inventories/formats.md`](../inventories/formats.md) assigns PNG, JPEG, BMP, PSD, PFM, and TGA interchange to VTF, but this owner is limited to VTF bytes. A root decision must accept `packages/formats/raster-image` or name another owner.
- **Allocation policy:** no accepted package-wide decision defines encoded-byte, decoded-byte, resource-byte, or subresource-count defaults for browser, Node, and native runtimes. Checked `ARCHITECTURE.md`, `TERMINOLOGY.md`, the VTF README, and current package roadmaps.
- **Inventory generators:** no checked-in command emits any assigned VTF inventory. Checked `tools/`, root manifests, and the VTF package tree.
- **Public VTF revision:** the checked Valve Developer Community page exposes no immutable revision identity; its revision is `Unknown` until a retained revision or content hash is recorded.
