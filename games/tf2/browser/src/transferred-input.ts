/** Batched inputs can alias inside one message. Retire each backing store only
 * after every consumer has copied it, and only once. Do not wait for idle GC
 * while a synchronous map compile holds the message alive. */
export function retireTransferredInputs(inputs: Iterable<ArrayBuffer>): void {
  for (const input of new Set(inputs)) input.transfer(0)
}
