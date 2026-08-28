import {expect,test} from "bun:test"
import type {PresentationArtifacts} from "../src/artifacts"
import {mapRendererInputs} from "../../../../apps/web/tf2/src/map-renderer-inputs"

test("initial, replacement and rollback share the entire authored render closure including legacy materials",()=>{
  const make=()=>({directionalTextures:[],environment:{},particleTextures:[],legacyVisualTextures:[{material:"cable"}],modelOccurrences:[{entity:7,lighting:{},eyes:{}}],modelMaterials:new Map(),authoredTextures:new Map(),brushModels:[],staticProps:{}} as unknown as PresentationArtifacts)
  const first=make(),second=make()
  for(const artifacts of [first,second,first]){
    const input=mapRendererInputs(artifacts)
    for(const key of ["directionalTextures","environment","particleTextures","legacyVisualTextures","modelOccurrences","modelMaterials","authoredTextures","brushModels","staticProps"] as const)expect(input[key]).toBe(artifacts[key])
    expect(input.modelDrawInputs[0]!.lighting).toBe(artifacts.modelOccurrences[0]!.lighting)
    expect(input.modelDrawInputs[0]!.eyes).toBe(artifacts.modelOccurrences[0]!.eyes)
  }
})
