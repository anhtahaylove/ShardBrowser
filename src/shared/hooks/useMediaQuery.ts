import { useEffect, useState } from "react";

/// Subscribes to a CSS media query from React.
///
/// Used where a layout decision must also change the DOM rather than just
/// hide it: a button that is merely display:none is still focusable by name
/// for assistive tech and still reachable in tests, which makes "this action
/// collapses into the menu on a narrow window" untestable and a11y-hostile.
export function useMediaQuery(query: string): boolean {
  const [matches, setMatches] = useState(() =>
    typeof window !== "undefined" && typeof window.matchMedia === "function"
      ? window.matchMedia(query).matches
      : false,
  );

  useEffect(() => {
    if (typeof window === "undefined" || typeof window.matchMedia !== "function") return;
    const mql = window.matchMedia(query);
    const onChange = () => setMatches(mql.matches);
    onChange();
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, [query]);

  return matches;
}

/// Wide enough to show the secondary row actions inline. Kept next to the
/// --breakpoint-wide token in index.css; change both together.
export const WIDE_ACTIONS_QUERY = "(min-width: 1600px)";
