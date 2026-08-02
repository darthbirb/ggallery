/**
 * Right-click menus and the header's dropdowns, over Radix.
 *
 * The primitives are taken for keyboard behaviour, focus trapping and
 * dismissal — tedious to get right by hand and invisible when wrong. Every
 * pixel of the look is ours; no visual component kit is adopted (PLAN.md
 * §M2.5 "Build notes").
 *
 * Radix ContextMenu and DropdownMenu have the same item API, so the item,
 * separator and label components below are shared and take which family to
 * render from context. That is what lets a menu definition be written once and
 * opened either way.
 */

import * as RadixContextMenu from "@radix-ui/react-context-menu";
import * as RadixDropdownMenu from "@radix-ui/react-dropdown-menu";
import { createContext, useContext, type ReactNode } from "react";

type Family = "context" | "dropdown";

const FamilyContext = createContext<Family>("context");

const SURFACE =
  "surface-in z-50 min-w-[196px] rounded-[5px] border border-line bg-panel p-1 shadow-[0_10px_30px_rgba(0,0,0,0.45)]";

const ITEM =
  "flex cursor-default select-none items-center gap-2 rounded-[3px] px-2 py-[5px] text-[13px] outline-none " +
  "data-[highlighted]:bg-hover data-[disabled]:pointer-events-none data-[disabled]:opacity-35";

export interface MenuItemProps {
  children: ReactNode;
  onSelect?: () => void;
  disabled?: boolean;
  /** Destructive items are red, and always sit last in their group. */
  danger?: boolean;
  /** The keyboard equivalent, shown right-aligned. Never the only path to
   *  the action — locked decision 23. */
  shortcut?: string;
}

export function MenuItem({
  children,
  onSelect,
  disabled,
  danger,
  shortcut,
}: MenuItemProps) {
  const family = useContext(FamilyContext);
  const Item = family === "context" ? RadixContextMenu.Item : RadixDropdownMenu.Item;
  return (
    <Item
      disabled={disabled}
      onSelect={() => onSelect?.()}
      className={`${ITEM} ${danger ? "text-danger" : "text-fg"}`}
    >
      <span className="flex-1 truncate">{children}</span>
      {shortcut && (
        <span className="font-mono text-[11px] text-fg-dim">{shortcut}</span>
      )}
    </Item>
  );
}

export function MenuSeparator() {
  const family = useContext(FamilyContext);
  const Separator =
    family === "context" ? RadixContextMenu.Separator : RadixDropdownMenu.Separator;
  return <Separator className="my-1 h-px bg-line-soft" />;
}

export function MenuLabel({ children }: { children: ReactNode }) {
  const family = useContext(FamilyContext);
  const Label = family === "context" ? RadixContextMenu.Label : RadixDropdownMenu.Label;
  return (
    <Label className="px-2 pb-1 pt-1.5 font-mono text-[10px] uppercase tracking-[0.12em] text-fg-dim">
      {children}
    </Label>
  );
}

/** A nested menu — "Move to…" opening the folder list, and so on. */
export function MenuSub({
  label,
  children,
  disabled,
}: {
  label: ReactNode;
  children: ReactNode;
  disabled?: boolean;
}) {
  const family = useContext(FamilyContext);
  const parts =
    family === "context"
      ? {
          Sub: RadixContextMenu.Sub,
          Trigger: RadixContextMenu.SubTrigger,
          Portal: RadixContextMenu.Portal,
          Content: RadixContextMenu.SubContent,
        }
      : {
          Sub: RadixDropdownMenu.Sub,
          Trigger: RadixDropdownMenu.SubTrigger,
          Portal: RadixDropdownMenu.Portal,
          Content: RadixDropdownMenu.SubContent,
        };
  return (
    <parts.Sub>
      <parts.Trigger disabled={disabled} className={`${ITEM} text-fg`}>
        <span className="flex-1 truncate">{label}</span>
        <span className="text-[10px] text-fg-dim">▸</span>
      </parts.Trigger>
      <parts.Portal>
        <parts.Content
          sideOffset={2}
          alignOffset={-4}
          className={`${SURFACE} max-h-[60vh] overflow-y-auto`}
        >
          {children}
        </parts.Content>
      </parts.Portal>
    </parts.Sub>
  );
}

