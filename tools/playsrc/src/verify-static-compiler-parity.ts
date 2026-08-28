import { readFile, writeFile } from "node:fs/promises"
import path from "node:path"
import { loadLocalConfig } from "./config"
import { createStaticCompilerParityOwner, THREE, TSL } from "../../../packages/presentation/rendering/tests/fixtures/static-compiler-parity"
import { sourceTextureLayout } from "../../../packages/presentation/rendering/src/source-texture-layout"
import { swizzleModelTexture } from "../../../packages/presentation/rendering/src/model-material-graphs"

const digest=(bytes:string|Uint8Array)=>new Bun.CryptoHasher("sha256").update(bytes).digest("hex")
const require=(value:unknown,message:string)=>{if(!value)throw new Error(message)}
const [file]=process.argv.slice(2),{sourceCacheDir}=await loadLocalConfig()
require(file&&process.argv.length===3&&path.resolve(file).startsWith(path.resolve(sourceCacheDir)+path.sep),"Expected configured-cache compiler fixture")
const bytes=await readFile(file!),encoded=JSON.parse(bytes.toString()),arena=await readFile(path.join(path.dirname(file!),encoded.arena.file))
require(path.basename(file!)===`${digest(bytes)}.json`&&digest(arena)===encoded.arena.sha256&&arena.length===encoded.arena.byteLength,"Fixture identity differs")
const types={Float32Array,Uint32Array,Uint16Array,Uint8Array,Int32Array,Float64Array}
const fixture=JSON.parse(JSON.stringify(encoded.fixture),(_,v)=>{if(v?.bigInt!==undefined)return BigInt(v.bigInt);if(!v?.arrayType)return v
  const Type=types[v.arrayType as keyof typeof types];require(Type&&v.byteOffset>=0&&v.byteOffset+v.byteLength<=arena.length,"Fixture array bounds")
  return new Type(arena.buffer,arena.byteOffset+v.byteOffset,v.byteLength/Type.BYTES_PER_ELEMENT)})
require(fixture.contentBuild==="24245096"&&fixture.staticProps,"Exact configured static inputs required")
const props=fixture.staticProps,models=new Map<string,any>(fixture.geometry.map((m:any)=>[m.logicalPath,m])),headers=new Map<string,any>(fixture.models)
const materials=new Map<string,any>(fixture.materials),states=new Map<string,any>(fixture.materialStates),textures=new Map<string,any>(fixture.textures),samples=new Map<string,any>(),owned:THREE.Texture[]=[]
const owner=createStaticCompilerParityOwner(),excluded:any[]=[],geometries:THREE.BufferGeometry[]=[],started=performance.now()
for(let index=0;index<props.count;index++){
  if(props.lightingKind[index]!==0)continue
  const modelName=props.models[props.presentationModel[index]],key=`${modelName}${props.skin[index]?`#skin=${props.skin[index]}`:""}`,model=models.get(key)
  require(model,`Static model unavailable:${key}`)
  const vhv=props.vhv[props.vhvObjects[index*2+fixture.profile]],colors=vhv.meshes.filter((mesh:any)=>mesh.lod===props.lod[index])
  let colorIndex=0
  for(const [primitiveIndex,primitive] of model.primitives.entries()){
    const identity=model.materials[primitive.material].logicalPath.toLowerCase(),material=materials.get(identity),state=states.get(identity)
    if(state.noDraw)continue
    const color=colors[colorIndex++],label=`${props.source[index]}:${key}:${primitiveIndex}:${identity}`
    if(!["vertex-lit-generic","unlit-generic"].includes(material.shader)||material.state.colorModulation?.some((v:number)=>v!==1)){excluded.push(label);continue}
    require(color.vertexCount===primitive.positions.length/3&&color.colors.length===color.vertexCount*4,`VHV order differs:${label}`)
    const binding=material.bindings.find((binding:any)=>binding.kind==="material"&&binding.role===0),input=textures.get(binding.logicalPath)
    const sampleKey=`${input.sourceSha256}:${binding.colorRead}:${input.sourceFormat}`
    let base=samples.get(sampleKey)
    if(!base){const layout=sourceTextureLayout(input.sourceFormat,input.scalarEncoding)!,texture=layout.compressed===null?new THREE.DataTexture(null,input.width,input.height):new THREE.CompressedTexture([],input.width,input.height,layout.compressed,layout.type)
      texture.type=layout.type;texture.format=layout.format;texture.colorSpace=binding.colorRead==="srgb"?THREE.SRGBColorSpace:THREE.NoColorSpace;texture.flipY=false;owned.push(texture)
      base=swizzleModelTexture(TSL.texture(texture,TSL.uv()),input.sourceFormat);samples.set(sampleKey,base)}
    const geometry=new THREE.BufferGeometry();geometry.setAttribute("position",new THREE.BufferAttribute(primitive.positions,3));geometry.setAttribute("normal",new THREE.BufferAttribute(primitive.normals,3));geometry.setAttribute("uv",new THREE.BufferAttribute(primitive.uv,2));geometry.setAttribute("staticLighting",new THREE.Uint8BufferAttribute(color.colors,4,true));geometry.setIndex(new THREE.BufferAttribute(primitive.indices,1));geometries.push(geometry)
    owner.admit(label,`${key}:${primitiveIndex}`,geometry,base,state,material.shader==="unlit-generic",(props.flags[index]&1)!==0,headers.get(modelName).descriptor.frontFace==="clockwise"?THREE.BackSide:THREE.FrontSide)
  }
}
const result=owner.finish();require(result.draws>0,"No actual VHV draws")
for(const geometry of geometries)geometry.dispose();for(const texture of owned)texture.dispose()
const report={input:path.basename(file!),provenance:fixture.provenance,...result,records:result.records.map(record=>({...record,vertex:digest(record.vertex),fragment:digest(record.fragment)})),excluded,milliseconds:performance.now()-started,pixelsVerified:false}
const output=path.join(path.dirname(file!),`${digest(JSON.stringify(report))}.static-parity.json`);await writeFile(output,JSON.stringify(report,null,2))
console.log(JSON.stringify({output,draws:result.draws,dedicatedCompilerStates:result.dedicatedCompilerStates,sharedCompilerStates:result.sharedCompilerStates,excluded:excluded.length,milliseconds:report.milliseconds}))
