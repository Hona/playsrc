export type DiagnosticModelBaseDisposition = "authored-texture" | "identity-color"

export function selectDiagnosticModelBase(authoredTextureAvailable: boolean): DiagnosticModelBaseDisposition {
  return authoredTextureAvailable ? "authored-texture" : "identity-color"
}
