export interface GraphPoint { x: number; y: number }

/** Orthogonal wires share one routing rule in the primer and editor. */
export function orthogonalWire(from: GraphPoint, to: GraphPoint): string {
  if (Math.abs(from.y - to.y) < 0.5) return `M${from.x} ${from.y} L${to.x} ${to.y}`
  if (to.x >= from.x + 64) {
    const middle = (from.x + to.x) / 2
    return `M${from.x} ${from.y} L${middle} ${from.y} L${middle} ${to.y} L${to.x} ${to.y}`
  }
  const right = from.x + 36
  const left = to.x - 36
  const bus = (from.y + to.y) / 2
  return `M${from.x} ${from.y} L${right} ${from.y} L${right} ${bus} L${left} ${bus} L${left} ${to.y} L${to.x} ${to.y}`
}
