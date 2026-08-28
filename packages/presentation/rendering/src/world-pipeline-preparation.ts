import * as THREE from "three/webgpu"

export function createWorldClipGroup(plane=new THREE.Plane(new THREE.Vector3(),0)):THREE.ClippingGroup {
  const group=new THREE.ClippingGroup();group.clippingPlanes=[plane];group.enabled=true;return group
}

/** World views retain one clip-plane slot. The zero plane rejects no fragments;
 * changing to an authored Water plane updates data, not the shader program. */
export async function prepareWorldViewPipelines(
  renderer:THREE.WebGPURenderer,root:THREE.Object3D,camera:THREE.Camera,scene:THREE.Scene,
  clipping:THREE.ClippingGroup,waterTargets:readonly THREE.RenderTarget[],
  waterFogs:readonly (THREE.Fog|null)[]=[],
):Promise<void>{
  const target=renderer.getRenderTarget(),enabled=clipping.enabled,fog=scene.fog
  try{
    clipping.enabled=true
    const fogs=[fog,...waterFogs].filter((value,index,values)=>values.findIndex(candidate=>candidate?.constructor===value?.constructor)===index)
    for(const selected of [target,...waterTargets]){
      renderer.setRenderTarget(selected)
      for(const variant of fogs){scene.fog=variant;await renderer.compileAsync(root,camera,scene)}
    }
  }finally{scene.fog=fog;clipping.enabled=enabled;renderer.setRenderTarget(target)}
}
