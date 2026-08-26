declare module "virtual:playsrc-generation" {
  export const APPLICATION_BUILD: string
  export const WASM_SHA256: string
  export const RESOURCE_ROOTS: Readonly<Record<string, string>>
}
