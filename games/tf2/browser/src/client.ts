import type { DerivedObjectCache } from "@playsrc/asset-store/browser"
import { decodeSnapshot, type Snapshot } from "./codec"
import type { InitialView, WorkerFailureCode, WorkerRequest, WorkerResponse } from "./protocol"

const HASH = /^[0-9a-f]{64}$/
const MAX_PENDING = 64
const MAX_BSP_BYTES = 512 * 1024 * 1024
const MAX_CONFIGURATION_BYTES = 256 * 1024 * 1024
type RequestWithoutId = WorkerRequest extends infer Request
  ? Request extends { id: number }
    ? Omit<Request, "id">
    : never
  : never

export type WorkerLike = Readonly<{
  postMessage(message: WorkerRequest, transfer?: Transferable[]): void
  addEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  addEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  removeEventListener(type: "message", listener: (event: MessageEvent<WorkerResponse>) => void): void
  removeEventListener(type: "error", listener: (event: ErrorEvent) => void): void
  terminate(): void
}>

export type LoadedGame = Readonly<{
  generation: number
  payload: Uint8Array
  payloadSha256: string
  cache: "hit" | "stored"
  presentation: Uint8Array
  presentationSha256: string
  presentationCache: "hit" | "stored" | "unavailable"
  initialView: InitialView
}>
export type StagedGame = LoadedGame
export type SimulationEventBatch = Readonly<{ hostTick: bigint; bytes: Uint8Array; snapshot: Snapshot }>
export type SimulationPublication = Readonly<{ hostFrame: bigint; firstHostTick: bigint; lastHostTick: bigint; selectedTicks: number; interpolation: number; snapshotBytes: Uint8Array; eventBatches: readonly SimulationEventBatch[]; snapshot: Snapshot }>
export type WaterViewPass = Readonly<{ kind: "reflection" | "refraction" | "main" | "intersection"; origin: readonly [number,number,number]; angles: readonly [number,number,number]; renderAboveWater:boolean;renderUnderWater:boolean;renderWaterSurface:boolean;drawEntities:boolean;drawSky2d:boolean;clip:null|Readonly<{height:number;keep:"above"|"below"}>;forcedVisibilityLeaf:number|null;fog:Readonly<{kind:"world"}|{kind:"water";volume:number;heightFog:boolean}>;surfaces:Uint32Array }>
export type WaterViewPlan = Readonly<{ visibleWater:null|Readonly<{volume:number;visibleLeaf:number;eyeLeaf:number;eyeInVolume:boolean;surfaceZ:number;distanceToWater:number|null;material:string;translucent:boolean;evaluated:Readonly<{normalFrame:number;normalTransform:Float32Array;cheapStart:number;cheapEnd:number}>}>;render:Readonly<{cheap:boolean;reflect:boolean;refract:boolean;reflectEntities:boolean;drawSurface:boolean;opaque:boolean}>;nearPlaneIntersects:boolean;passes:readonly WaterViewPass[] }>
export type VisibilityResult = Readonly<{ worldIdentity:string;cacheIdentity:string;outsideWorld:boolean;sky:0|1|2;eyeLeaf:number|null;leaves:readonly number[];areas:readonly number[];surfaces:Uint32Array;water:WaterViewPlan }>

export class Tf2WorkerError extends Error {
  constructor(
    readonly code: WorkerFailureCode | "WorkerFailed" | "Closed" | "BoundExceeded" | "IntegrityFailure",
    readonly detail = 0,
  ) {
    super(detail === 0 ? code : `${code}:${detail}`)
    this.name = "Tf2WorkerError"
  }
}

async function sha256(bytes: Uint8Array): Promise<string> {
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))
  return Array.from(digest, (value) => value.toString(16).padStart(2, "0")).join("")
}
async function presentationKey(key: string): Promise<string> {
  return sha256(new TextEncoder().encode(`playsrc-tf2-presentation-v8\0${key}`))
}

export class Tf2WorkerClient {
  readonly #worker: WorkerLike
  readonly #cache: DerivedObjectCache
  readonly #pending = new Map<
    number,
    {
      resolve: (response: WorkerResponse) => void
      reject: (error: Error) => void
    }
  >()
  #nextId = 1
  #closed = false

