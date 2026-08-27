type Group = object
type Texture = { addEventListener(type: string, callback: () => void): void; removeEventListener(type: string, callback: () => void): void }
type TextureData = { bindGroups?: Set<Group> }
type Textures = { updateTexture(texture: Texture, ...args: any[]): any; get(texture: Texture): TextureData }
type Backend = { deleteBindGroupData(group: Group): void }

class TextureMemberships extends Set<Group> {
  constructor(readonly reverse: WeakMap<Group, Set<TextureMemberships>>, values: Iterable<Group>) {
    super()
    for (const group of values) this.add(group)
  }
  override add(group: Group): this {
    super.add(group)
    let memberships = this.reverse.get(group)
    if (!memberships) this.reverse.set(group, memberships = new Set())
    memberships.add(this)
    return this
  }
  override delete(group: Group): boolean {
    if (!super.delete(group)) return false
    this.reverse.get(group)?.delete(this)
    return true
  }
  override clear(): void { for (const group of this) this.delete(group) }
}

/** Texture invalidation tracks live bind groups, not historical draws. Retired
 * groups must leave every texture they visited, including animated/cubemap swaps. */
export function installTextureBindingLifetime(textures: Textures, backend: Backend): () => void {
  const update = textures.updateTexture, remove = backend.deleteBindGroupData
  const updateDescriptor = Object.getOwnPropertyDescriptor(textures, "updateTexture")
  const removeDescriptor = Object.getOwnPropertyDescriptor(backend, "deleteBindGroupData")
  const reverse = new WeakMap<Group, Set<TextureMemberships>>()
  const detachers = new Set<() => void>()
  textures.updateTexture = function (texture, ...args) {
    const result = update.call(this, texture, ...args)
    const data = this.get(texture)
    if (data.bindGroups && !(data.bindGroups instanceof TextureMemberships)) {
      const memberships = new TextureMemberships(reverse, data.bindGroups)
      data.bindGroups = memberships
      const detach = () => { texture.removeEventListener("dispose", disposed); detachers.delete(detach) }
      const disposed = () => { memberships.clear(); detach() }
      // Native invalidation was installed by updateTexture and runs first.
      texture.addEventListener("dispose", disposed)
      detachers.add(detach)
    }
    return result
  }
  backend.deleteBindGroupData = function (group) {
    const memberships = reverse.get(group)
    if (memberships) for (const set of [...memberships]) set.delete(group)
    reverse.delete(group)
    remove.call(this, group)
  }
  return () => {
    for (const detach of [...detachers]) detach()
    if (updateDescriptor) Object.defineProperty(textures, "updateTexture", updateDescriptor)
    else delete (textures as Partial<Textures>).updateTexture
    if (removeDescriptor) Object.defineProperty(backend, "deleteBindGroupData", removeDescriptor)
    else delete (backend as Partial<Backend>).deleteBindGroupData
  }
}
