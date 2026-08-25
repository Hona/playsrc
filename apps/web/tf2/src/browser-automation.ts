import { SOURCE_CONSOLE_CEILINGS } from "@playsrc/vgui"

const CONSOLE_INPUT = "[aria-label='Console command']"
const WORLD_CANVAS = ".world-canvas"

export type Tf2BrowserAutomationTransport = Readonly<{
  evaluate<T>(expression: string): Promise<T>
  press(key: string): Promise<void>
  click(selector: string): Promise<void>
  focus(selector: string): Promise<void>
  fill(selector: string, value: string): Promise<void>
  waitFor(expression: string, timeoutMilliseconds: number): Promise<void>
  activateCurrentTab(): Promise<void>
}>

export type Tf2PointerLockEvidence = Readonly<{
  mode: "native" | "emulated"
  native: "available" | "unavailable"
  unavailableReason?: string
}>

export type Tf2MovementEvidence = Readonly<{
  firstTick: number
  lastTick: number
  before: readonly [number, number, number]
  after: readonly [number, number, number]
  distance: number
}>

export type Tf2CameraEvidence = Readonly<{
  position: readonly [number, number, number]
  yaw: number
  pitch: number
  verticalFov: number
  near: number
  far: number
}>

type PointerState = Readonly<{
  locked: boolean
  lockOwnerMatches: boolean
  focused: boolean
  detail: string
  gameUi: string
  mode: "native" | "emulated"
}>

function boundedCommand(command: string): string {
  if (
    command.length === 0
    || /[\r\n\0]/u.test(command)
    || new TextEncoder().encode(command).byteLength > SOURCE_CONSOLE_CEILINGS.maxInputUtf8Bytes
  ) throw new TypeError("TF2 automation console command is invalid")
  return command
}

function boundedMap(identity: string): string {
  if (!/^[a-z0-9][a-z0-9_-]{0,254}$/u.test(identity)) {
    throw new TypeError("TF2 automation map identity is invalid")
  }
  return identity
}

function pointerAdapterScript(reason: string): string {
  return `((reason)=>{
    if(globalThis.__playsrcBrowserTestPointer?.mode==="emulated")return true;
    let owner=null;
    const nativeRequest=Element.prototype.requestPointerLock;
    const nativeExit=document.exitPointerLock.bind(document);
    Object.defineProperty(document,"pointerLockElement",{configurable:true,get:()=>owner});
    Object.defineProperty(Element.prototype,"requestPointerLock",{configurable:true,value:function(options){
      const canvas=document.querySelector("canvas.world-canvas");
      if(this!==canvas||!this.isConnected)return Promise.reject(new DOMException("TF2 test pointer target is not attached","WrongDocumentError"));
      owner=this;
      queueMicrotask(()=>document.dispatchEvent(new Event("pointerlockchange")));
      return Promise.resolve();
    }});
    Object.defineProperty(document,"exitPointerLock",{configurable:true,value:function(){
      if(owner===null)return Promise.resolve();
      owner=null;
      queueMicrotask(()=>document.dispatchEvent(new Event("pointerlockchange")));
      return Promise.resolve();
    }});
    Object.defineProperty(globalThis,"__playsrcBrowserTestPointer",{configurable:true,value:Object.freeze({mode:"emulated",native:"unavailable",reason,nativeRequest,nativeExit})});
    return true;
  })(${JSON.stringify(reason)})`
}

const pointerStateExpression = `(()=>{
  const main=document.querySelector("main"),canvas=document.querySelector("canvas.world-canvas");
  return{
    locked:main?.dataset.pointerLocked==="true",
    lockOwnerMatches:document.pointerLockElement===canvas,
    focused:document.hasFocus(),
    detail:main?.dataset.detail??"",
    gameUi:main?.dataset.gameui??"",
    mode:globalThis.__playsrcBrowserTestPointer?.mode??"native"
  };
})()`

export class Tf2BrowserAutomation {
  readonly #transport: Tf2BrowserAutomationTransport
  #nativeUnavailableReason?: string