  constructor(worker: WorkerLike, cache: DerivedObjectCache) {
    this.#worker = worker
    this.#cache = cache
    worker.addEventListener("message", this.#message)
    worker.addEventListener("error", this.#error)
  }

  readonly #message = (event: MessageEvent<WorkerResponse>): void => {
    const response = event.data
    const pending = response && this.#pending.get(response.id)
    if (!pending) return
    this.#pending.delete(response.id)
    if (response.kind === "failure") pending.reject(new Tf2WorkerError(response.code, response.detail))
    else pending.resolve(response)
  }

  readonly #error = (): void => {
    this.#closed = true
    this.#failAll(new Tf2WorkerError("WorkerFailed"))
    this.#worker.removeEventListener("message", this.#message)
    this.#worker.removeEventListener("error", this.#error)
    this.#worker.terminate()
  }

  #failAll(error: Error): void {
    for (const pending of this.#pending.values()) pending.reject(error)
    this.#pending.clear()
  }

  #request(request: RequestWithoutId, transfer: Transferable[] = []): Promise<WorkerResponse> {
    if (this.#closed) return Promise.reject(new Tf2WorkerError("Closed"))
    if (this.#pending.size >= MAX_PENDING) return Promise.reject(new Tf2WorkerError("BoundExceeded"))
    while (this.#pending.has(this.#nextId)) {
      this.#nextId = this.#nextId === 0xffff_ffff ? 1 : this.#nextId + 1
    }
    const id = this.#nextId
    this.#nextId = id === 0xffff_ffff ? 1 : id + 1
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject })
      try {
        this.#worker.postMessage({ ...request, id } as WorkerRequest, transfer)
      } catch {
        this.#pending.delete(id)
        reject(new Tf2WorkerError("WorkerFailed"))
      }
    })
  }

  async initialize(wasmBytes: Uint8Array, wasmSha256: string): Promise<void> {
    if (wasmBytes.byteLength < 1 || wasmBytes.byteLength > 64 * 1024 * 1024 || !HASH.test(wasmSha256)) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    if ((await sha256(wasmBytes)) !== wasmSha256) throw new Tf2WorkerError("IntegrityFailure")
    const transferred = wasmBytes.slice().buffer
    const response = await this.#request({ kind: "initialize", wasm: transferred, wasmSha256 }, [transferred])
    if (response.kind !== "initialized") throw new Tf2WorkerError("WorkerFailed")
  }

  async stage(
    generation: number,
    bsp: Uint8Array,
    profile: 0 | 1,
    configuration: Uint8Array,
    derivedKey: string,
  ): Promise<StagedGame> {
    if (
      !Number.isSafeInteger(generation) ||
      generation < 1 ||
      generation > 0xffff_ffff ||
      bsp.byteLength < 1 ||
      bsp.byteLength > MAX_BSP_BYTES ||
      configuration.byteLength > MAX_CONFIGURATION_BYTES ||
      !HASH.test(derivedKey)
    ) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const cached = await this.#cache.read(derivedKey)
    const pkey = await presentationKey(derivedKey)
    const cachedPresentation = await this.#cache.read(pkey)
    const bspBuffer = bsp.slice().buffer
    const configurationBuffer = configuration.slice().buffer
    const loaded = await this.#request(
      { kind: "load", generation, profile, bsp: bspBuffer, configuration: configurationBuffer },
      [bspBuffer, configurationBuffer],
    )
    try {
      if (
        loaded.kind !== "loaded" ||
        loaded.generation !== generation ||
        !Number.isSafeInteger(loaded.payloadBytes) ||
        loaded.payloadBytes < 1 ||
        !HASH.test(loaded.payloadSha256) ||
        !Number.isSafeInteger(loaded.presentationBytes) ||
        loaded.presentationBytes < 1 ||
        loaded.presentationBytes > MAX_BSP_BYTES ||
        !HASH.test(loaded.presentationSha256) ||
        !Number.isSafeInteger(loaded.initialView?.entity) ||
        loaded.initialView.entity < 0 ||
        loaded.initialView.entity > 0xffff_ffff ||
        (loaded.initialView.hammerId !== null &&
          (!Number.isSafeInteger(loaded.initialView.hammerId) ||
            loaded.initialView.hammerId < 0 ||
            loaded.initialView.hammerId >= 0xffff_ffff)) ||
        loaded.initialView.position.length !== 3 ||
        loaded.initialView.angles.length !== 3 ||
        ![...loaded.initialView.position, ...loaded.initialView.angles].every(Number.isFinite)
      ) {
        throw new Tf2WorkerError("WorkerFailed")
      }
      let payload: Uint8Array
      let cache: LoadedGame["cache"]
      if (cached) {
        if (cached.byteLength !== loaded.payloadBytes || (await sha256(cached)) !== loaded.payloadSha256) {
          throw new Tf2WorkerError("IntegrityFailure")
        }
        payload = cached
        cache = "hit"
      } else {
        const map = await this.#request({ kind: "read-map", generation })
        if (map.kind !== "map" || map.generation !== generation || !(map.payload instanceof ArrayBuffer)) {
          throw new Tf2WorkerError("WorkerFailed")
        }
        payload = new Uint8Array(map.payload)
        if (payload.byteLength !== loaded.payloadBytes || (await sha256(payload)) !== loaded.payloadSha256) {
          throw new Tf2WorkerError("IntegrityFailure")
        }
        await this.#cache.write(derivedKey, loaded.payloadSha256, payload)
        cache = "stored"
      }
      let presentation: Uint8Array
      let presentationCache: LoadedGame["presentationCache"]
      if (cachedPresentation) {
        if (
          cachedPresentation.byteLength !== loaded.presentationBytes ||
          (await sha256(cachedPresentation)) !== loaded.presentationSha256
        )
          throw new Tf2WorkerError("IntegrityFailure")
        presentation = cachedPresentation
        presentationCache = "hit"
      } else {
        const response = await this.#request({ kind: "read-presentation", generation })
        if (
          response.kind !== "presentation" ||
          response.generation !== generation ||
          !(response.payload instanceof ArrayBuffer)
        )
          throw new Tf2WorkerError("WorkerFailed")
        presentation = new Uint8Array(response.payload)
        if (
          presentation.byteLength !== loaded.presentationBytes ||
          (await sha256(presentation)) !== loaded.presentationSha256
        )
          throw new Tf2WorkerError("IntegrityFailure")
        try {
          await this.#cache.write(pkey, loaded.presentationSha256, presentation)
          presentationCache = "stored"
        } catch {
          presentationCache = "unavailable"
        }
      }
      const released=await this.#request({kind:"release-presentation",generation})
      if(released.kind!=="presentation-released"||released.generation!==generation)throw new Tf2WorkerError("WorkerFailed")
      return Object.freeze({
        generation,
        payload,
        payloadSha256: loaded.payloadSha256,
        cache,
        presentation,
        presentationSha256: loaded.presentationSha256,
        presentationCache,
        initialView: Object.freeze({
          entity: loaded.initialView.entity,
          hammerId: loaded.initialView.hammerId,
          position: Object.freeze([...loaded.initialView.position]) as readonly [number, number, number],
          angles: Object.freeze([...loaded.initialView.angles]) as readonly [number, number, number],
        }),
      })
    } catch (error) {
      try {
        await this.#request({ kind: "discard", generation })
      } catch {
        // The worker can already have failed; the original classified failure remains authoritative.
      }
      throw error
    }
  }

  async activate(generation: number): Promise<void> {
    const activated = await this.#request({ kind: "activate", generation })
    if (activated.kind !== "activated" || activated.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
  }

  async discard(generation: number): Promise<void> {
    const discarded = await this.#request({ kind: "discard", generation })
    if (discarded.kind !== "discarded" || discarded.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
  }

  async configureCourse(generation: number, definition: Uint8Array): Promise<void> {
    if (definition.byteLength < 52 || definition.byteLength > 64 * 1024) {
      throw new Tf2WorkerError("BoundExceeded")
    }
    const transferred = definition.slice().buffer
    const response = await this.#request({ kind: "configure-course", generation, definition: transferred }, [
      transferred,
    ])
    if (response.kind !== "course-configured" || response.generation !== generation) {
      throw new Tf2WorkerError("WorkerFailed")
    }
  }

  async load(
    generation: number,
    bsp: Uint8Array,
    profile: 0 | 1,
    configuration: Uint8Array,
    derivedKey: string,
  ): Promise<LoadedGame> {
    const staged = await this.stage(generation, bsp, profile, configuration, derivedKey)
    try {
      await this.activate(generation)
      return staged
    } catch (error) {
      try {
        await this.discard(generation)
      } catch {
        // Activation failure remains the authoritative result when the worker cannot discard.
      }
      throw error
    }
  }

  async observe(generation: number, nowSeconds: number, command: ArrayBuffer, suspended = false): Promise<readonly SimulationPublication[]> {
    if (command.byteLength < 48 || command.byteLength > 64 * 1024) throw new Tf2WorkerError("BoundExceeded")
    if (!Number.isFinite(nowSeconds) || nowSeconds < 0) throw new Tf2WorkerError("BoundExceeded")
    const transferred = command.slice(0)
    const response = await this.#request({ kind: "observe", generation, nowSeconds, suspended, command: transferred }, [transferred])
    if (response.kind !== "simulation" || response.generation !== generation || !(response.output instanceof ArrayBuffer)) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    return decodeSimulationPublications(response.output)
  }
  async particles(generation: number, batch: Uint8Array): Promise<Uint8Array> {
    if (batch.byteLength < 32 || batch.byteLength > 4 * 1024 * 1024) throw new Tf2WorkerError("BoundExceeded")
    const transferred = batch.slice().buffer
    const response = await this.#request({ kind: "particles", generation, batch: transferred }, [transferred])
    if (
      response.kind !== "particles" ||
      response.generation !== generation ||
      !(response.output instanceof ArrayBuffer)
    )
      throw new Tf2WorkerError("WorkerFailed")
    return new Uint8Array(response.output)
  }
  async models(generation: number, batch: Uint8Array): Promise<Uint8Array> {
    if (batch.byteLength < 12 || batch.byteLength > 1024 * 1024) throw new Tf2WorkerError("BoundExceeded")
    const transferred = batch.slice().buffer
    const response = await this.#request({ kind: "models", generation, batch: transferred }, [transferred])
    if (response.kind !== "models" || response.generation !== generation || !(response.output instanceof ArrayBuffer)) {
      throw new Tf2WorkerError("WorkerFailed")
    }
    return new Uint8Array(response.output)
  }
  async visibility(
    generation: number,
    input: Readonly<{ position: readonly [number, number, number]; yawDegrees:number; pitchDegrees:number; verticalFovDegrees:number; aspectRatio:number; near:number; presentationTimeSeconds:number }>,
  ): Promise<VisibilityResult> {
    const response = await this.#request({ kind: "visibility", generation, view: input })
    if (
      response.kind !== "visibility" ||
      response.generation !== generation ||
      !(response.output instanceof ArrayBuffer)
    )
      throw new Tf2WorkerError("WorkerFailed")
    const bytes = new Uint8Array(response.output), view = new DataView(response.output), decoder=new TextDecoder("utf-8",{fatal:true})
    if (decoder.decode(bytes.subarray(0, 4)) !== "PVIS" || view.getUint32(4, true) !== 2)
      throw new Tf2WorkerError("WorkerFailed")
    let at=76
    const require=(length:number)=>{if(at+length>bytes.length)throw new Tf2WorkerError("WorkerFailed")},u8=()=>{require(1);return bytes[at++]!},u32=()=>{require(4);const value=view.getUint32(at,true);at+=4;return value},i32=()=>{require(4);const value=view.getInt32(at,true);at+=4;return value},f32=()=>{require(4);const value=view.getFloat32(at,true);at+=4;if(!Number.isFinite(value))throw new Tf2WorkerError("WorkerFailed");return value},text=()=>{const length=u32();require(length);const value=decoder.decode(bytes.subarray(at,at+length));at+=length;return value},vector=()=>Object.freeze([f32(),f32(),f32()]) as readonly[number,number,number]
    const count = u32(), surfaces=new Uint32Array(count);for(let index=0;index<count;index++)surfaces[index]=u32()
    const eyeLeafValue=u32(),leaves=Object.freeze(Array.from({length:u32()},u32)),areas=Object.freeze(Array.from({length:u32()},u32))
    const present=u8(),cheap=u8(),reflect=u8(),refract=u8(),reflectEntities=u8(),drawSurface=u8(),opaque=u8(),nearPlaneIntersects=u8()
    if([present,cheap,reflect,refract,reflectEntities,drawSurface,opaque,nearPlaneIntersects].some(value=>value>1))throw new Tf2WorkerError("WorkerFailed")
    let visibleWater:WaterViewPlan["visibleWater"]=null
    if(present===1){const volume=u32(),visibleLeaf=u32(),eyeLeaf=u32(),eyeInVolume=u8(),translucent=u8();if(eyeInVolume>1||translucent>1||u8()||u8())throw new Tf2WorkerError("WorkerFailed");const surfaceZ=f32(),distance=u32(),material=text(),normalFrame=i32(),normalTransform=new Float32Array(16);for(let index=0;index<16;index++)normalTransform[index]=f32();visibleWater=Object.freeze({volume,visibleLeaf,eyeLeaf,eyeInVolume:eyeInVolume===1,surfaceZ,distanceToWater:distance===0xffff?null:distance,material,translucent:translucent===1,evaluated:Object.freeze({normalFrame,normalTransform,cheapStart:f32(),cheapEnd:f32()})})}
    const passes:WaterViewPass[]=[]
    for(let passCount=u32();passCount>0;passCount--){const kind=u8(),renderAboveWater=u8(),renderUnderWater=u8(),renderWaterSurface=u8(),drawEntities=u8(),drawSky2d=u8(),hasClip=u8(),keep=u8();if(kind>3||[renderAboveWater,renderUnderWater,renderWaterSurface,drawEntities,drawSky2d,hasClip].some(value=>value>1)||keep>2||(hasClip===0)!==(keep===0))throw new Tf2WorkerError("WorkerFailed");const origin=vector(),angles=vector(),clipHeight=f32(),forced=u32(),fogKind=u8(),heightFog=u8();if(fogKind>1||heightFog>1||u8()||u8())throw new Tf2WorkerError("WorkerFailed");const fogVolume=u32();if(fogKind===0&&(heightFog!==0||fogVolume!==0))throw new Tf2WorkerError("WorkerFailed");const passSurfaces=new Uint32Array(u32());for(let index=0;index<passSurfaces.length;index++)passSurfaces[index]=u32();passes.push(Object.freeze({kind:(["reflection","refraction","main","intersection"] as const)[kind]!,origin,angles,renderAboveWater:renderAboveWater===1,renderUnderWater:renderUnderWater===1,renderWaterSurface:renderWaterSurface===1,drawEntities:drawEntities===1,drawSky2d:drawSky2d===1,clip:hasClip===1?Object.freeze({height:clipHeight,keep:keep===1?"above" as const:"below" as const}):null,forcedVisibilityLeaf:forced===0xffff_ffff?null:forced,fog:fogKind===0?Object.freeze({kind:"world" as const}):Object.freeze({kind:"water" as const,volume:fogVolume,heightFog:heightFog===1}),surfaces:passSurfaces}))}
    if(at!==bytes.length||(present===0&&visibleWater!==null))throw new Tf2WorkerError("WorkerFailed")
    const hex = (values: Uint8Array) => Array.from(values, (value) => value.toString(16).padStart(2, "0")).join("")
    return Object.freeze({
      cacheIdentity: hex(bytes.subarray(8, 40)),
      worldIdentity: hex(bytes.subarray(40, 72)),
      outsideWorld:bytes[72]===1,
      sky:bytes[73] as 0|1|2,
      eyeLeaf:eyeLeafValue===0xffff_ffff?null:eyeLeafValue,
      leaves,areas,surfaces,
      water:Object.freeze({visibleWater,render:Object.freeze({cheap:cheap===1,reflect:reflect===1,refract:refract===1,reflectEntities:reflectEntities===1,drawSurface:drawSurface===1,opaque:opaque===1}),nearPlaneIntersects:nearPlaneIntersects===1,passes:Object.freeze(passes)}),
    })
  }

  async shutdown(): Promise<void> {
    if (this.#closed) return
    try {
      const response = await this.#request({ kind: "shutdown" })
      if (response.kind !== "shutdown") throw new Tf2WorkerError("WorkerFailed")
    } finally {
      this.#closed = true
      this.#worker.removeEventListener("message", this.#message)
      this.#worker.removeEventListener("error", this.#error)
      this.#worker.terminate()
      this.#failAll(new Tf2WorkerError("Closed"))
    }
  }
}

