import type { Scenario } from './types'

/** Worker messages contain JSON data; Vue proxies cannot be structured-cloned. */
export function plainWorkerData<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T
}

export function scenarioForWorker(scenario: Scenario): Scenario {
  return plainWorkerData(scenario)
}
