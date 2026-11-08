import assert from 'node:assert/strict'
import { test } from 'node:test'
import { storeMigrationBackup, storeProject } from '../src/lib/projectStorage'
import type { Project } from '../src/lib/types'

class LimitedStorage {
  private entries = new Map<string, string>()
  constructor(private readonly limit: number) {}
  get length() { return this.entries.size }
  key(index: number) { return [...this.entries.keys()][index] ?? null }
  getItem(key: string) { return this.entries.get(key) ?? null }
  removeItem(key: string) { this.entries.delete(key) }
  setItem(key: string, value: string) {
    const size = [...this.entries].reduce((sum, [name, text]) => sum + (name === key ? 0 : text.length), value.length)
    if (size > this.limit) throw new DOMException('Storage full', 'QuotaExceededError')
    this.entries.set(key, value)
  }
}

test('migration backups shed older snapshots while preserving the current project', () => {
  const storage = new LimitedStorage(100)
  storage.setItem('ace-cdl-project-v1', 'current'.repeat(5))
  storage.setItem('ace-cdl-g36-pre-old-backup', 'x'.repeat(50))
  storage.setItem('other-app', 'keep')
  assert.equal(storeMigrationBackup(storage as unknown as Storage, 'ace-cdl-g36-pre-co2-reset-backup', 'y'.repeat(50)), true)
  assert.equal(storage.getItem('ace-cdl-g36-pre-old-backup'), null)
  assert.equal(storage.getItem('ace-cdl-project-v1'), 'current'.repeat(5))
  assert.equal(storage.getItem('other-app'), 'keep')
})

test('project autosave can release migration backups when the edited graph grows', () => {
  const storage = new LimitedStorage(100)
  storage.setItem('ace-cdl-project-v1', 'old')
  storage.setItem('ace-cdl-g36-pre-co2-reset-backup', 'x'.repeat(70))
  const project = { version: 1, name: 'Edited', graph: { name: 'Edited' }, scenario: {}, layout: {} } as Project
  assert.equal(storeProject(storage as unknown as Storage, project), true)
  assert.equal(storage.getItem('ace-cdl-project-v1'), JSON.stringify(project))
  assert.equal(storage.getItem('ace-cdl-g36-pre-co2-reset-backup'), null)
})

test('failed storage write leaves the saved project intact', () => {
  const storage = new LimitedStorage(30)
  storage.setItem('ace-cdl-project-v1', 'saved project')
  assert.equal(storeMigrationBackup(storage as unknown as Storage, 'ace-cdl-g36-pre-co2-reset-backup', 'x'.repeat(50)), false)
  assert.equal(storage.getItem('ace-cdl-project-v1'), 'saved project')
})
