import type { Summary } from './types'
import { summaryMetrics } from './formatSummary'

export const runHistoryKey = 'ace-cdl-run-history-v1'
export const maxRunHistory = 12

export interface SimulationRun {
  id: string
  program: 'g36' | 'conventional' | 'custom'
  label: string
  completedAt: string
  durationMs: number
  requestedStepSeconds: number
  effectiveStepSeconds: number
  summary: Summary
}

function validSummary(value: unknown): value is Summary {
  if (!value || typeof value !== 'object') return false
  return summaryMetrics.every(metric => Number.isFinite((value as Record<string, unknown>)[metric.key]))
}

export function readRunHistory(storage: Pick<Storage, 'getItem'>): SimulationRun[] {
  try {
    const saved = JSON.parse(storage.getItem(runHistoryKey) ?? '[]')
    if (!Array.isArray(saved)) return []
    return saved.filter((run): run is SimulationRun =>
      run && typeof run.id === 'string' && typeof run.label === 'string' &&
      typeof run.completedAt === 'string' && Number.isFinite(Date.parse(run.completedAt)) &&
      Number.isFinite(run.durationMs) && Number.isFinite(run.requestedStepSeconds) &&
      Number.isFinite(run.effectiveStepSeconds) && validSummary(run.summary))
      .map(run => ({...run, program: run.program === 'g36' || run.program === 'custom'
        ? run.program
        : run.program == null && run.label.includes('G36') ? 'g36' : 'conventional'} as SimulationRun))
      .slice(0, maxRunHistory)
  } catch { return [] }
}

export function saveRunHistory(storage: Pick<Storage, 'setItem'>, runs: SimulationRun[]): boolean {
  try { storage.setItem(runHistoryKey, JSON.stringify(runs.slice(0, maxRunHistory))); return true }
  catch { return false }
}

export function clearRunHistory(storage: Pick<Storage, 'removeItem'>): boolean {
  try { storage.removeItem(runHistoryKey); return true }
  catch { return false }
}
