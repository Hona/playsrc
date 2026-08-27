import { test, expect } from "./application-test"
import path from "node:path"

test("partial pixel visibility counts real covered MSAA samples", async ({ page }, testInfo) => {
  await page.route("**/__pixel-visibility-fixture", route => route.fulfill({ contentType: "text/html", body: `<!doctype html><title>Source pixel visibility raster acceptance</title><style>body{background:#222;color:white;font:20px sans-serif}canvas{width:512px;height:512px;image-rendering:pixelated}</style><h1>Source partial pixel visibility</h1><canvas width="64" height="64"></canvas><pre id="result"></pre>` }))
  await page.goto("/__pixel-visibility-fixture")
  await page.bringToFront()
  const result = await page.evaluate(async modulePath => {
    const { SourcePixelVisibility } = await import(modulePath)
    const adapter = await navigator.gpu.requestAdapter()
    if (!adapter) throw new Error("WebGPU adapter unavailable")
    const device = await adapter.requestDevice()
    const errors: string[] = []
    device.addEventListener("uncapturederror", event => errors.push(event.error.message))
    const canvas = document.querySelector("canvas")!
    const context = canvas.getContext("webgpu")!
    const format = navigator.gpu.getPreferredCanvasFormat()
    context.configure({ device, format, alphaMode: "opaque" })
    const counter = new SourcePixelVisibility(device)
    const results: unknown[] = []
    for (const samples of [1, 4]) {
      await counter.prepare(samples)
      const depth = device.createTexture({ size: [64, 64], sampleCount: samples, format: "depth32float", usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING })
      const color = samples === 1 ? null : device.createTexture({ size: [64, 64], sampleCount: samples, format, usage: GPUTextureUsage.RENDER_ATTACHMENT })
      const module = device.createShaderModule({ code: `
        @vertex fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4f {
          let p = array<vec2f, 3>(vec2f(-1., -1.), vec2f(3., -1.), vec2f(-1., 3.)); return vec4f(p[index], 0.5, 1.);
        }
        struct Output { @location(0) color: vec4f, @builtin(frag_depth) depth: f32 };
        @fragment fn fragment(@builtin(position) position: vec4f${samples > 1 ? ", @builtin(sample_index) sample: u32" : ""}) -> Output {
          let blocked = ${samples > 1 ? "sample < 2u" : "position.x < 32."};
          return Output(select(vec4f(0.2,0.8,0.3,1.), vec4f(0.8,0.2,0.3,1.), blocked), select(0.75, 0.25, blocked));
        }` })
      const pipeline = await device.createRenderPipelineAsync({ layout: "auto", vertex: { module, entryPoint: "vertex" }, fragment: { module, entryPoint: "fragment", targets: [{ format }] }, depthStencil: { format: "depth32float", depthWriteEnabled: true, depthCompare: "always" }, multisample: { count: samples } })
      const encoder = device.createCommandEncoder()
      const output = context.getCurrentTexture().createView()
      const pass = encoder.beginRenderPass({ colorAttachments: [{ view: color?.createView() ?? output, ...(color ? { resolveTarget: output } : {}), loadOp: "clear", storeOp: "store" }], depthStencilAttachment: { view: depth.createView(), depthLoadOp: "clear", depthClearValue: 1, depthStoreOp: "store" } })
      pass.setPipeline(pipeline); pass.draw(3); pass.end()
      const vertices = new Float32Array([0.1, 0.5, 0.9].flatMap(z => [0,0,z,1, -.5,.5,z,1, .5,.5,z,1, .5,-.5,z,1, -.5,-.5,z,1]))
      const read = counter.issue(encoder, depth, vertices)!
      if (counter.issue(encoder, depth, vertices) !== null) throw new Error("Pending query overwritten")
      device.queue.submit([encoder.finish()])
      const counts = [...await read()]
      results.push({ samples, counts })
      depth.destroy(); color?.destroy()
    }
    counter.dispose()
    document.querySelector("#result")!.textContent = JSON.stringify(results, null, 2)
    return { results, errors }
  }, `/@fs/${path.resolve("packages/presentation/rendering/src/pixel-visibility.ts")}`)
  await testInfo.attach("sample-counts", { body: JSON.stringify(result), contentType: "application/json" })
  expect(result.errors).toEqual([])
  expect(result.results).toEqual([
    { samples: 1, counts: [1024, 1024, 512, 1024, 0, 1024] },
    { samples: 4, counts: [4096, 4096, 2048, 4096, 0, 4096] },
  ])
  await page.screenshot({ path: testInfo.outputPath("sample-counts.png") })
})
