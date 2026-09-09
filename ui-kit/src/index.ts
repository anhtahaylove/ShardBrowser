/**
 * Shardx UI Kit — public entry point.
 *
 * Consume from another project:
 *   import { Button, ThemeProvider } from 'shardx-ui-kit'
 *   import 'shardx-ui-kit/styles.css'
 */
export * from './components'
export * as Icons from './icons'
export { ThemeProvider } from './theme/ThemeProvider'
export { useTheme } from './theme/theme-context'
export { default as ThemeToggle } from './theme/ThemeToggle'
export type { Theme, ResolvedTheme } from './theme/theme-context'
export { LinkProvider, Link } from './lib/link'
export { useLinkComponent } from './lib/link-context'
export type { LinkComponent, LinkComponentProps } from './lib/link-context'
export { cn } from './lib/cn'
