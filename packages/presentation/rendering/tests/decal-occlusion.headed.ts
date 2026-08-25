#!/usr/bin/env bun

import { chromium } from "@playwright/test"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { pathToFileURL } from "node:url"
import { inflateRawSync } from "node:zlib"
import { loadLocalConfig } from "../../../../tools/playsrc/src/config"
import { projectedDecalDepthBias } from "../src/decal-occlusion"
import { parseRuntimeMap } from "../src/runtime-map"

const MAP_SHA256 = "735995d68920adcb971fe4c5e773986f438c2a95c07c935882dc7fd081ce1e3a"
const MAP_BYTES = 78_255_714
const VIEWPORT = Object.freeze({ width: 960, height: 540 })
const DIGITS = Object.freeze([
  Object.freeze({
    digit: 1,
    source: 345,
    face: 233,
    logicalPath: "materials/signs/number_01.vtf",
    sha256: "de1e006ace891068e7abd37d094967db81256fcb6e41ba23e2cffacb16d4fbc1",
    center: Object.freeze([12_695.9, 672, -2_391.88] as const),
  }),
  Object.freeze({
    digit: 4,
    source: 346,
    face: 229,
    logicalPath: "materials/signs/number_04.vtf",
    sha256: "0b1abe48cf9e1b8fa16956a7ee5642dff55cab62aea2488829357954c286619d",
    center: Object.freeze([12_695.9, 548.148, -2_392.56] as const),
  }),
])
const SOURCE_TEXTURE_BYTES = 43_896
const AUTHORED_MIP_ZERO_OFFSET = 11_128
const AUTHORED_MIP_ZERO_BYTES = 32_768

type GraphEntry = Readonly<{ logicalPath: string; byteLength: string; offset: string; sha256: string }>
type GraphChunk = Readonly<{
  codec: string
  encodedSha256: string
  encodedByteLength: string
  decodedSha256: string
  decodedByteLength: string
  entries: readonly GraphEntry[]
}>
type ScenarioResult = Readonly<{
  name: string
  camera: readonly number[]
  receiverDepth: number
  opaqueDepth: number | null
  whitePixels: number
  changedOpaquePixels: number
  representative: null | Readonly<{ x: number; y: number; rgba: readonly number[]; depth: number }>
  comparison: null | Readonly<{ x: number; y: number; rgba: readonly number[]; depth: number }>
}>

function hash(bytes: Uint8Array): string {
  return new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
}

function require(condition: unknown, message: string): asserts condition {
  if (!condition) throw new Error(message)
}

async function sourceTexture(
  cacheDirectory: string,
  graph: readonly GraphChunk[],
  input: typeof DIGITS[number],
): Promise<Readonly<{ logicalPath: string; sha256: string; mipZero: readonly number[] }>> {
  const matches = graph.flatMap((chunk) => chunk.entries
    .filter((entry) => entry.logicalPath === input.logicalPath)
    .map((entry) => ({ chunk, entry })))
  require(matches.length === 1, `${input.logicalPath} has no unique configured graph entry`)
  const { chunk, entry } = matches[0]!
  require(chunk.codec === "deflate" && entry.sha256 === input.sha256
    && Number(entry.byteLength) === SOURCE_TEXTURE_BYTES, `${input.logicalPath} differs from its fixed source identity`)
  const encoded = new Uint8Array(await readFile(path.join(
    cacheDirectory, "browser-bundles", "jump_beef.graph", "objects", chunk.encodedSha256,
  )))
  require(encoded.byteLength === Number(chunk.encodedByteLength)
    && hash(encoded) === chunk.encodedSha256, `${input.logicalPath} encoded graph chunk differs`)
  const decoded = inflateRawSync(encoded)
  require(decoded.byteLength === Number(chunk.decodedByteLength)
    && hash(decoded) === chunk.decodedSha256, `${input.logicalPath} decoded graph chunk differs`)
  const source = decoded.subarray(Number(entry.offset), Number(entry.offset) + Number(entry.byteLength))
  require(hash(source) === input.sha256, `${input.logicalPath} source bytes differ`)
  const mipZero = source.subarray(AUTHORED_MIP_ZERO_OFFSET)
  require(mipZero.byteLength === AUTHORED_MIP_ZERO_BYTES, `${input.logicalPath} authored BC3 mip range differs`)
  return Object.freeze({ logicalPath: input.logicalPath, sha256: input.sha256, mipZero: Array.from(mipZero) })
}

