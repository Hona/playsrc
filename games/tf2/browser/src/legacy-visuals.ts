import type { LegacyVisualFrame,LegacyVisualFrameSet,LegacyViewKind, PixelVisibilityFeedback } from "@playsrc/rendering"
import type { VisibilityView } from "./protocol"

export type LegacyVisualView = VisibilityView & Readonly<{kind:LegacyViewKind;viewportHeight:number;pixelVisibility:readonly PixelVisibilityFeedback[] }>

export function encodeLegacyVisualQuery(views:readonly LegacyVisualView[]):Uint8Array {
  if(views.length<1||views.length>5||!views.some(view=>view.kind===0)||new Set(views.map(view=>view.kind)).size!==views.length)throw new Error("Legacy visual view count")
  const length=12+views.reduce((sum,view)=>sum+64+view.pixelVisibility.length*20,0)
  if(length>4*1024*1024)throw new Error("Legacy visual query bound")
  const bytes=new Uint8Array(length),output=new DataView(bytes.buffer);let at=0
  const u32=(value:number)=>{if(!Number.isSafeInteger(value)||value<0||value>0xffffffff)throw new Error("Legacy visual integer");output.setUint32(at,value,true);at+=4}
  const f32=(value:number)=>{if(!Number.isFinite(value))throw new Error("Legacy visual scalar");output.setFloat32(at,value,true);at+=4}
  bytes.set(new TextEncoder().encode("PLVQ"));at=4;u32(2);u32(views.length)
  views.forEach((view,index)=>{
    if(view.position.length!==3||(view.visibilityPosition??view.position).length!==3||view.viewportHeight<1||view.viewportHeight>32768||view.pixelVisibility.length>65536||view.presentationTimeSeconds<0
      ||view.verticalFovDegrees<=0||view.verticalFovDegrees>=180||view.aspectRatio<=0||view.near<=0||view.far<=view.near)throw new Error("Legacy visual view")
    if(view.kind<0||view.kind>4)throw new Error("Legacy visual view kind")
    u32(view.kind);f32(view.presentationTimeSeconds);u32(view.viewportHeight);u32(view.pixelVisibility.length)
    for(const value of [...view.position,...(view.visibilityPosition??view.position),view.yawDegrees,view.pitchDegrees,view.verticalFovDegrees,view.aspectRatio,view.near,view.far])f32(value)
    for(const value of view.pixelVisibility){
      if(value.submission<1||!Number.isSafeInteger(value.visible)||!Number.isSafeInteger(value.possible)||value.visible < -1||value.possible < -1||value.visible>0x7fffffff||value.possible>0x7fffffff||value.clipFraction<0||value.clipFraction>1)throw new Error("Legacy raster feedback")
      u32(value.source);u32(value.submission);u32(value.visible>>>0);u32(value.possible>>>0);f32(value.clipFraction)
    }
  })
  return bytes
}

class Reader {
  at=0
  readonly view:DataView
  constructor(readonly bytes:Uint8Array){this.view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength)}
  take(count:number):Uint8Array{if(!Number.isSafeInteger(count)||count<0||this.at+count>this.bytes.length)throw new Error("Truncated legacy visuals");const result=this.bytes.subarray(this.at,this.at+count);this.at+=count;return result}
  u32():number{this.take(4);return this.view.getUint32(this.at-4,true)}
  f32():number{this.take(4);const value=this.view.getFloat32(this.at-4,true);if(!Number.isFinite(value))throw new Error("Nonfinite legacy visual");return value}
  floats(count:number):Float32Array{const values=new Float32Array(count);for(let i=0;i<count;i++)values[i]=this.f32();return values}
  header(version:number):void{if(new TextDecoder().decode(this.take(4))!=="PLVF"||this.u32()!==version)throw new Error("Legacy visual identity")}
  end():void{if(this.at!==this.bytes.length)throw new Error("Trailing legacy visual bytes")}
}

export function decodeLegacyVisualViews(bytes:Uint8Array):LegacyVisualFrameSet {
  if(bytes.byteLength>4*1024*1024)throw new Error("Legacy visual output bound")
  const packet=new Reader(bytes);packet.header(5)
  const count=packet.u32();if(count<1||count>5)throw new Error("Legacy visual view count")
  const views:Partial<Record<LegacyViewKind,LegacyVisualFrame>>={}
  for(let index=0;index<count;index++){
    const kind=packet.u32() as LegacyViewKind
    if(kind>4||views[kind])throw new Error("Legacy visual view kind")
    const frame=new Reader(packet.take(packet.u32()));frame.header(7)
    const proxyCount=frame.u32(),quadCount=frame.u32()
    if(proxyCount>65536||quadCount>65536||proxyCount*88+quadCount*176!==frame.bytes.length-frame.at)throw new Error("Legacy visual record count")
    const proxies=Object.freeze(Array.from({length:proxyCount},()=>{
      const source=frame.u32(),clipFraction=frame.f32(),vertices=frame.floats(20)
      if(clipFraction<0||clipFraction>1)throw new Error("Legacy proxy clipping")
      return Object.freeze({source,clipFraction,vertices})
    }))
    const quads=Object.freeze(Array.from({length:quadCount},()=>{
      const source=frame.u32(),material=frame.u32(),selectedFrame=frame.u32(),layer=frame.u32()
      if(layer>2)throw new Error("Legacy visual render layer")
      return Object.freeze({source,material,frame:selectedFrame,layer:layer as 0|1|2,hdrScale:frame.f32(),origin:frame.floats(3),positions:frame.floats(12),color:frame.floats(16),uv:frame.floats(8)})
    }))
    frame.end();views[kind]=Object.freeze({proxies,quads})
  }
  if(!views[0])throw new Error("Missing legacy main view")
  packet.end();return Object.freeze(views)
}
