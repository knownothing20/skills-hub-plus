import { describe, it, expect } from 'vitest'
import { useLibraryHealth } from './useLibraryHealth'

describe('useLibraryHealth hook', () => {
  it('is exported as a function', () => {
    expect(typeof useLibraryHealth).toBe('function')
  })
})
