export class AudioError extends Error {
  constructor(
    readonly code: "MalformedResource" | "MissingResource" | "MalformedEvent" | "Capacity" | "Suspended" | "Closed" | "BrowserFailure",
    message: string,
  ) {
    super(message)
    this.name = "AudioError"
  }
}
