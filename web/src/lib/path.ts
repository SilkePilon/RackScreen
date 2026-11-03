// Read and immutably write nested values by dotted path ("screens.0.roles").

export function getAt(obj: unknown, path: string): unknown {
  let cur: unknown = obj
  for (const key of path.split(".")) {
    if (cur === null || cur === undefined || typeof cur !== "object") return undefined
    cur = (cur as Record<string, unknown>)[key]
  }
  return cur
}

export function setAt<T>(obj: T, path: string, value: unknown): T {
  const keys = path.split(".")
  const write = (node: unknown, i: number): unknown => {
    const key = keys[i]
    const base: Record<string, unknown> | unknown[] = Array.isArray(node)
      ? [...node]
      : { ...((node ?? {}) as Record<string, unknown>) }
    const rec = base as Record<string, unknown>
    if (i === keys.length - 1) {
      if (value === undefined && !Array.isArray(base)) delete rec[key]
      else rec[key] = value
    } else {
      rec[key] = write(rec[key], i + 1)
    }
    return base
  }
  return write(obj, 0) as T
}

export function deepEqual(a: unknown, b: unknown): boolean {
  if (a === b) return true
  if (a === null || b === null || typeof a !== "object" || typeof b !== "object") {
    // A missing key and an explicit null mean the same to the backend.
    return (a ?? null) === (b ?? null)
  }
  if (Array.isArray(a) !== Array.isArray(b)) return false
  const ka = Object.keys(a as object).filter((k) => (a as Record<string, unknown>)[k] !== undefined)
  const kb = Object.keys(b as object).filter((k) => (b as Record<string, unknown>)[k] !== undefined)
  if (ka.length !== kb.length) return false
  return ka.every((k) =>
    deepEqual((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]),
  )
}

/** Paths of the leaves (or arrays, as a whole) that differ between a and b. */
export function diffPaths(a: unknown, b: unknown, prefix = ""): string[] {
  const isObj = (v: unknown) => v !== null && typeof v === "object" && !Array.isArray(v)
  if (isObj(a) && isObj(b)) {
    const keys = new Set([...Object.keys(a as object), ...Object.keys(b as object)])
    return [...keys].flatMap((k) =>
      diffPaths(
        (a as Record<string, unknown>)[k],
        (b as Record<string, unknown>)[k],
        prefix ? `${prefix}.${k}` : k,
      ),
    )
  }
  if (
    Array.isArray(a) &&
    Array.isArray(b) &&
    a.length === b.length &&
    a.every(isObj) &&
    b.every(isObj)
  ) {
    return a.flatMap((v, i) => diffPaths(v, b[i], `${prefix}.${i}`))
  }
  return deepEqual(a, b) ? [] : [prefix]
}
