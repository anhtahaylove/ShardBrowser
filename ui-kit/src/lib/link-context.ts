'use client'
import { createContext, useContext } from 'react'

/**
 * Link plumbing that carries no component export.
 *
 * Fast Refresh only preserves state for modules that export components and
 * nothing else, so the context and its hook live here rather than beside
 * <Link> and <LinkProvider>. Both are still re-exported from the package
 * entry point, so this split is invisible to consumers.
 */
export type LinkComponentProps = {
  href: string
  children?: React.ReactNode
} & Omit<React.AnchorHTMLAttributes<HTMLAnchorElement>, 'href'>

export type LinkComponent = React.ComponentType<LinkComponentProps>

export const LinkContext = createContext<LinkComponent | null>(null)

export function useLinkComponent(): LinkComponent | null {
  return useContext(LinkContext)
}
