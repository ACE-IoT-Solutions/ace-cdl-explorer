import type { Scenario } from './types'

/** Vue's reactive nested arrays cannot cross the Worker structured-clone boundary. */
export function scenarioForWorker(scenario: Scenario): Scenario {
  return JSON.parse(JSON.stringify(scenario)) as Scenario
}