function equalBytes(a: Uint8Array,b:Uint8Array){return a.length===b.length&&a.every((v,i)=>v===b[i])}
export function mergePublicationSnapshots(snapshots: readonly Snapshot[]): Snapshot {
  const final = snapshots.at(-1)
  if (!final) throw new Tf2WorkerError("WorkerFailed")
  const all = (key: keyof Snapshot) => Object.freeze(snapshots.flatMap((snapshot) => snapshot[key] as readonly unknown[]))
  const projectileTimeline = Object.freeze(snapshots.flatMap((snapshot) => snapshot.projectileTimeline))
  if (
    projectileTimeline.length === 0
    || projectileTimeline.at(-1)?.tick !== final.tick
    || projectileTimeline.some((entry, index) => index > 0 && entry.tick <= projectileTimeline[index - 1]!.tick)
  ) {
    throw new Tf2WorkerError("WorkerFailed")
  }
  return Object.freeze({
    ...final,
    projectileEvents: all("projectileEvents"),
    projectileTimeline,
    entityEvents: all("entityEvents"),
    events: all("events"),
    activities: all("activities"),
    lifecycleEvents: all("lifecycleEvents"),
    physicsRequests: all("physicsRequests"),
    rocketTraceRequests: all("rocketTraceRequests"),
    radiusDamageRequests: all("radiusDamageRequests"),
    moverRequests: all("moverRequests"),
    contactReconcileRequests: all("contactReconcileRequests"),
    mapEffects: all("mapEffects"),
    regenerateAnimationEvents: all("regenerateAnimationEvents"),
    randomDraws: all("randomDraws"),
    audioEvents: all("audioEvents"),
    rocketTraceResults: all("rocketTraceResults"),
    moverResults: all("moverResults"),
  }) as Snapshot
}
function decodeSimulationPublications(bytes:ArrayBuffer):readonly SimulationPublication[]{
  const data=new Uint8Array(bytes),view=new DataView(bytes); if(bytes.byteLength<16||new TextDecoder().decode(data.subarray(0,4))!=="PSIM"||view.getUint32(4,true)!==1||view.getUint32(12,true)!==0)throw new Tf2WorkerError("WorkerFailed")
  const count=view.getUint32(8,true);if(count>256)throw new Tf2WorkerError("BoundExceeded");let at=16;const output:SimulationPublication[]=[]
  const require=(n:number)=>{if(at+n>bytes.byteLength)throw new Tf2WorkerError("WorkerFailed")}
  for(let i=0;i<count;i++){require(40);const hostFrame=view.getBigUint64(at,true),first=view.getBigUint64(at+8,true),last=view.getBigUint64(at+16,true),selected=view.getUint32(at+24,true),interpolation=view.getFloat32(at+28,true),sl=view.getUint32(at+32,true),ec=view.getUint32(at+36,true);at+=40;if(selected<1||ec!==selected||last-first+1n!==BigInt(selected))throw new Tf2WorkerError("WorkerFailed");require(sl);const snapshotBytes=data.slice(at,at+sl);at+=sl;const eventBatches:SimulationEventBatch[]=[];for(let e=0;e<ec;e++){require(12);const hostTick=view.getBigUint64(at,true),l=view.getUint32(at+8,true);at+=12;require(l);const value=data.slice(at,at+l);at+=l;eventBatches.push(Object.freeze({hostTick,bytes:value,snapshot:decodeSnapshot(value.slice().buffer)}))}if(!equalBytes(eventBatches.at(-1)!.bytes,snapshotBytes))throw new Tf2WorkerError("WorkerFailed");output.push(Object.freeze({hostFrame,firstHostTick:first,lastHostTick:last,selectedTicks:selected,interpolation,snapshotBytes,eventBatches:Object.freeze(eventBatches),snapshot:mergePublicationSnapshots(eventBatches.map(e=>e.snapshot))}))}
  if(at!==bytes.byteLength)throw new Tf2WorkerError("WorkerFailed");return Object.freeze(output)
}
