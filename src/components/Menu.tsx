/**
 * Right-click menus and the header's dropdowns.
 *
 * shadcn/ui ships `dropdown-menu` and `context-menu` as two near-identical
 * files over two Radix families with the same item API. GGallery opens the
 * *same* menu definition either way — right-click a folder row, or click the
 * header's button — so the look lives once in `ui/menu.tsx` and this file
 * picks the family from context. That is what lets a menu be written once and
 * still be complete in both places (locked decision 23).
 */

import * as RadixContextMenu from "@radix-ui/react-context-menu";
import * as RadixDropdownMenu from "@radix-ui/react-dropdown-menu";
import { ChevronRight } from "lucide-react";
import { createContext, useContext, type ReactNode } from "react";

import { cn } from "../lib/utils";
import { menuItem, menuLabel, menuSeparator, menuShortcut, menuSurface } from "./ui/menu";

type Family = "context" | "dropdown";

const FamilyContext = createContext<Family>("context");

export interface MenuItemProps {
  children: ReactNode;
  onSelect?: () => void;
  disabled?: boolean;
  /** Named to match `@shadcn/dropdown-menu` and `@shadcn/context-menu`'s own
   *  `DropdownMenuItem`/`ContextMenuItem` `variant` prop (M2.5a.3 audit) — one
   *  fewer name to translate when reading either against the registry.
   *  Destructive items are red, and always sit last in their group. */
  variant?: "destructive";
  /** The keyboard equivalent, shown right-aligned. Never the only path to
   *  the action — locked decision 23. */
  shortcut?: string;
}

export function MenuItem({
  children,
  onSelect,
  disabled,
  variant,
  shortcut,
}: MenuItemProps) {
  const family = useContext(FamilyContext);
  const Item = family === "context" ? RadixContextMenu.Item : RadixDropdownMenu.Item;
  return (
    <Item
      disabled={disabled}
      onSelect={() => onSelect?.()}
      className={cn(
        menuItem,
        variant === "destructive" ? "text-danger data-[highlighted]:text-danger" : "text-fg",
      )}
    >
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {shortcut && <span className={menuShortcut}>{shortcut}</span>}
    </Item>
  );
}

export function MenuSeparator() {
  const family = useContext(FamilyContext);
  const Separator =
    family === "context" ? RadixContextMenu.Separator : RadixDropdownMenu.Separator;
  return <Separator className={menuSeparator} />;
}

export function MenuLabel({ children }: { children: ReactNode }) {
  const family = useContext(FamilyContext);
  const Label = family === "context" ? RadixContextMenu.Label : RadixDropdownMenu.Label;
  return <Label className={menuLabel}>{children}</Label>;
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
      <parts.Trigger
        disabled={disabled}
        className={cn(menuItem, "text-fg data-[state=open]:bg-hover")}
      >
        <span className="min-w-0 flex-1 truncate">{label}</span>
        <ChevronRight className="ml-auto size-4 shrink-0 text-fg-dim" />
      </parts.Trigger>
      <parts.Portal>
        <parts.Content
          sideOffset={2}
          alignOffset={-4}
          className={cn(menuSurface, "max-h-[60vh] overflow-y-auto")}
        >
          {children}
        </parts.Content>
      </parts.Portal>
    </parts.Sub>
  );
}

/**
 * Right-click anywhere inside `trigger` opens `menu`. The WebView's own menu
 * is suppressed globally; this is what replaces it.
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
        className={cn("block", className)}
        // Menus nest — a folder row sits inside the tree's background menu.
        // Without this both would open, and the outer one would win. Only
        // propagation is stopped, never the default, so Radix's own handler
        // still runs on the innermost trigger.
        onContextMenu={(event) => event.stopPropagation()}
      >
        {children}
      </RadixContextMenu.Trigger>
      <RadixContextMenu.Portal>
        <RadixContextMenu.Content className={menuSurface} collisionPadding={8}>
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
          className={cn(menuSurface, "max-h-[70vh] overflow-y-auto")}
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
          className={cn(menuSurface, "max-h-[70vh] overflow-y-auto")}
        >
          <FamilyContext.Provider value="dropdown">{children}</FamilyContext.Provider>
        </RadixDropdownMenu.Content>
      </RadixDropdownMenu.Portal>
    </RadixDropdownMenu.Root>
  );
}
