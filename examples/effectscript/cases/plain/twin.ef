export const effect = (x: number) => x * 2
export const use = (x: number) => x + 1

export function add(a: number, b: number): number {
  return a + b
}

export async function later(a: number): Promise<number> {
  const value = await Promise.resolve(a)
  if (value < 0) {
    throw new Error("negative")
  }
  return effect(use(value))
}

export function* numbers() {
  yield 1
  yield* [2, 3]
}

export class Counter {
  count = 0
  increment() {
    this.count++
    return this
  }
}
