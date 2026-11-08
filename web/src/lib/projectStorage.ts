import type { Project } from './types'

const projectKey = 'ace-cdl-project-v1'
const isBackup = (key: string) => key.startsWith('ace-cdl-') && key.endsWith('-backup')
const isQuotaError = (error: unknown) => error instanceof DOMException &&
  (error.name === 'QuotaExceededError' || error.name === 'NS_ERROR_DOM_QUOTA_REACHED')

function writeWithBackupPruning(storage: Storage, key: string, value: string): boolean {
  try {
    storage.setItem(key, value)
    return true
  } catch (error) {
    if (!isQuotaError(error)) throw error
  }
  // Migration backups are internal snapshots. Keep the current project and
  // newer backups by releasing older snapshots only when storage is full.
  const backups = Array.from({ length: storage.length }, (_, index) => storage.key(index))
    .filter((candidate): candidate is string => candidate !== null && candidate !== key && isBackup(candidate))
  for (const backup of backups) {
    storage.removeItem(backup)
    try {
      storage.setItem(key, value)
      return true
    } catch (error) {
      if (!isQuotaError(error)) throw error
    }
  }
  return false
}

export function storeMigrationBackup(storage: Storage, key: string, value: string): boolean {
  if (!isBackup(key)) throw new Error(`Invalid migration backup key: ${key}`)
  return writeWithBackupPruning(storage, key, value)
}

export function storeProject(storage: Storage, project: Project): boolean {
  return writeWithBackupPruning(storage, projectKey, JSON.stringify(project))
}
