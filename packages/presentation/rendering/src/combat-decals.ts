import * as THREE from "three/webgpu"

export type CombatDecalInput=Readonly<{identity:number;face:number;reference:string;positions:Float32Array;normals:Float32Array;uv:Float32Array;indices:Uint32Array}>

/** Geometry is per impact; the authored atlas and shader are per map. */
export class CombatDecals {
  readonly #records=new Map<number,Readonly<{face:number;mesh:THREE.Mesh}>>()
  #material:THREE.Material|undefined
  constructor(readonly group:THREE.Group,readonly materialIdentity:string,readonly createMaterial:()=>THREE.Material){}
  get records():ReadonlyMap<number,Readonly<{face:number;mesh:THREE.Mesh}>>{return this.#records}

  update(inputs:readonly CombatDecalInput[],maximum:number):void{
    for(const input of inputs){
      if(this.#records.has(input.identity)||maximum===0)continue
      const material=this.#material??=this.createMaterial()
      const geometry=new THREE.BufferGeometry()
      geometry.setAttribute("position",new THREE.BufferAttribute(input.positions,3))
      geometry.setAttribute("normal",new THREE.BufferAttribute(input.normals,3))
      geometry.setAttribute("uv",new THREE.BufferAttribute(input.uv,2))
      geometry.setIndex(new THREE.BufferAttribute(input.indices,1))
      const mesh=new THREE.Mesh(geometry,material)
      mesh.matrixAutoUpdate=false;mesh.updateMatrix();mesh.renderOrder=0;mesh.visible=false
      mesh.userData.combatDecal=input.reference;mesh.userData.materialIdentity=this.materialIdentity
      this.group.add(mesh);this.#records.set(input.identity,{face:input.face,mesh})
      while(this.#records.size>maximum){
        const identity=this.#records.keys().next().value!
        const expired=this.#records.get(identity)!
        this.group.remove(expired.mesh);expired.mesh.geometry.dispose();this.#records.delete(identity)
      }
    }
  }

  dispose():void{
    for(const {mesh}of this.#records.values()){this.group.remove(mesh);mesh.geometry.dispose()}
    this.#records.clear();this.#material?.dispose();this.#material=undefined
  }
}