  readonly console = Object.freeze({
    submitCommand: async (command: string): Promise<void> => {
      const text = boundedCommand(command)
      const visible = await this.#transport.evaluate<boolean>(
        `(()=>{const value=document.querySelector(".developer-layer [data-vgui-service=developer-console] [role=dialog]");return !!value&&getComputedStyle(value).display!=="none"})()`,
      )
      if (!visible) await this.#transport.press("Backquote")
      await this.#transport.click(CONSOLE_INPUT)
      await this.#transport.waitFor("document.activeElement?.getAttribute('aria-label')==='Console command'", 30_000)
      await this.#transport.fill(CONSOLE_INPUT, text)
      await this.#transport.press("Enter")
    },
  })

  readonly maps = Object.freeze({
    load: async (identity: string): Promise<void> => {
      await this.console.submitCommand(`map ${boundedMap(identity)}`)
    },
  })

  readonly pointer = Object.freeze({
    capture: async (identity: string): Promise<Tf2PointerLockEvidence> => {
      if (!identity || /[\r\n\0]/u.test(identity)) throw new TypeError("TF2 automation pointer identity is invalid")
      let state: PointerState | undefined
      for (let attempt = 0; attempt < 10; attempt += 1) {
        await this.#transport.activateCurrentTab()
        await this.#transport.evaluate<boolean>(
          "(()=>{window.focus();document.querySelector('.world-canvas')?.focus();return document.hasFocus()})()",
        )
        await this.#transport.focus(WORLD_CANVAS)
        await this.#transport.click(WORLD_CANVAS)
        await this.#transport.waitFor(
          "(()=>{const value=document.querySelector('main')?.dataset;return value?.pointerLocked==='true'||value?.detail?.startsWith('Pointer lock failed:')})()",
          10_000,
        )
        state = await this.#transport.evaluate<PointerState>(pointerStateExpression)
        if (state.locked && state.lockOwnerMatches) {
          return Object.freeze(state.mode === "emulated"
            ? { mode: "emulated", native: "unavailable", unavailableReason: this.#nativeUnavailableReason! }
            : { mode: "native", native: "available" })
        }
        if (state.detail.startsWith("Pointer lock failed: WrongDocumentError:")) {
          this.#nativeUnavailableReason = state.detail
          await this.#transport.evaluate<boolean>(pointerAdapterScript(state.detail))
          continue
        }
        if (state.gameUi === "pause") {
          await this.#transport.click("[data-vgui-name=ResumeButton]")
          await this.#transport.waitFor("document.querySelector('main').dataset.gameui==='in-game'", 30_000)
        }
      }
      throw new Error(`TF2 ${identity} pointer capture failed: ${JSON.stringify(state ?? null)}`)
    },
    release: async (): Promise<void> => {
      await this.#transport.evaluate<boolean>("Promise.resolve(document.exitPointerLock()).then(()=>true)")
      await this.#transport.waitFor("document.pointerLockElement===null", 10_000)
    },
  })

  readonly player = Object.freeze({
    selectClass: async (identity: "soldier" | "demoman"): Promise<void> => {
      if (identity !== "soldier" && identity !== "demoman") throw new TypeError("TF2 automation class is invalid")
      await this.console.submitCommand(`class ${identity}`)
    },
    lookBy: async (movement: Readonly<{ x: number; y: number }>): Promise<Tf2CameraEvidence> => {
      if (!Number.isFinite(movement.x) || !Number.isFinite(movement.y) || (movement.x === 0 && movement.y === 0)) {
        throw new TypeError("TF2 automation pointer movement is invalid")
      }
      await this.#ensureCapturedPointer("look")
      const camera = await this.#transport.evaluate<{
        position: number[]; yaw: number; pitch: number; verticalFov: number; near: number; far: number
      }>(`new Promise((resolve,reject)=>{
        const root=document.querySelector("main"),canvas=document.querySelector("canvas.world-canvas");
        if(!root||!canvas){reject(new Error("TF2 automation view is unavailable"));return;}
        const start=Number(canvas.dataset.displayMouseRevision);
        const observer=new MutationObserver(()=>{
          if(Number(canvas.dataset.displayMouseRevision)<=start)return;
          clearTimeout(deadline);observer.disconnect();
          const value=root.dataset;
          resolve({position:value.cameraPosition.split(",").map(Number),yaw:Number(value.cameraYaw),pitch:Number(value.cameraPitch),verticalFov:Number(value.cameraVerticalFov),near:Number(value.cameraNear),far:Number(value.cameraFar)});
        });
        const deadline=setTimeout(()=>{observer.disconnect();reject(new Error("TF2 automation pointer movement was not displayed"));},10000);
        observer.observe(canvas,{attributes:true,attributeFilter:["data-display-frame"]});
        const event=new MouseEvent("mousemove",{bubbles:true});
        Object.defineProperties(event,{movementX:{value:${movement.x}},movementY:{value:${movement.y}}});
        window.dispatchEvent(event);
      })`)
      if (
        camera.position.length !== 3 || !camera.position.every(Number.isFinite)
        || ![camera.yaw, camera.pitch, camera.verticalFov, camera.near, camera.far].every(Number.isFinite)
      ) throw new Error("TF2 automation displayed camera is invalid")
      return Object.freeze({ ...camera, position: Object.freeze([...camera.position]) as readonly [number, number, number] })
    },
    pressPrimaryFire: async (): Promise<void> => {
      await this.#ensureCapturedPointer("primary fire")
      await this.#transport.evaluate<boolean>(
        "(()=>{window.dispatchEvent(new MouseEvent('mousedown',{button:0,buttons:1,bubbles:true}));return true})()",
      )
    },
    releasePrimaryFire: async (): Promise<void> => {
      await this.#transport.evaluate<boolean>(
        "(()=>{window.dispatchEvent(new MouseEvent('mouseup',{button:0,buttons:0,bubbles:true}));return true})()",
      )
    },
    jump: async (): Promise<void> => {
      await this.#transport.evaluate<boolean>(
        "(()=>{window.dispatchEvent(new KeyboardEvent('keydown',{code:'Space',key:' ',bubbles:true}));window.dispatchEvent(new KeyboardEvent('keyup',{code:'Space',key:' ',bubbles:true}));return true})()",
      )
    },
    settle: async (minimumTicks: number): Promise<Readonly<{ tick: number; position: readonly [number, number, number] }>> => {
      if (!Number.isSafeInteger(minimumTicks) || minimumTicks < 1 || minimumTicks > 256) {
        throw new TypeError("TF2 automation stationary tick bound is invalid")
      }
      const state = await this.#transport.evaluate<{ tick: number; position: number[] }>(`new Promise((resolve,reject)=>{
        const root=document.querySelector("main");
        if(!root){reject(new Error("TF2 automation player root is unavailable"));return;}
        let previousTick=Number(root.dataset.snapshotTick),previousPosition=root.dataset.cameraPosition,stableTicks=0;
        const observer=new MutationObserver(()=>{
          const tick=Number(root.dataset.snapshotTick),position=root.dataset.cameraPosition;
          if(!Number.isSafeInteger(tick)||tick<=previousTick)return;
          if(root.dataset.grounded==="true"&&Number(root.dataset.wishSpeed)===0&&position===previousPosition)stableTicks+=tick-previousTick;
          else stableTicks=0;
          previousTick=tick;previousPosition=position;
          if(stableTicks>=${minimumTicks}){clearTimeout(deadline);observer.disconnect();resolve({tick,position:position.split(",").map(Number)});}
        });
        const deadline=setTimeout(()=>{observer.disconnect();reject(new Error("TF2 automation player did not settle"));},30000);
        observer.observe(root,{attributes:true,attributeFilter:["data-snapshot-tick","data-camera-position","data-wish-speed","data-grounded"]});
      })`)
      if (!Number.isSafeInteger(state.tick) || state.position.length !== 3 || !state.position.every(Number.isFinite)) {
        throw new Error("TF2 automation stationary observation is invalid")
      }
      return Object.freeze({ tick: state.tick, position: Object.freeze([...state.position]) as readonly [number, number, number] })
    },
    walkForward: async (minimumTicks: number): Promise<Tf2MovementEvidence> => {
      if (!Number.isSafeInteger(minimumTicks) || minimumTicks < 1 || minimumTicks > 256) {
        throw new TypeError("TF2 automation movement tick bound is invalid")
      }
      const observation = "(()=>{const value=document.querySelector('main').dataset;return{tick:Number(value.snapshotTick),position:value.cameraPosition.split(',').map(Number)}})()"
      const before = await this.#transport.evaluate<{ tick: number; position: number[] }>(observation)
      if (!Number.isSafeInteger(before.tick) || before.position.length !== 3 || !before.position.every(Number.isFinite)) {
        throw new Error("TF2 automation initial movement observation is invalid")
      }
      try {
        await this.#transport.evaluate<boolean>(
          "(()=>{window.dispatchEvent(new KeyboardEvent('keydown',{code:'KeyW',key:'w',bubbles:true}));return true})()",
        )
        await this.#transport.waitFor(
          `Number(document.querySelector('main').dataset.snapshotTick)>=${before.tick + minimumTicks}&&Number(document.querySelector('main').dataset.wishSpeed)>0`,
          30_000,
        )
      } finally {
        await this.#transport.evaluate<boolean>(
          "(()=>{window.dispatchEvent(new KeyboardEvent('keyup',{code:'KeyW',key:'w',bubbles:true}));return true})()",
        )
      }
      const after = await this.#transport.evaluate<{ tick: number; position: number[] }>(observation)
      if (!Number.isSafeInteger(after.tick) || after.position.length !== 3 || !after.position.every(Number.isFinite)) {
        throw new Error("TF2 automation terminal movement observation is invalid")
      }
      return Object.freeze({
        firstTick: before.tick,
        lastTick: after.tick,
        before: Object.freeze([...before.position]) as readonly [number, number, number],
        after: Object.freeze([...after.position]) as readonly [number, number, number],
        distance: Math.hypot(...after.position.map((value, index) => value - before.position[index]!)),
      })
    },
  })

  async #ensureCapturedPointer(identity: string): Promise<void> {
    const state = await this.#transport.evaluate<PointerState>(pointerStateExpression)
    if (!state.locked || !state.lockOwnerMatches) await this.pointer.capture(identity)
  }

  constructor(transport: Tf2BrowserAutomationTransport) {
    this.#transport = transport
  }
}
