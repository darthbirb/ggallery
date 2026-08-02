import {
  createContext,
  createElement,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import * as ipc from "../lib/ipc";

/** Row heights the size control steps through, in pixels. */
export const TILE_SIZES = [96, 132, 180, 240, 320];

/**
 * The fixed accent set — locked decision 24. Fixed rather than free so every
 * value can be contrast-checked against the same greys; the CSS for each lives
 * in `styles/index.css` under `:root[data-accent="…"]`.
 */
export const ACCENTS = [
  { key: "slate", label: "Slate" },
  { key: "teal", label: "Teal" },
  { key: "violet", label: "Violet" },
  { key: "rose", label: "Rose" },
  { key: "moss", label: "Moss" },
  { key: "amber", label: "Amber" },
] as const;

export type Accent = (typeof ACCENTS)[number]["key"];

/**
 * What the pane holds. Only `preview` is built in M2.5a; the other two are
 * M2.5b's, and the control that switches between them says so rather than
 * pretending they are not coming.
 */
export type PaneMode = "preview" | "grid" | "folders";

export const PANE_MODES: { key: PaneMode; label: string }[] = [
  { key: "preview", label: "Preview" },
  { key: "grid", label: "Grid" },
  { key: "folders", label: "Folders" },
];

/** Below this a panel is not usable; the drag stops rather than collapsing. */
export const NAV_MIN = 150;
export const NAV_MAX = 420;
export const NAV_DEFAULT = 200;
/** Folded: an icon strip that keeps queue badges on screen. */
export const NAV_FOLDED = 44;
export const PANE_MIN = 260;
export const PANE_DEFAULT: Record<PaneMode, number> = {
  preview: 460,
  grid: 520,
  folders: 380,
};

export interface UiPrefs {
  navWidth: number;
  navFolded: boolean;
  paneOpen: boolean;
  paneMode: PaneMode;
  /** Remembered per mode — DESIGN.md §2 "Panels are resizable". */
  paneWidths: Record<PaneMode, number>;
  /** Global, not per folder: per-folder state would reflow the grid on every
   *  navigation, and nobody would curate it. DESIGN.md §2 "Folder band". */
  bandExpanded: boolean;
  /** The preview's details block, likewise global. */
  detailsExpanded: boolean;
  accent: Accent;
  tileHeight: number;
}

const DEFAULTS: UiPrefs = {
  navWidth: NAV_DEFAULT,
  navFolded: false,
  paneOpen: true,
  paneMode: "preview",
  paneWidths: { ...PANE_DEFAULT },
  bandExpanded: false,
  detailsExpanded: false,
  accent: "slate",
  tileHeight: TILE_SIZES[1],
};

export interface UiState extends UiPrefs {
  /** Whether the stored preferences have been read yet. Nothing persists
   *  before they have, so a slow first read can never overwrite them. */
  loaded: boolean;
  set: <K extends keyof UiPrefs>(key: K, value: UiPrefs[K]) => void;
  setPaneWidth: (mode: PaneMode, width: number) => void;
  /** Current mode's remembered width, clamped to what fits. */
  paneWidth: number;
  resetNavWidth: () => void;
  resetPaneWidth: () => void;
}

/** Merge what was stored over the defaults, field by field, so a preferences
 *  file written by an older build (or hand-edited) can never leave a value
 *  missing or of the wrong type. */
function reconcile(stored: unknown): UiPrefs {
  if (!stored || typeof stored !== "object") return { ...DEFAULTS };
  const raw = stored as Record<string, unknown>;

  const number = (value: unknown, fallback: number) =>
    typeof value === "number" && Number.isFinite(value) ? value : fallback;
  const boolean = (value: unknown, fallback: boolean) =>
    typeof value === "boolean" ? value : fallback;

  const widths = (raw.paneWidths ?? {}) as Record<string, unknown>;
  const accent = ACCENTS.some((option) => option.key === raw.accent)
    ? (raw.accent as Accent)
    : DEFAULTS.accent;
  const paneMode = PANE_MODES.some((option) => option.key === raw.paneMode)
    ? (raw.paneMode as PaneMode)
    : DEFAULTS.paneMode;

  return {
    navWidth: clamp(number(raw.navWidth, DEFAULTS.navWidth), NAV_MIN, NAV_MAX),
    navFolded: boolean(raw.navFolded, DEFAULTS.navFolded),
    paneOpen: boolean(raw.paneOpen, DEFAULTS.paneOpen),
    paneMode,
    paneWidths: {
      preview: number(widths.preview, PANE_DEFAULT.preview),
      grid: number(widths.grid, PANE_DEFAULT.grid),
      folders: number(widths.folders, PANE_DEFAULT.folders),
    },
    bandExpanded: boolean(raw.bandExpanded, DEFAULTS.bandExpanded),
    detailsExpanded: boolean(raw.detailsExpanded, DEFAULTS.detailsExpanded),
    accent,
    tileHeight: number(raw.tileHeight, DEFAULTS.tileHeight),
  };
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

/** Writes are debounced: a drag-resize fires on every mouse move, and this
 *  ends up in a file next to the exe. */
const SAVE_DEBOUNCE_MS = 400;

function useUiState(): UiState {
  const [prefs, setPrefs] = useState<UiPrefs>(DEFAULTS);
  const [loaded, setLoaded] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const stored = await ipc.uiPrefs();
        if (!cancelled) setPrefs(reconcile(stored));
      } catch {
        // A preferences file that cannot be read is not worth a message —
        // the defaults are a perfectly good interface.
      } finally {
        if (!cancelled) setLoaded(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // One hue, swapped on the root — decision 24. Everything else reads
  // `--color-accent` and needs to know nothing about which one is on.
  useEffect(() => {
    document.documentElement.dataset.accent = prefs.accent;
  }, [prefs.accent]);

  useEffect(() => {
    if (!loaded) return;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void (async () => {
        try {
          await ipc.setUiPrefs(prefs);
        } catch {
          // Same reasoning: losing a panel width is not worth interrupting
          // for. Caught rather than left to reject, because this fires from a
          // timer with nothing above it to catch anything.
        }
      })();
    }, SAVE_DEBOUNCE_MS);
    return () => window.clearTimeout(timer.current);
  }, [prefs, loaded]);

  const set = useCallback(
    <K extends keyof UiPrefs>(key: K, value: UiPrefs[K]) => {
      setPrefs((current) => ({ ...current, [key]: value }));
    },
    [],
  );

  const setPaneWidth = useCallback((mode: PaneMode, width: number) => {
    setPrefs((current) => ({
      ...current,
      paneWidths: { ...current.paneWidths, [mode]: Math.max(width, PANE_MIN) },
    }));
  }, []);

  const resetNavWidth = useCallback(() => {
    setPrefs((current) => ({ ...current, navWidth: NAV_DEFAULT }));
  }, []);

  const resetPaneWidth = useCallback(() => {
    setPrefs((current) => ({
      ...current,
      paneWidths: {
        ...current.paneWidths,
        [current.paneMode]: PANE_DEFAULT[current.paneMode],
      },
    }));
  }, []);

  return useMemo(
    () => ({
      ...prefs,
      loaded,
      set,
      setPaneWidth,
      paneWidth: prefs.paneWidths[prefs.paneMode],
      resetNavWidth,
      resetPaneWidth,
    }),
    [prefs, loaded, set, setPaneWidth, resetNavWidth, resetPaneWidth],
  );
}

const UiContext = createContext<UiState | null>(null);

export function UiProvider({ children }: { children: ReactNode }) {
  const value = useUiState();
  return createElement(UiContext.Provider, { value }, children);
}

export function useUi(): UiState {
  const value = useContext(UiContext);
  if (!value) throw new Error("useUi must be used inside <UiProvider>");
  return value;
}
