import { test } from "@playwright/test"

test("measure browser execution boundaries", async ({ page }) => {
  await page.goto("data:text/html,<canvas id='canvas' width='1024' height='1024'></canvas><main></main>")
  const report = await page.evaluate(async () => {
    const measure = (name: string, operations: number, run: () => void) => {
      run()
      const started = performance.now()
      run()
      const milliseconds = performance.now() - started
      return { name, operations, milliseconds, nanosecondsPerOperation: milliseconds * 1_000_000 / operations }
    }

    const iterations = 5_000_000
    let sink = 0
    const js = measure("js-call", iterations, () => {
      const add = (left: number, right: number) => (left + right) | 0
      for (let index = 0; index < iterations; index += 1) sink = add(sink, index)
    })
    const module = await WebAssembly.instantiate(Uint8Array.from([
      0,97,115,109,1,0,0,0,1,7,1,96,2,127,127,1,127,3,2,1,0,7,7,1,3,97,100,100,0,0,10,9,1,7,0,32,0,32,1,106,11,
    ]))
    const add = (module.instance.exports.add as (left: number, right: number) => number)
    const wasm = measure("wasm-call", iterations, () => {
      for (let index = 0; index < iterations; index += 1) sink = add(sink, index)
    })

    const workerSource = `onmessage=event=>{if(event.data.kind==='canvas'){const c=event.data.canvas,x=c.getContext('2d'),n=event.data.n,s=performance.now();for(let i=0;i<n;i++){x.fillStyle='rgb('+i%255+',0,0)';x.fillRect(i%1024,(i/1024)|0,1,1)}postMessage({id:event.data.id,milliseconds:performance.now()-s})}else postMessage({id:event.data.id,buffer:event.data.buffer},event.data.buffer?[event.data.buffer]:[])}`
    const worker = new Worker(URL.createObjectURL(new Blob([workerSource], { type: "text/javascript" })))
    let nextId = 1
    const pending = new Map<number, (value: any) => void>()
    worker.onmessage = (event) => {
      pending.get(event.data.id)?.(event.data)
      pending.delete(event.data.id)
    }
    const request = (value: any, transfer: Transferable[] = []) => new Promise<any>((resolve) => {
      const id = nextId++
      pending.set(id, resolve)
      worker.postMessage({ id, ...value }, transfer)
    })
    const sequentialCount = 10_000
    let started = performance.now()
    for (let index = 0; index < sequentialCount; index += 1) {
      const buffer = new ArrayBuffer(48)
      await request({ kind: "echo", buffer }, [buffer])
    }
    const sequentialMilliseconds = performance.now() - started
    const pipelineCount = 10_000
    started = performance.now()
    await Promise.all(Array.from({ length: pipelineCount }, () => {
      const buffer = new ArrayBuffer(48)
      return request({ kind: "echo", buffer }, [buffer])
    }))
    const pipelineMilliseconds = performance.now() - started

    const main = document.querySelector("main") as HTMLElement
    const domIterations = 100_000
    const dom = measure("dom-dataset-write", domIterations, () => {
      for (let index = 0; index < domIterations; index += 1) main.dataset.value = String(index)
    })
    const object: Record<string, string> = {}
    const objectWrite = measure("object-property-write", domIterations, () => {
      for (let index = 0; index < domIterations; index += 1) object.value = String(index)
    })

    const canvas = document.querySelector("canvas") as HTMLCanvasElement
    const context = canvas.getContext("2d")!
    const drawCount = 100_000
    started = performance.now()
    for (let index = 0; index < drawCount; index += 1) {
      context.fillStyle = `rgb(${index % 255},0,0)`
      context.fillRect(index % 1024, Math.floor(index / 1024), 1, 1)
    }
    const mainCanvasMilliseconds = performance.now() - started
    const offscreen = new OffscreenCanvas(1024, 1024)
    const workerCanvas = await request({ kind: "canvas", canvas: offscreen, n: drawCount }, [offscreen])
    worker.terminate()

    return {
      sink,
      calls: [js, wasm],
      worker: {
        sequentialCount,
        sequentialMilliseconds,
        sequentialMicrosecondsPerRoundTrip: sequentialMilliseconds * 1_000 / sequentialCount,
        pipelineCount,
        pipelineMilliseconds,
        pipelineMicrosecondsPerRoundTrip: pipelineMilliseconds * 1_000 / pipelineCount,
      },
      state: [objectWrite, dom],
      canvas2d: {
        drawCount,
        mainMilliseconds: mainCanvasMilliseconds,
        workerMilliseconds: workerCanvas.milliseconds,
      },
    }
  })
  console.log(`PLAYSRCCROSSBOUNDARY ${JSON.stringify(report)}`)
})