async function main(): Promise<void> {
  const configuration = await loadLocalConfig()
  const evidenceDirectory = path.join(configuration.sourceCacheDir, "evidence", "tf2-browser-performance", "decal-occlusion")
  await mkdir(evidenceDirectory, { recursive: true })
  const mapBytes = new Uint8Array(await readFile(path.join(
    configuration.sourceCacheDir, "browser-bundles", "jump_beef.native-hdr.psmp",
  )))
  require(mapBytes.byteLength === MAP_BYTES && hash(mapBytes) === MAP_SHA256,
    "configured jump_beef HDR runtime-map identity differs")
  const map = parseRuntimeMap(mapBytes)
  const surface = (face: number, material: string): readonly number[][] => {
    const triangles: number[][] = []
    for (const batch of map.batches) {
      if (map.materials[batch.material]!.logicalPath !== material) continue
      for (let triangle = 0; triangle < batch.faces.length; triangle += 1) {
        if (batch.faces[triangle] !== face) continue
        triangles.push(Array.from({ length: 9 }, (_, value) => {
          const vertex = batch.indices[triangle * 3 + Math.floor(value / 3)]!
          return batch.positions[vertex * 3 + value % 3]!
        }))
      }
    }
    require(triangles.length > 0, `configured receiver/occluder face ${face} is unavailable`)
    return triangles
  }
  const receiverFaces = [
    { face: 229, triangles: surface(229, "materials/WOOD/WALL020B.vmt") },
    { face: 233, triangles: surface(233, "materials/WOOD/WALL020B.vmt") },
  ]
  const opaqueFace = { face: 288, triangles: surface(288, "materials/WOOD/WALL007A.vmt") }
  const graph = JSON.parse(await readFile(path.join(
    configuration.sourceCacheDir, "browser-bundles", "jump_beef.graph.json",
  ), "utf8")) as { target?: string; chunks?: readonly GraphChunk[] }
  require(graph.target === "jump_beef" && Array.isArray(graph.chunks), "configured jump_beef resource graph is malformed")
  const textures = await Promise.all(DIGITS.map((digit) => sourceTexture(configuration.sourceCacheDir, graph.chunks!, digit)))
  const bias = projectedDecalDepthBias("decal")
  const browser = await chromium.launch({ headless: false })
  const captures: Array<ScenarioResult & { capture: string; captureSha256: string }> = []

  try {
    const page = await browser.newPage({ viewport: { width: VIEWPORT.width, height: VIEWPORT.height + 80 } })
    await page.goto(pathToFileURL(path.join(configuration.sourceCacheDir, "browser-bundles", "catalog.json")).href)
    await page.setContent(`<html><body style="margin:0;background:#111;color:#eee;font:15px system-ui"><h1 id="title" style="height:60px;margin:0;padding:10px 16px;box-sizing:border-box">Configured jump_beef projected-number wall occlusion</h1><canvas id="scene" width="${VIEWPORT.width}" height="${VIEWPORT.height}" style="display:block"></canvas></body></html>`)
    await page.evaluate(async ({ receiverFaces, opaqueFace, textures, digits, bias, viewport }) => {
      const adapter = await navigator.gpu?.requestAdapter()
      if (!adapter?.features.has("texture-compression-bc")) throw new Error("headed WebGPU adapter lacks authored BC3 texture support")
      const device = await adapter.requestDevice({ requiredFeatures: ["texture-compression-bc"] })
      const canvas = document.querySelector<HTMLCanvasElement>("#scene")!
      const context = canvas.getContext("webgpu")!
      const format = navigator.gpu.getPreferredCanvasFormat()
      context.configure({ device, format, alphaMode: "opaque", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC })
      const depth = device.createTexture({ size: [viewport.width, viewport.height], format: "depth24plus", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING })
      const sampler = device.createSampler({ magFilter: "linear", minFilter: "linear", addressModeU: "clamp-to-edge", addressModeV: "clamp-to-edge" })
      const digitTextures = textures.map((input) => {
        const texture = device.createTexture({ size: [128, 256], format: "bc3-rgba-unorm-srgb", usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST })
        device.queue.writeTexture({ texture }, Uint8Array.from(input.mipZero), { bytesPerRow: 512, rowsPerImage: 64 }, [128, 256])
        return texture
      })
      const source = device.createShaderModule({ code: `
        struct Out { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) color: vec3<f32> }
        @vertex fn vertex(@location(0) position: vec4<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec3<f32>) -> Out {
          var result: Out; result.position = position; result.uv = uv; result.color = color; return result;
        }
        @fragment fn opaque(input: Out) -> @location(0) vec4<f32> { return vec4<f32>(input.color, 1.0); }
        @group(0) @binding(0) var decalSampler: sampler;
        @group(0) @binding(1) var decalTexture: texture_2d<f32>;
        @fragment fn decal(input: Out) -> @location(0) vec4<f32> { return textureSample(decalTexture, decalSampler, input.uv); }
      ` })
      const vertex = { module: source, entryPoint: "vertex", buffers: [{ arrayStride: 36, attributes: [
        { shaderLocation: 0, offset: 0, format: "float32x4" as const },
        { shaderLocation: 1, offset: 16, format: "float32x2" as const },
        { shaderLocation: 2, offset: 24, format: "float32x3" as const },
      ] }] }
      const opaquePipeline = device.createRenderPipeline({ layout: "auto", vertex, primitive: { topology: "triangle-list", cullMode: "back" }, fragment: { module: source, entryPoint: "opaque", targets: [{ format }] }, depthStencil: { format: "depth24plus", depthWriteEnabled: true, depthCompare: "less-equal" } })
      const decalPipeline = device.createRenderPipeline({ layout: "auto", vertex, primitive: { topology: "triangle-list", cullMode: "back" }, fragment: { module: source, entryPoint: "decal", targets: [{ format, blend: { color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" }, alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" } } }] }, depthStencil: { format: "depth24plus", depthWriteEnabled: false, depthCompare: "less-equal", depthBias: bias.units, depthBiasSlopeScale: bias.slopeScale } })
      const bindGroups = digitTextures.map((texture) => device.createBindGroup({ layout: decalPipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: sampler }, { binding: 1, resource: texture.createView() }] }))
      const readDepth = device.createShaderModule({ code: `
        @group(0) @binding(0) var surface: texture_depth_2d;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;
        @compute @workgroup_size(8, 8) fn main(@builtin(global_invocation_id) id: vec3<u32>) {
          let extent = textureDimensions(surface);
          if (id.x >= extent.x || id.y >= extent.y) { return; }
          output[id.y * extent.x + id.x] = textureLoad(surface, vec2<i32>(id.xy), 0);
        }
      ` })
      const depthPipeline = device.createComputePipeline({ layout: "auto", compute: { module: readDepth, entryPoint: "main" } })
      const depthStorage = device.createBuffer({ size: viewport.width * viewport.height * 4, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC })
      const depthBindGroup = device.createBindGroup({ layout: depthPipeline.getBindGroupLayout(0), entries: [{ binding: 0, resource: depth.createView() }, { binding: 1, resource: { buffer: depthStorage } }] })
      let referenceColors: Uint8Array | undefined
      let referenceDepths: Float32Array | undefined
      let representativePosition: { x: number; y: number } | undefined
      const near = 7, far = 28_377.919921875
      const verticalTangent = Math.tan(59.84044400898543 * Math.PI / 360)
      const horizontalTangent = verticalTangent * viewport.width / viewport.height

      ;(globalThis as any).__renderDecalScenario = async (name: string, camera: readonly number[], includeOpaque: boolean, includeDecals: boolean) => {
        document.querySelector("#title")!.textContent = `Source projected 14: ${name}`
        const center = [12_695.9, 610, -2_392]
        const difference = center.map((value, index) => value - camera[index]!)
        const distance = Math.hypot(...difference)
        const forward = difference.map((value) => value / distance)
        const horizontal = Math.hypot(forward[0]!, forward[1]!)
        const right = [forward[1]! / horizontal, -forward[0]! / horizontal, 0]
        const up = [-forward[0]! * forward[2]! / horizontal, -forward[1]! * forward[2]! / horizontal, horizontal]
        const view = (point: readonly number[]) => {
          const relative = point.map((value, index) => value - camera[index]!)
          return [
            relative.reduce((sum, value, index) => sum + value * right[index]!, 0),
            relative.reduce((sum, value, index) => sum + value * up[index]!, 0),
            relative.reduce((sum, value, index) => sum + value * forward[index]!, 0),
          ]
        }
        const clip = (point: readonly number[]) => {
          const value = view(point)
          return [value[0]! / horizontalTangent, value[1]! / verticalTangent, value[2]! * far / (far - near) - far * near / (far - near), value[2]!]
        }
        const rgba = (colors: Uint8Array, x: number, y: number) => {
          const offset = (y * viewport.width + x) * 4
          return format.startsWith("bgra")
            ? [colors[offset + 2]!, colors[offset + 1]!, colors[offset]!, colors[offset + 3]!]
            : Array.from(colors.subarray(offset, offset + 4))
        }
        const buffers: GPUBuffer[] = []
        const vertices = (triangles: readonly number[][], color: readonly number[]) => {
          const values: number[] = []
          for (const triangle of triangles) {
            const corners = Array.from({ length: 3 }, (_, vertex) => triangle.slice(vertex * 3, vertex * 3 + 3))
            if (corners.every((point) => view(point)[2]! <= near)) continue
            for (const point of corners) values.push(...clip(point), 0, 0, ...color)
          }
          if (!values.length) return null
          const bytes = Float32Array.from(values)
          const buffer = device.createBuffer({ size: bytes.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST })
          buffers.push(buffer)
          device.queue.writeBuffer(buffer, 0, bytes)
          return { buffer, count: bytes.length / 9 }
        }
        const receiver = vertices(receiverFaces.flatMap((face) => face.triangles), [0.14, 0.20, 0.27])
        const opaque = includeOpaque ? vertices(opaqueFace.triangles, [0.34, 0.20, 0.13]) : null
        const markBuffers = digits.map((digit) => {
          const [x, y, z] = digit.center
          const corners = [
            { point: [x, y + 64, z - 128], uv: [0, 1] },
            { point: [x, y - 64, z + 128], uv: [1, 0] },
            { point: [x, y + 64, z + 128], uv: [0, 0] },
            { point: [x, y - 64, z - 128], uv: [1, 1] },
          ]
          const values = [0, 1, 2, 0, 3, 1].flatMap((index) => [...clip(corners[index]!.point), ...corners[index]!.uv, 1, 1, 1])
          const bytes = Float32Array.from(values)
          const buffer = device.createBuffer({ size: bytes.byteLength, usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST })
          buffers.push(buffer)
          device.queue.writeBuffer(buffer, 0, bytes)
          return buffer
        })
        const colorTexture = context.getCurrentTexture()
        const rowBytes = Math.ceil(viewport.width * 4 / 256) * 256
        const colorReadback = device.createBuffer({ size: rowBytes * viewport.height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })
        const depthReadback = device.createBuffer({ size: viewport.width * viewport.height * 4, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })
        const encoder = device.createCommandEncoder()
        const render = encoder.beginRenderPass({ colorAttachments: [{ view: colorTexture.createView(), clearValue: { r: 0.03, g: 0.04, b: 0.05, a: 1 }, loadOp: "clear", storeOp: "store" }], depthStencilAttachment: { view: depth.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" } })
        render.setPipeline(opaquePipeline)
        if (receiver) { render.setVertexBuffer(0, receiver.buffer); render.draw(receiver.count) }
        if (opaque) { render.setVertexBuffer(0, opaque.buffer); render.draw(opaque.count) }
        if (includeDecals) for (let index = 0; index < markBuffers.length; index += 1) {
          render.setPipeline(decalPipeline)
          render.setBindGroup(0, bindGroups[index]!)
          render.setVertexBuffer(0, markBuffers[index]!)
          render.draw(6)
        }
        render.end()
        const compute = encoder.beginComputePass()
        compute.setPipeline(depthPipeline)
        compute.setBindGroup(0, depthBindGroup)
        compute.dispatchWorkgroups(Math.ceil(viewport.width / 8), Math.ceil(viewport.height / 8))
        compute.end()
        encoder.copyTextureToBuffer({ texture: colorTexture }, { buffer: colorReadback, bytesPerRow: rowBytes }, [viewport.width, viewport.height])
        encoder.copyBufferToBuffer(depthStorage, 0, depthReadback, 0, viewport.width * viewport.height * 4)
        device.queue.submit([encoder.finish()])
        await Promise.all([colorReadback.mapAsync(GPUMapMode.READ), depthReadback.mapAsync(GPUMapMode.READ)])
        const padded = new Uint8Array(colorReadback.getMappedRange())
        const colors = new Uint8Array(viewport.width * viewport.height * 4)
        for (let y = 0; y < viewport.height; y += 1) colors.set(padded.subarray(y * rowBytes, y * rowBytes + viewport.width * 4), y * viewport.width * 4)
        const depths = new Float32Array(depthReadback.getMappedRange().slice(0))
        colorReadback.unmap(); depthReadback.unmap(); colorReadback.destroy(); depthReadback.destroy()
        const projected = digits.map(({ center }) => {
          const value = clip(center)
          return { x: Math.round((value[0]! / value[3]! + 1) * viewport.width / 2), y: Math.round((1 - value[1]! / value[3]!) * viewport.height / 2) }
        })
        const minX = Math.max(0, Math.min(...projected.map((value) => value.x)) - 64)
        const maxX = Math.min(viewport.width - 1, Math.max(...projected.map((value) => value.x)) + 64)
        const minY = Math.max(0, Math.min(...projected.map((value) => value.y)) - 100)
        const maxY = Math.min(viewport.height - 1, Math.max(...projected.map((value) => value.y)) + 100)
        let whitePixels = 0, changedOpaquePixels = 0
        let firstWhite: { x: number; y: number } | undefined
        for (let y = minY; y <= maxY; y += 1) for (let x = minX; x <= maxX; x += 1) {
          const value = rgba(colors, x, y)
          if (value[0]! > 120 && value[1]! > 115 && value[2]! > 95) {
            whitePixels += 1
            firstWhite ??= { x, y }
            if (referenceColors) {
              const before = rgba(referenceColors, x, y)
              if (value.slice(0, 3).some((channel, index) => Math.abs(channel - before[index]!) > 12)) changedOpaquePixels += 1
            }
          }
        }
        if (name === "opaque-control") { referenceColors = colors; referenceDepths = depths }
        if (name === "unobstructed-back") representativePosition = firstWhite
        const point = representativePosition ?? firstWhite
        const representative = point ? { ...point, rgba: rgba(colors, point.x, point.y), depth: depths[point.y * viewport.width + point.x]! } : null
        const comparison = point && referenceColors && referenceDepths
          ? { ...point, rgba: rgba(referenceColors, point.x, point.y), depth: referenceDepths[point.y * viewport.width + point.x]! }
          : null
        for (const buffer of buffers) buffer.destroy()
        return { name, camera: [...camera], receiverDepth: distance, opaqueDepth: includeOpaque && opaque ? distance * (12_064 - camera[0]!) / (12_695.9 - camera[0]!) : null, whitePixels, changedOpaquePixels, representative, comparison }
      }
      ;(globalThis as any).__disposeDecalScenario = () => { depthStorage.destroy(); depth.destroy(); for (const texture of digitTextures) texture.destroy(); device.destroy() }
    }, {
      receiverFaces,
      opaqueFace,
      textures,
      digits: DIGITS,
      bias,
      viewport: VIEWPORT,
    })

    for (const scenario of [
      { name: "opaque-control", camera: [11_600, 0, -2_392], opaque: true, decals: false },
      { name: "unobstructed-back", camera: [11_600, 0, -2_392], opaque: false, decals: true },
      { name: "occluded-back", camera: [11_600, 0, -2_392], opaque: true, decals: true },
      { name: "unobstructed-front", camera: [12_400, 610, -2_392], opaque: true, decals: true },
    ] as const) {
      const result = await page.evaluate(({ name, camera, opaque, decals }) =>
        (globalThis as any).__renderDecalScenario(name, camera, opaque, decals), scenario) as ScenarioResult
      const capture = path.join(evidenceDirectory, `${scenario.name}.png`)
      await page.locator("#scene").screenshot({ path: capture })
      const bytes = new Uint8Array(await readFile(capture))
      captures.push(Object.freeze({ ...result, capture, captureSha256: hash(bytes) }))
      await page.waitForTimeout(250)
    }
    await page.evaluate(() => (globalThis as any).__disposeDecalScenario())
  } finally {
    await browser.close()
  }

  const report = Object.freeze({
    schema: "playsrc-headed-decal-occlusion-v1",
    mapSha256: MAP_SHA256,
    mapBytes: MAP_BYTES,
    depthFormat: "depth24plus",
    source: DIGITS.map(({ digit, source, face, logicalPath, sha256, center }) => ({ digit, source, face, logicalPath, sha256, center })),
    opaqueFace: opaqueFace.face,
    receiverFaces: receiverFaces.map(({ face }) => face),
    bias,
    captures,
  })
  const output = path.join(evidenceDirectory, "report.json")
  await writeFile(output, `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify({ output, ...report }, null, 2))
  const opaque = captures.find((capture) => capture.name === "opaque-control")!
  const back = captures.find((capture) => capture.name === "unobstructed-back")!
  const occluded = captures.find((capture) => capture.name === "occluded-back")!
  const front = captures.find((capture) => capture.name === "unobstructed-front")!
  require(opaque.whitePixels === 0, `opaque control unexpectedly contains ${opaque.whitePixels} painted-number pixels`)
  require(back.whitePixels > 100, `actual authored 14 is not visible without the opaque face: ${back.whitePixels} pixels`)
  require(front.whitePixels > 100, `actual authored 14 is not visible from the unobstructed front: ${front.whitePixels} pixels`)
  require(occluded.whitePixels === 0 && occluded.changedOpaquePixels === 0,
    `actual authored 14 remains visible through opaque face 288: ${occluded.whitePixels} white pixels, ${occluded.changedOpaquePixels} changed wall pixels`)
  require(occluded.representative?.depth === occluded.comparison?.depth,
    "projected decal changed the stored opaque-world depth")
}

await main()
