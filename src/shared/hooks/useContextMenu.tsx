import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ContextItem } from "../types";

/// Right-click context menu styled after the UI-kit dropdown surface.
export function useContextMenu() {
  const [menu, setMenu] = useState<{ x: number; y: number; items: ContextItem[] } | null>(null);
  const close = () => setMenu(null);
  useEffect(() => {
    if (!menu) return;
    // Registered on the next frame: the click that opened the menu is still
    // propagating to window, and dismissing on it would close the menu
    // instantly (it never appears for a left-click trigger).
    let armed = false;
    const arm = requestAnimationFrame(() => { armed = true; });
    const dismiss = () => { if (armed) close(); };
    window.addEventListener("click", dismiss);
    window.addEventListener("scroll", dismiss, true);
    return () => {
      cancelAnimationFrame(arm);
      window.removeEventListener("click", dismiss);
      window.removeEventListener("scroll", dismiss, true);
    };
  }, [menu]);
  const open = (e: React.MouseEvent, items: ContextItem[]) => {
    e.preventDefault();
    setMenu({ x: e.clientX, y: e.clientY, items });
  };
  // Clamp menu into viewport post-layout.
  const ref = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!menu || !el) return;
    const { width, height } = el.getBoundingClientRect();
    const pad = 8;
    let left = menu.x;
    let top = menu.y;
    if (left + width > window.innerWidth - pad) {
      left = Math.max(pad, window.innerWidth - width - pad);
    }
    if (top + height > window.innerHeight - pad) {
      top = Math.max(pad, window.innerHeight - height - pad);
    }
    el.style.left = `${left}px`;
    el.style.top = `${top}px`;
  }, [menu]);
  const node = menu ? (
    <div
      ref={ref}
      className="fixed z-9000 min-w-[160px] rounded-12 bg-bg-white-0 p-1 shadow-[var(--shadow-md)] ring-1 ring-inset ring-stroke-soft-200"
      style={{ left: menu.x, top: menu.y }}
      onClick={(e) => e.stopPropagation()}
    >
      {menu.items.map((it, i) =>
        it.sep ? (
          <div key={i} className="my-1 border-t border-stroke-soft-200" />
        ) : (
          <button
            key={i}
            type="button"
            disabled={!!it.disabledReason}
            title={it.disabledReason}
            className={`w-full rounded-8 border-0 bg-transparent px-2.5 py-2 text-left text-label-xs transition-colors ${
              it.disabledReason
                ? "cursor-not-allowed text-text-disabled-300"
                : it.danger
                  ? "cursor-pointer text-error-base hover:bg-bg-weak-50"
                  : "cursor-pointer text-text-sub-600 hover:bg-bg-weak-50 hover:text-text-strong-950"
            }`}
            onClick={() => {
              if (it.disabledReason) return;
              it.onClick();
              close();
            }}
          >
            {it.label}
          </button>
        ),
      )}
    </div>
  ) : null;
  return { open, node };
}
