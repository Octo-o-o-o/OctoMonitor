import { buildSnapshotRange, isSnapshotWindowClamped } from './snapshotWindow'

describe('snapshot window helpers', () => {
  it('returns the full loaded range for all', () => {
    const range = buildSnapshotRange('all', {
      from: new Date('2026-04-01T10:00:00.000Z'),
      to: new Date('2026-04-03T08:00:00.000Z'),
    })

    expect(range?.from.getFullYear()).toBe(2026)
    expect(range?.from.getMonth()).toBe(3)
    expect(range?.from.getDate()).toBe(1)
    expect(range?.from.getHours()).toBe(0)
    expect(range?.from.getMinutes()).toBe(0)
    expect(range?.to.getFullYear()).toBe(2026)
    expect(range?.to.getMonth()).toBe(3)
    expect(range?.to.getDate()).toBe(3)
    expect(range?.to.getHours()).toBe(23)
    expect(range?.to.getMinutes()).toBe(59)
  })

  it('clamps month to the loaded snapshot span', () => {
    const range = buildSnapshotRange('month', {
      from: new Date('2026-03-28T10:00:00.000Z'),
      to: new Date('2026-04-03T08:00:00.000Z'),
    })

    expect(range?.from.getFullYear()).toBe(2026)
    expect(range?.from.getMonth()).toBe(2)
    expect(range?.from.getDate()).toBe(28)
    expect(range?.from.getHours()).toBe(0)
    expect(range?.to.getFullYear()).toBe(2026)
    expect(range?.to.getMonth()).toBe(3)
    expect(range?.to.getDate()).toBe(3)
    expect(range?.to.getHours()).toBe(23)
    expect(isSnapshotWindowClamped('month', 7)).toBe(true)
    expect(isSnapshotWindowClamped('week', 7)).toBe(false)
  })
})
