type Member = {
  role: number
  source?: { slot: number; version: number }
  encodedBytes: number
  decodedBytes: number
  encodedHash: Uint8Array
  decodedHash: Uint8Array
  itemCount: number
}

export type HdrFixture = Readonly<{
  bytes: Uint8Array
  closureOffset: number
  profileTextureHashOffset: number
  profileReservedOffset: number
}>

const encoder = new TextEncoder()

const hash = (bytes: Uint8Array): Uint8Array =>
  new Uint8Array(new Bun.CryptoHasher("sha256").update(bytes).digest())

export function hdrFixture(styles: readonly number[] = [0], linearAttenuation = 0, radius = 100): HdrFixture {
  const bytes: number[] = []
  const u8 = (value: number) => bytes.push(value & 0xff)
  const u16 = (value: number) => bytes.push(value & 0xff, value >>> 8 & 0xff)
  const u32 = (value: number) => bytes.push(value & 0xff, value >>> 8 & 0xff, value >>> 16 & 0xff, value >>> 24 & 0xff)
  const i32 = (value: number) => u32(value >>> 0)
  const f32 = (value: number) => bytes.push(...new Uint8Array(new Float32Array([value]).buffer))
  const raw = (value: Uint8Array | readonly number[]) => bytes.push(...value)
  const sized = (value: Uint8Array) => { u32(value.byteLength); raw(value) }
  const text = (value: string) => sized(encoder.encode(value))

  raw(encoder.encode("PSMP"))
  u32(7)
  u32(20)
  u32(731)
  u8(1)
  u32(1)
  u32(1)
  const sampleCount = styles.length * 4
  u32(sampleCount)
  u32(1)
  text("materials/test.vmt")
  i32(64)
  i32(64)

  u32(0)
  u32(0)
  u32(0)
  i32(0x0800)
  u8(1)
  u32(3)
  u32(1)
  for (const value of [0, 0, 0, 1, 0, 0, 0, 1, 0]) f32(value)
  for (const value of [0, 0, 1, 0, 0, 1, 0, 0, 1]) f32(value)
  for (const value of [0, 0, 1, 0, 0, 1]) f32(value)
  for (const value of [0, 0, 1, 0, 0, 1]) f32(value)
  u32(0)
  u32(1)
  u32(2)
  i32(0)
  for (let index = 0; index < 4; index += 1) u8(styles[index] ?? 255)
  i32(0)
  i32(0)

  const samples = styles.flatMap((style) => [
    [20 + style, 1 + style, 0.5 + style],
    [2 + style, 0.5 + style, 0.25 + style],
    [4 + style, 1 + style, 0.5 + style],
    [6 + style, 1.5 + style, 0.75 + style],
  ])
  for (const sample of samples) for (const value of sample) f32(value)
  sized(encoder.encode("{}\0"))
  u32(1)
  u8(1)
  u8(1 << 5)
  u8(1)
  u8(0)
  text("materials/test.vtf")
  u32(1)
  u32(1)
  u32(0)
  u32(0)

  raw(encoder.encode("PSHD"))
  u32(1)
  u8(1)
  raw([0, 0, 0])
  text("map-runtime-hdr")
  text("playsrc-map-runtime-hdr-1")
  raw(new Uint8Array(32).fill(0x11))
  raw(new Uint8Array(32).fill(0x22))
  const closureOffset = bytes.length
  raw(new Uint8Array(32))

  const member = (role: number, slot: number | undefined, version: number, decodedBytes: number, itemCount: number): Member => {
    const source = new Uint8Array(decodedBytes || 1).fill(role)
    return {
      role,
      source: slot === undefined ? undefined : { slot, version },
      encodedBytes: slot === undefined ? 0 : decodedBytes,
      decodedBytes: slot === undefined ? 0 : decodedBytes,
      encodedHash: slot === undefined ? new Uint8Array(32) : hash(source),
      decodedHash: slot === undefined ? new Uint8Array(32) : hash(source),
      itemCount,
    }
  }
  const members = [
    member(1, 58, 1, 56, 1),
    member(2, 53, 1, sampleCount * 4, sampleCount),
    member(3, 54, 0, 88, 1),
    member(4, 51, 0, 4, 1),
    member(5, 55, 1, 28, 1),
    member(6, 59, 0, 4, 1),
    member(7, undefined, 0, 0, 0),
    member(8, undefined, 0, 0, 0),
    member(9, undefined, 0, 0, 0),
    member(10, undefined, 0, 0, 0),
  ]
  const closure: number[] = [...encoder.encode("playsrc-lighting-profile-v1"), 1]
  const closureU32 = (value: number) => closure.push(value & 0xff, value >>> 8 & 0xff, value >>> 16 & 0xff, value >>> 24 & 0xff)
  for (const item of members) {
    closure.push(item.role)
    if (item.source) {
      closure.push(1, item.source.slot)
      closureU32(item.source.version >>> 0)
    } else closure.push(0)
    closureU32(item.encodedBytes)
    closureU32(item.decodedBytes)
    closure.push(...item.encodedHash, ...item.decodedHash)
    closureU32(item.itemCount)
  }
  const closureHash = hash(new Uint8Array(closure))
  bytes.splice(closureOffset, 32, ...closureHash)

  u32(members.length)
  for (const item of members) {
    u8(item.role)
    if (item.source) {
      u8(1)
      u8(item.source.slot)
      raw([0, 0])
      i32(item.source.version)
    } else raw(new Uint8Array(8))
    u32(item.encodedBytes)
    u32(item.decodedBytes)
    raw(item.encodedHash)
    raw(item.decodedHash)
    u32(item.itemCount)
  }

  u32(1)
  u32(1)
  u32(1)
  u32(0)
  u8(3)
  u8(styles.length)
  u8(4)
  u8(0)
  u32(0)
  u32(1)
  for (let index = 0; index < 4; index += 1) u8(styles[index] ?? 255)

  u32(1)
  for (const value of [1, 2, 3, 4, 5, 6, 0, 0, -1]) f32(value)
  i32(0)
  i32(1)
  u8(0)
  raw([0, 0, 0])
  for (const value of [0.9, 0.8, 1, radius, 1, linearAttenuation, 0]) f32(value)
  i32(0)
  i32(-1)
  i32(-1)

  u32(1)
  u16(1)
  u16(0)
  u32(1)
  for (let side = 0; side < 6; side += 1) {
    f32(side + 1)
    f32(side + 2)
    f32(side + 3)
  }
  raw([64, 128, 255, 0])
  u32(0)
  u32(0)
  u32(0)
  u32(0)

  const vtf = new Uint8Array(80)
  const vtfView = new DataView(vtf.buffer)
  vtf.set([0x56, 0x54, 0x46, 0], 0)
  vtfView.setUint32(4, 7, true)
  vtfView.setUint32(8, 2, true)
  vtfView.setUint32(12, 80, true)
  vtfView.setUint16(16, 1, true)
  vtfView.setUint16(18, 1, true)
  vtfView.setInt32(52, 12, true)
  u32(1)
  text("materials/skybox/test_hdrrt.vmt")
  u8(8)
  u8(0)
  u8(2)
  const profileReservedOffset = bytes.length
  u8(0)
  text("materials/skybox/test_hdrrt.vtf")
  u32(1)
  u32(1)
  i32(12)
  const profileTextureHashOffset = bytes.length
  raw(hash(vtf))
  sized(vtf)

  u32(1)
  u8(1)
  raw([0, 0, 0])
  text("materials/test.vmt")
  raw(hash(encoder.encode("material")))
  return Object.freeze({
    bytes: new Uint8Array(bytes),
    closureOffset,
    profileTextureHashOffset,
    profileReservedOffset,
  })
}
