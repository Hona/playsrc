import { expect, test } from "bun:test"
import * as THREE from "three/webgpu"
import { installParticleAliasOwnerReceipt } from "../src/particle-alias-owner-evidence"

test("post-sample owner hashes copy shared canonical views for WebCrypto without changing ownership", async () => {
  const data = new Uint8Array(new SharedArrayBuffer(8)); data.set([1, 2, 3, 4, 5, 6, 7, 8])
  const texture = new THREE.CompressedTexture([{ data, width: 4, height: 4 }], 4, 4, THREE.RGB_S3TC_DXT1_Format)
  const receipt = installParticleAliasOwnerReceipt(), material = new THREE.MeshBasicMaterial()
  receipt.register("a", material, texture); receipt.register("b", material, texture)
  const digest = crypto.subtle.digest.bind(crypto.subtle)
  crypto.subtle.digest = ((algorithm: any, input: any) => {
    if (input.buffer instanceof SharedArrayBuffer) throw new TypeError("WebCrypto rejects shared views")
    return digest(algorithm, input)
  }) as typeof crypto.subtle.digest
  try {
    const value = await receipt.snapshot()
    expect(value.uniqueImages).toBe(1)
    expect(value.records[0]!.mips).toEqual(value.records[1]!.mips)
    expect(value.records[0]!.mips[0]!.sha256).toBe(new Bun.CryptoHasher("sha256").update(data).digest("hex"))
    expect(texture.mipmaps[0]!.data).toBe(data)
  } finally { crypto.subtle.digest = digest; texture.dispose(); material.dispose() }
})
