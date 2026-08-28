/** Observe input latency without feeding it back into the movement workload. */
export async function fixedInputPulses<T>(driver: {
  now(): number; wait(milliseconds: number): Promise<void>; admit(): Promise<void>
  send(down: boolean): Promise<void>; observe(down: boolean): Promise<T>
}) {
  const start = driver.now(), observations: Promise<T>[] = [], delivery: { down: boolean; scheduled: number; delivered: number }[] = []
  for (let event = 0; event < 6; event++) {
    const down = event % 2 === 0, scheduled = start + 200 + event * 100
    await driver.admit()
    const remaining = scheduled - driver.now()
    if (remaining > 0) await driver.wait(remaining)
    await driver.send(down)
    delivery.push({ down, scheduled: scheduled - start, delivered: driver.now() - start })
    observations.push(driver.observe(down))
  }
  return { delivery, observations: await Promise.all(observations) }
}
