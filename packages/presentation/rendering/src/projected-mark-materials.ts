import * as THREE from "three/webgpu"

/** Share the authored shader graph, without changing opaque painter ordering. */
export class ProjectedMarkMaterials {
  readonly #programs=new Map<string,THREE.MeshBasicNodeMaterial>()
  readonly #owned=new Set<THREE.MeshBasicNodeMaterial>()
  constructor(readonly create:(identity:string)=>THREE.MeshBasicNodeMaterial){}
  fragment(identity:string):THREE.MeshBasicNodeMaterial {
    let program=this.#programs.get(identity)
    if(!program){program=this.create(identity);this.#programs.set(identity,program);this.#owned.add(program)}
    if(program.transparent)return program
    // Three sorts opaque ties by material id. Preserve the original per-fragment
    // order while cloning only material state, not the texture/fog node graph.
    const material=program.clone();this.#owned.add(material);return material
  }
  dispose():void{for(const material of this.#owned)material.dispose();this.#owned.clear();this.#programs.clear()}
}
