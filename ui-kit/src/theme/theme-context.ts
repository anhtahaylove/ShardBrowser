import { createContext, useContext } from 'react'

/**
 * Theme context and hook, kept apart from <ThemeProvider>.
 *
 * Fast Refresh only preserves state for modules that export components and
 * nothing else, so the context, its types and useTheme live here. All of it
 * is re-exported from the package entry point, so consumers see no change.
 */
export type Theme = 'light' | 'dark' | 'system'
export type ResolvedTheme = 'light' | 'dark'

export type ThemeContextValue = {
  theme: Theme
  systemTheme: ResolvedTheme
  resolvedTheme: ResolvedTheme
  setTheme: (theme: Theme) => void
  toggleTheme: () => void
}

export const STORAGE_KEY = 'shardx-ui-kit-theme'

export const ThemeContext = createContext<ThemeContextValue | null>(null)

export function getSystemTheme(): ResolvedTheme {
  if (typeof window === 'undefined') return 'light'
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext)
  if (!ctx) throw new Error('useTheme must be used within a ThemeProvider')
  return ctx
}
