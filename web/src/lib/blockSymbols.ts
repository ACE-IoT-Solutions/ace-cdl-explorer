import { blockSymbols } from './blockSymbolData'

const cdl = 'Buildings.Controls.OBC.CDL.'
const aliases: Record<string, string> = {
  Add: 'Reals.Add', Gain: 'Reals.MultiplyByParameter',
  Delay: 'Discrete.UnitDelay', LimPID: 'Reals.PIDWithReset',
  LimRateLimiter: 'Reals.LimitSlewRate', Switch: 'Reals.Switch',
  Constant: 'Reals.Sources.Constant',
}

/** Resolve full CDL class names before short legacy names to retain signal types. */
export function symbolForType(type: string): { source?: string; graphics: string } {
  const normalized = type.replace(/^.*#(?=Buildings\.)/, '')
  const candidates = normalized.startsWith(cdl) ? [normalized] : [
    cdl + (aliases[normalized] ?? normalized),
    ...['Reals.', 'Logical.', 'Discrete.', 'Conversions.', 'Routing.'].map(group => cdl + group + normalized),
  ]
  const source = candidates.find(candidate => Object.hasOwn(blockSymbols, candidate))
  return source ? { source, graphics: blockSymbols[source]! } : { graphics: '' }
}
