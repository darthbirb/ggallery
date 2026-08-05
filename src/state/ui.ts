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
 * What the pane holds. All three modes keep a remembered width — DESIGN.md §2
 * says widths are per mode, and Settings lists all three so the numbers are
 * already there when the modes arrive.
 */
export type PaneMode = "preview" | "grid" | "folders";

export const PANE_MODES: { key: PaneMode; label: string }[] = [
  { key: "preview", label: "Preview" },
  { key: "grid", label: "Grid" },
  { key: "folders", label: "Folders" },
];

/**
 * What the pane's mode control actually offers. All three modes are built
 * as of M2.5b — this list exists so a preferences file naming a mode that
 * is not (yet, or any more) built can never leave the pane rendering
 * nothing at all; see `reconcile`, below.
 */
export const AVAILABLE_PANE_MODES = PANE_MODES;

/** Below this a panel is not usable; the drag stops rather than collapsing. */
export const NAV_MIN = 150;
export const NAV_MAX = 420;
export const NAV_DEFAULT = 200;
/** Folded: an icon strip that keeps queue badges on screen. */
export const NAV_FOLDED = 44;
export const PANE_MIN = 260;
/** Closed, the pane folds to a strip of its mode icons — same width as the
 *  nav rail's own fold, `NAV_FOLDED`. */
export const PANE_STRIP_WIDTH = 44;

/** The filmstrip's height, dragged by the handle on its top edge. The floor
 *  is a usable thumbnail plus its scrollbar channel; the ceiling stops it
 *  eating the media it exists to navigate. */
export const FILMSTRIP_MIN = 52;
export const FILMSTRIP_MAX = 240;
export const FILMSTRIP_DEFAULT = 64;
/** **One** width, shared by every pane mode. It was per mode until M2.5a.1:
 *  in use, switching modes moving the split under you reads as the window
 *  losing its place, not as the app being helpful. */
export const PANE_DEFAULT = 460;

export interface UiPrefs {
  navWidth: number;
  navFolded: boolean;
  paneOpen: boolean;
  paneMode: PaneMode;
  paneWidth: number;
  filmstripHeight: number;
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
  paneWidth: PANE_DEFAULT,
  filmstripHeight: FILMSTRIP_DEFAULT,
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
  /** Clamped to `PANE_MIN` on the way in, so a drag cannot store a width the
   *  pane could never render at. */
  setPaneWidth: (width: number) => void;
  resetNavWidth: () => void;
  resetPaneWidth: () => void;
  resetFilmstripHeight: () => void;
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

  // M2.5a.1 collapsed three per-mode widths into one. A preferences file from
  // before that carries `paneWidths`; take Preview's, since it is the only
  // mode that was ever built, so nobody's split jumps on upgrade.
  const legacyWidths = (raw.paneWidths ?? {}) as Record<string, unknown>;
  const paneWidth = number(
    raw.paneWidth,
    number(legacyWidths.preview, DEFAULTS.paneWidth),
  );

  const accent = ACCENTS.some((option) => option.key === raw.accent)
    ? (raw.accent as Accent)
    : DEFAULTS.accent;
  // Only a mode that is actually built: a preferences file written by a later
  // build (or by M2.5b, then rolled back) must not leave the pane rendering
  // nothing at all.
  const paneMode = AVAILABLE_PANE_MODES.some((option) => option.key === raw.paneMode)
    ? (raw.paneMode as PaneMode)
    : DEFAULTS.paneMode;

  return {
    navWidth: clamp(number(raw.navWidth, DEFAULTS.navWidth), NAV_MIN, NAV_MAX),
    navFolded: boolean(raw.navFolded, DEFAULTS.navFolded),
    paneOpen: boolean(raw.paneOpen, DEFAULTS.paneOpen),
    paneMode,
    paneWidth: Math.max(paneWidth, PANE_MIN),
    filmstripHeight: clamp(
      number(raw.filmstripHeight, DEFAULTS.filmstripHeight),
      FILMSTRIP_MIN,
      FILMSTRIP_MAX,
    ),
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

  const setPaneWidth = useCallback((width: number) => {
    setPrefs((current) => ({ ...current, paneWidth: Math.max(width, PANE_MIN) }));
  }, []);

  const resetNavWidth = useCallback(() => {
    setPrefs((current) => ({ ...current, navWidth: NAV_DEFAULT }));
  }, []);

  const resetPaneWidth = useCallback(() => {
    setPrefs((current) => ({ ...current, paneWidth: PANE_DEFAULT }));
  }, []);

  const resetFilmstripHeight = useCallback(() => {
    setPrefs((current) => ({ ...current, filmstripHeight: FILMSTRIP_DEFAULT }));
  }, []);

  return useMemo(
    () => ({
      ...prefs,
      loaded,
      set,
      setPaneWidth,
      resetNavWidth,
      resetPaneWidth,
      resetFilmstripHeight,
    }),
    [
      prefs,
      loaded,
      set,
      setPaneWidth,
      resetNavWidth,
      resetPaneWidth,
      resetFilmstripHeight,
    ],
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
