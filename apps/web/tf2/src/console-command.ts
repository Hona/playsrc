const BREAK_CHARACTERS = new Set(["{", "}", "(", ")", "'", ":"])

export function tokenizeSourceCommand(input: string): readonly string[] {
  const tokens: string[] = []
  let offset = 0
  while (offset < input.length) {
    while (offset < input.length && input.charCodeAt(offset) > 0 && input.charCodeAt(offset) <= 32) offset += 1
    if (input.startsWith("//", offset)) {
      const line = input.indexOf("\n", offset + 2)
      if (line === -1) break
      offset = line + 1
      continue
    }
    if (offset >= input.length || input[offset] === "\0") break
    const character = input[offset]!
    if (character === '"') {
      offset += 1
      const end = input.indexOf('"', offset)
      if (end === -1) {
        tokens.push(input.slice(offset))
        break
      }
      tokens.push(input.slice(offset, end))
      offset = end + 1
      continue
    }
    if (BREAK_CHARACTERS.has(character)) {
      tokens.push(character)
      offset += 1
      continue
    }
    const start = offset
    while (offset < input.length && input.charCodeAt(offset) > 32
      && input[offset] !== '"' && !BREAK_CHARACTERS.has(input[offset]!)) offset += 1
    tokens.push(input.slice(start, offset))
  }
  return Object.freeze(tokens)
}
