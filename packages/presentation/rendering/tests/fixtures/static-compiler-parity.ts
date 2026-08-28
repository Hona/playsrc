import * as THREE from "three/webgpu"
import * as TSL from "three/tsl"
import NodeManager from "three/src/renderers/common/nodes/NodeManager.js"
import RenderObjects from "three/src/renderers/common/RenderObjects.js"
import NodeFrame from "three/src/nodes/core/NodeFrame.js"
import { StaticMaterialGraphs } from "../../src/static-material-graphs"
import { ModelLightingGraphs, bindStaticPropFade } from "../../src/model-lighting-graphs"
import { sourceFragmentColor } from "../../src/source-fragment-color"
import { sourceStaticVertexLightingNode } from "../../src/source-model-lighting"
import { createSourceWaterFogUniforms } from "../../src/source-water"
import { installRenderObjectLifetime } from "../../src/render-object-lifetime"
import { prepareReachablePipelineVisibility, pipelinePreparationIdentity } from "../../src/reachable-pipeline-visibility"
import { createStaticPropFadeVariant } from "../../src/static-prop-fade"

export function createStaticCompilerParityOwner() {
  const equal=(a:unknown,b:unknown,label:string)=>{if(JSON.stringify(a)!==JSON.stringify(b))throw new Error(label)}
  const renderer=new THREE.WebGPURenderer({canvas:{width:1,height:1,style:{},addEventListener(){}} as any});renderer.hasFeature=()=>false
  const nodes=new NodeManager(renderer,{createNodeBuilder:(mesh:THREE.Mesh)=>new THREE.WGSLNodeBuilder(mesh,renderer)})
  const manager=new RenderObjects(renderer,nodes,{}, {delete(){}},{deleteForRender(){}},{})
  const lifetime=installRenderObjectLifetime(manager),scene=new THREE.Scene(),camera=new THREE.PerspectiveCamera(),lights=TSL.lights([]),context={id:1},frame=new NodeFrame()
  const waterFog=createSourceWaterFogUniforms(),exposure=TSL.uniform(1),lighting=new ModelLightingGraphs(),graphs=new StaticMaterialGraphs(waterFog,exposure,lighting.staticFade)
  const oldTemplates=new Map<string,any>(),oldColors=new Map<string,any>(),newColors=new Map<string,any>(),oldStates=new Set(),newStates=new Set(),retained:any[]=[],records:any[]=[]
  const root=new THREE.Group()
  const build=(mesh:THREE.Mesh)=>nodes.getForRender(manager.get(mesh,mesh.material,scene,camera,lights,context,null))
  const values=(state:any,mesh:THREE.Mesh)=>{
    frame.object=mesh;frame.renderer=renderer;frame.camera=camera;frame.scene=scene;frame.material=mesh.material as THREE.Material;frame.frameId++;frame.renderId++
    for(const node of state.updateBeforeNodes)frame.updateBeforeNode(node)
    for(const node of state.updateNodes)frame.updateNode(node)
    for(const group of state.bindings)for(const binding of group.bindings)if(binding.textureNode)binding.update()
    return state.bindings.map((group:any)=>group.bindings.map((binding:any)=>({kind:binding.constructor.name,visibility:binding.visibility,byteLength:binding.byteLength,
      uniforms:binding.uniforms?.map((uniform:any)=>{const v=uniform.getValue();return {kind:uniform.constructor.name,itemSize:uniform.itemSize,boundary:uniform.boundary,value:v?.toArray?v.toArray():v}}),texture:binding.texture?.uuid??null})))
  }
  return {
    admit(label:string,templateKey:string,materialIdentity:string,geometry:THREE.BufferGeometry,base:any,state:any,unlit:boolean,fading:boolean,side:THREE.Side){
      const fade=TSL.uniform(1),material=new THREE.MeshBasicNodeMaterial({side,transparent:state.blendEnabled,depthWrite:state.depthWrite,depthTest:state.depthTest})
      material.toneMapped=false
      let oldBase=oldTemplates.get(templateKey)
      if(!oldBase){oldBase=sourceFragmentColor(base,state,waterFog);oldTemplates.set(templateKey,oldBase)}
      const key=`${materialIdentity}:${side}:${unlit}:${fading}`
      let oldColor=oldColors.get(key)
      if(!oldColor){const rgb=unlit?oldBase.rgb:oldBase.rgb.mul(sourceStaticVertexLightingNode()).mul(exposure),opacity=state.alphaOwnership.opacity?oldBase.a:TSL.float(1)
        oldColor=sourceFragmentColor(TSL.vec4(rgb,opacity.mul(fading?lighting.staticFade:fade)),state,waterFog,fading);oldColors.set(key,oldColor)}
      let shared=newColors.get(key)
      if(!shared){
        const templateMaterial=material.clone();templateMaterial.colorNode=oldBase
        shared={color:graphs.vertex(graphs.template(base,state),state,unlit,fading,fade),preparation:templateMaterial.customProgramCacheKey()}
        templateMaterial.dispose();newColors.set(key,shared)
      }
      material.colorNode=shared.color
      const original=material.clone();original.colorNode=oldColor
      material.userData.sourcePreparationIdentity=shared.preparation
      for(const [selected,referenceMaterial,faded] of [[material,original,false],...(fading?[[createStaticPropFadeVariant(material),createStaticPropFadeVariant(original),true]]:[])] as const){
      const mesh=new THREE.Mesh(geometry,selected),before=new THREE.Mesh(geometry,referenceMaterial)
      bindStaticPropFade(mesh,fade);bindStaticPropFade(before,fade);root.add(mesh,before)
      const candidate=build(mesh),reference=build(before)
      equal(candidate.vertexShader,reference.vertexShader,`${label}: vertex WGSL`);equal(candidate.fragmentShader,reference.fragmentShader,`${label}: fragment WGSL`)
      for(const opacity of faded?[.5,0]:[1]){fade.value=opacity;equal(values(candidate,mesh),values(reference,before),`${label}: bind layout/values`)}
      newStates.add(candidate);oldStates.add(reference);retained.push({mesh,before,candidate,reference,fade,fading:faded})
      records.push({label:`${label}:${faded?"faded":"authored"}`,vertex:candidate.vertexShader,fragment:candidate.fragmentShader})
      }
    },
    finish(){
      for(const item of retained.toReversed()){item.fade.value=item.fading?.75:1;if(build(item.mesh)!==item.candidate)throw new Error("Warm static state rebuilt");equal(values(item.candidate,item.mesh),values(item.reference,item.before),"reverse draw bindings")}
      const beforeGroup=new THREE.Group(),afterGroup=new THREE.Group()
      for(const item of retained){beforeGroup.add(item.before);afterGroup.add(item.mesh)}
      root.add(beforeGroup,afterGroup)
      const beforeAdmission=prepareReachablePipelineVisibility(beforeGroup),afterAdmission=prepareReachablePipelineVisibility(afterGroup)
      const admission={before:beforeAdmission.variants,after:afterAdmission.variants}
      const partition=(field:"mesh"|"before")=>{const first=new Map<string,number>();return retained.map((item,index)=>{const key=pipelinePreparationIdentity(item[field]);if(!first.has(key))first.set(key,index);return first.get(key)})}
      equal(partition("before"),partition("mesh"),"Static resource equivalence classes differ")
      equal(retained.map(item=>item.before.visible),retained.map(item=>item.mesh.visible),"Static resource admission differs")
      beforeAdmission.restore();afterAdmission.restore()
      lifetime.release(root)
      if(lifetime.size||nodes.nodeBuilderCache.size)throw new Error("Retired static draws retain compiler states")
      for(const item of retained){item.mesh.material.dispose();item.before.material.dispose()}
      lifetime.restore();lighting.releaseDrawReferences()
      return {draws:retained.length,dedicatedCompilerStates:oldStates.size,sharedCompilerStates:newStates.size,retiredCompilerStates:nodes.nodeBuilderCache.size,admission,records}
    },
  }
}

export {THREE,TSL}