/**
 * Right-click anywhere inside `trigger` opens `menu`. The WebView's own menu
 * is suppressed globally in `App.tsx`; this is what replaces it.
 */
export function ContextMenu({
  menu,
  children,
  disabled,
  className,
}: {
  menu: ReactNode;
  children: ReactNode;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <RadixContextMenu.Root>
      {/* The trigger is a `span` by default, which would make block content
          inline and quietly wreck the layout of anything wrapped in it. */}
      <RadixContextMenu.Trigger
        disabled={disabled}
        className={`block ${className ?? ""}`}
        // Menus nest — a folder row sits inside the tree's background menu.
        // Without this both would open, and the outer one would win. Only
        // propagation is stopped, never the default, so Radix's own handler
        // still runs on the innermost trigger.
        onContextMenu={(event) => event.stopPropagation()}
      >
        {children}
      </RadixContextMenu.Trigger>
      <RadixContextMenu.Portal>
        <RadixContextMenu.Content className={SURFACE} collisionPadding={8}>
          <FamilyContext.Provider value="context">{menu}</FamilyContext.Provider>
        </RadixContextMenu.Content>
      </RadixContextMenu.Portal>
    </RadixContextMenu.Root>
  );
}

/**
 * A menu opened at a point rather than by a trigger element.
 *
 * The grid's tiles are recycled DOM nodes rather than React components (see
 * `features/grid/Tile.tsx`), so they cannot each wrap themselves in a Radix
 * trigger. They report where the right-click happened instead, and this
 * anchors a menu there — same keyboard behaviour, same dismissal, no per-tile
 * React tree.
 */
export function PointMenu({
  at,
  onClose,
  children,
}: {
  at: { x: number; y: number } | null;
  onClose: () => void;
  children: ReactNode;
}) {
  return (
    <RadixDropdownMenu.Root
      open={at !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <RadixDropdownMenu.Trigger
        aria-hidden
        tabIndex={-1}
        style={{
          position: "fixed",
          left: at?.x ?? 0,
          top: at?.y ?? 0,
          width: 1,
          height: 1,
          pointerEvents: "none",
          opacity: 0,
        }}
      />
      <RadixDropdownMenu.Portal>
        <RadixDropdownMenu.Content
          align="start"
          side="bottom"
          sideOffset={0}
          collisionPadding={8}
          className={`${SURFACE} max-h-[70vh] overflow-y-auto`}
        >
          <FamilyContext.Provider value="dropdown">{children}</FamilyContext.Provider>
        </RadixDropdownMenu.Content>
      </RadixDropdownMenu.Portal>
    </RadixDropdownMenu.Root>
  );
}

/** The same menu vocabulary, opened by clicking a button. */
export function DropdownMenu({
  trigger,
  children,
  align = "start",
}: {
  trigger: ReactNode;
  children: ReactNode;
  align?: "start" | "center" | "end";
}) {
  return (
    <RadixDropdownMenu.Root>
      <RadixDropdownMenu.Trigger asChild>{trigger}</RadixDropdownMenu.Trigger>
      <RadixDropdownMenu.Portal>
        <RadixDropdownMenu.Content
          align={align}
          sideOffset={4}
          collisionPadding={8}
          className={`${SURFACE} max-h-[70vh] overflow-y-auto`}
        >
          <FamilyContext.Provider value="dropdown">{children}</FamilyContext.Provider>
        </RadixDropdownMenu.Content>
      </RadixDropdownMenu.Portal>
    </RadixDropdownMenu.Root>
  );
}
