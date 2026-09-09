'use client'
import { useContext } from 'react'
import { LinkContext, type LinkComponent, type LinkComponentProps } from './link-context'

/**
 * Framework-agnostic link plumbing.
 *
 * Kit components render links through <Link>, which uses whatever link
 * component the host app injects via <LinkProvider> (e.g. next/link or a
 * react-router Link). If nothing is provided, it falls back to a plain <a>,
 * so the kit works everywhere with zero required setup.
 *
 * The context and useLinkComponent live in ./link-context so this module
 * exports components only, which is what Fast Refresh needs.
 */
export function LinkProvider({
  component,
  children,
}: {
  component: LinkComponent
  children: React.ReactNode
}) {
  return <LinkContext.Provider value={component}>{children}</LinkContext.Provider>
}

/** Renders through the injected link component, or a native <a> as fallback. */
export function Link({ href, children, ...rest }: LinkComponentProps) {
  const Component = useContext(LinkContext)
  if (Component) {
    return (
      <Component href={href} {...rest}>
        {children}
      </Component>
    )
  }
  return (
    <a href={href} {...rest}>
      {children}
    </a>
  )
}
