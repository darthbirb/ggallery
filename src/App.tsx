/**
 * The shell: one split, grid on the left, pane on the right.
 *
 * The window is our own bar (decorations off — decision 28), a navigation
 * panel that folds, the grid, and a pane that resizes and closes. There is
 * no theatre view — full-window is the pane maximised (docs/DESIGN.md §2).
 */

import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";

import { Resizer } from "./components/Resizer";
import { Toaster, ToastProviderRoot } from "./components/Toaster";
import { TooltipProvider } from "./components/Tooltip";
import { WindowBar } from "./components/WindowBar";
import { Button } from "./components/ui/button";
import { Separator } from "./components/ui/separator";
import { KitchenSink } from "./dev/KitchenSink";
import { FolderBand } from "./features/folder/FolderBand";
import { Grid } from "./features/grid/Grid";
import { SCRUBBER_WIDTH } from "./features/grid/Scrubber";
import { FailureList } from "./features/indexing/FailureList";
import { NormaliseFilenamesModal } from "./features/import/NormaliseFilenamesModal";
import { ProgressScreen } from "./features/import/ProgressScreen";
import { ReviewScreen } from "./features/import/ReviewScreen";
import { DialogsProvider, useDialogs } from "./features/menus/Dialogs";
import { EmptyMenu, ItemMenu } from "./features/menus/ItemMenu";
import { OperationsProvider, useOperations } from "./features/menus/operations";
import { Nav } from "./features/nav/Nav";
import { Pane, PaneStrip } from "./features/pane/Pane";
import type { PreviewSlot } from "./features/pane/PreviewMode";
import { SettingsPanel } from "./features/settings/SettingsPanel";
import { formatCount } from "./lib/format";
import * as ipc from "./lib/ipc";
import type { ArchetypeInfo, FolderNode, FolderStatusDef } from "./lib/types";
import { cn } from "./lib/utils";
import { useLibrary, type LibraryController, type Scope } from "./state/library";
import { useSelection, type SelectionController } from "./state/selection";
import { ToastProvider, useToasts } from "./state/toasts";
import { NAV_FOLDED, NAV_MAX, NAV_MIN, PANE_MIN, UiProvider, useUi } from "./state/ui";

/** Tracks `location.hash`, live. Always called — never conditionally — so
 *  the `import.meta.env.DEV` check below stays a plain literal at its call
 *  site instead of being buried behind this hook's own return value, which
 *  is what a bundler actually needs to fold `<KitchenSink />` away. */
function useHash(): string {
  const [hash, setHash] = useState(() => window.location.hash);

  useEffect(() => {
    const onHashChange = () => setHash(window.location.hash);
    window.addEventListener("hashchange", onHashChange);
    return () => window.removeEventListener("hashchange", onHashChange);
  }, []);

  return hash;
}

export default function App() {
  const hash = useHash();

  // Dev-only escape hatch to `dev/KitchenSink.tsx` — see docs/STRUCTURE.md.
  // Checked before any of the app's own providers mount, so the route needs
  // no library, no config file and no IPC round-trip to render. The literal
  // `import.meta.env.DEV` has to sit directly in this expression, not behind
  // another function's return value — Vite replaces it with a compile-time
  // `false` in release builds, and that only lets Rollup fold the whole
  // branch (and drop the now-unreachable `KitchenSink` import with it) when
  // it can see the literal at the point `<KitchenSink />` is written.
  if (import.meta.env.DEV && hash === "#kitchen-sink") return <KitchenSink />;

  return (
    <UiProvider>
      <ToastProvider>
        <TooltipProvider>
          <ToastProviderRoot>
            <Shell />
            <Toaster />
          </ToastProviderRoot>
        </TooltipProvider>
      </ToastProvider>
    </UiProvider>
  );
}

/**
 * Owns the window bar for every state the app can be in — decorations are
 * off globally, so a screen with no bar has no way to drag, minimise or
 * close the window at all, first-run flows included. Each branch below just
 * fills the space beneath it.
 */
function Shell() {
  const library = useLibrary();
  const selection = useSelection(library.items);
  const [backupConfirmed, setBackupConfirmed] = useState(false);
  const [starting, setStarting] = useState(false);

  // Once the rename actually starts, `flowPhase` takes over as the Progress
  // screen — reset the local "starting" flag so it's fresh if the flow is
  // ever re-entered.
  useEffect(() => {
    if (library.flowPhase !== "idle") setStarting(false);
  }, [library.flowPhase]);

  let content: ReactNode;

  // Choose folder → Review → Progress → Gallery, as full-window screens —
  // see docs/DESIGN.md#first-import.
  if (library.flowPhase !== "idle") {
    content = (
      <ProgressScreen
        phase={library.flowPhase}
        renameProgress={library.renameProgress}
        indexProgress={library.progress}
      />
    );
  } else if (library.pendingReview) {
    content = (
      <ReviewScreen
        path={library.pendingReview.path}
        report={library.pendingReview.report}
        confirmed={backupConfirmed}
        onConfirmedChange={setBackupConfirmed}
        busy={starting}
        error={library.error}
        onCancel={() => {
          setBackupConfirmed(false);
          library.cancelImport();
        }}
        onImport={() => {
          setStarting(true);
          library.confirmImport(backupConfirmed);
          setBackupConfirmed(false);
        }}
      />
    );
  } else if (!library.info) {
    content = <Welcome library={library} />;
  } else {
    content = (
      <OperationsProvider library={library} selection={selection}>
        <DialogsProvider folders={library.folders}>
          <Gallery library={library} selection={selection} />
        </DialogsProvider>
      </OperationsProvider>
    );
  }

  return (
    <div className="flex h-full flex-col bg-ground text-fg">
      <WindowBar />
      <div className="min-h-0 flex-1">{content}</div>
    </div>
  );
}

function Gallery({
  library,
  selection,
}: {
  library: LibraryController;
  selection: SelectionController;
}) {
  const ui = useUi();
  const ops = useOperations();
  const dialogs = useDialogs();
  const toasts = useToasts();

  const [showFailures, setShowFailures] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showNormalise, setShowNormalise] = useState(false);
  const [maximised, setMaximised] = useState(false);

  const [statuses, setStatuses] = useState<FolderStatusDef[]>([]);
  const [archetypes, setArchetypes] = useState<ArchetypeInfo[]>([]);

  // The width transitions below are for folding and maximising, not for a
  // live drag — applying them unconditionally made every mousemove during a
  // resize queue another 180ms-eased animation, so the edge visibly trailed
  // the cursor instead of tracking it.
  const [navResizing, setNavResizing] = useState(false);
  const [paneResizing, setPaneResizing] = useState(false);

  const info = library.info!;
  const { scope } = library;

  // A run that fixed everything should not leave the panel open on nothing.
  useEffect(() => {
    if (library.failures.length === 0) setShowFailures(false);
  }, [library.failures.length]);

  const loadVocabulary = useCallback(() => {
    void Promise.all([ipc.listFolderStatuses(), ipc.listArchetypes()])
      .then(([nextStatuses, nextArchetypes]) => {
        setStatuses(nextStatuses);
        setArchetypes(nextArchetypes);
      })
      .catch(() => {
        // Nothing to say: the menus that use these simply stay empty, which
        // is also the correct state for a library with no archetypes.
      });
  }, []);

  useEffect(loadVocabulary, [loadVocabulary]);

  const folder = useMemo(
    () =>
      scope.kind === "folder"
        ? (library.folders.find((node) => node.relPath === scope.folder) ?? null)
        : null,
    [scope, library.folders],
  );

  // What the band's title slot shows for a scope with no folder identity to
  // expand into. A real folder never falls through to this.
  const scopeLabel =
    scope.kind === "everything"
      ? "Everything"
      : scope.kind === "sorting"
        ? "Sorting Box"
        : scope.kind === "favourites"
          ? "Favourites"
          : "";

  const favouriteCount = useMemo(
    () => library.items.filter((item) => item.favorite).length,
    [library.items],
  );

  // The Sorting Box *is* the library root (DESIGN.md §2 and §4), so its count
  // is the root folder's own items — not a subfolder's, and not a walk.
  const sortingCount = useMemo(
    () => library.folders.find((node) => node.parentId === null)?.directCount ?? 0,
    [library.folders],
  );

  const openFolder = useCallback(
    (node: FolderNode) =>
      library.setScope({ kind: "folder", folder: node.relPath, recursive: true }),
    [library],
  );

  const editFolderDetails = useCallback(
    (node: FolderNode) => {
      openFolder(node);
      ui.set("bandExpanded", true);
    },
    [openFolder, ui],
  );

  const showInPane = useCallback(
    (itemId: number) => {
      selection.focus(itemId);
      ui.set("paneMode", "preview");
      if (!ui.paneOpen) ui.set("paneOpen", true);
    },
    [selection, ui],
  );

  // One slot today. M6 and M7 pass two, M10 up to twelve — the pane's Preview
  // mode already lays out any number of them.
  const slots: PreviewSlot[] = useMemo(
    () => [{ key: "primary", itemId: selection.current }],
    [selection.current],
  );

  // Keys are a second path to something already on screen — never the only
  // one (locked decision 23). Every binding here has a visible control: the
  // right-click menus, the pane header, the navigation panel's fold button.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable)
      ) {
        return;
      }

      const ids = [...selection.selected];

      if (event.ctrlKey && event.key.toLowerCase() === "z") {
        event.preventDefault();
        // An accelerator over the toast's Undo button, not the journal stack
        // replayer — that is M4's, and reaches operations from previous
        // sessions. This reverses the most recent thing still on screen.
        const undoable = [...toasts.toasts].reverse().find((toast) => toast.undo);
        if (undoable) void toasts.runUndo(undoable.id);
        return;
      }
      if (event.ctrlKey && event.key.toLowerCase() === "a") {
        event.preventDefault();
        selection.selectAll();
        return;
      }
      if (event.ctrlKey && event.key.toLowerCase() === "c" && ids.length === 1) {
        event.preventDefault();
        void ops.copyItemFile(ids[0]);
        return;
      }
      if (event.key === "Escape") {
        if (maximised) setMaximised(false);
        else selection.clear();
        return;
      }
      if (event.key === "Delete" && ids.length > 0) {
        event.preventDefault();
        dialogs.deleteItems(ids);
        return;
      }
      if (event.key.toLowerCase() === "f" && ids.length > 0 && !event.ctrlKey) {
        event.preventDefault();
        const current = library.items.find((item) => item.id === selection.current);
        void ops.setFavorite(ids, !(current?.favorite ?? false));
        return;
      }
      if (event.key === "ArrowRight") {
        event.preventDefault();
        selection.step(1);
        return;
      }
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        selection.step(-1);
      }
    };

    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selection, ops, dialogs, toasts, library.items, maximised]);

  const pane = (
    <Pane
      mode={ui.paneMode}
      onModeChange={(mode) => ui.set("paneMode", mode)}
      onClose={() => {
        ui.set("paneOpen", false);
        setMaximised(false);
      }}
      maximised={maximised}
      onMaximisedChange={setMaximised}
      slots={slots}
      items={library.items}
      folders={library.folders}
      thumbsDir={info.thumbsDir}
      onStep={(delta) => selection.step(delta)}
      onPick={(itemId) => selection.focus(itemId)}
      detailsExpanded={ui.detailsExpanded}
      onDetailsExpandedChange={(expanded) => ui.set("detailsExpanded", expanded)}
      filmstripHeight={ui.filmstripHeight}
      onFilmstripHeightChange={(height) => ui.set("filmstripHeight", height)}
      onResetFilmstripHeight={ui.resetFilmstripHeight}
      refreshToken={library.refreshToken}
    />
  );

  return (
    <div className="flex h-full flex-col bg-ground text-fg">
      <div className="flex min-h-0 min-w-0 flex-1">
        {/* Maximising is "fill the window", not "unmount everything else" —
            it used to be the latter, which is why it never animated: there
            was nothing to tween between, just an instant swap. This region
            (nav + the grid side) now stays mounted and tweens its share of
            the row to zero via `flex-grow`, fading out as it goes; the pane
            below tweens the opposite way. `inert` while collapsed keeps it
            out of tab order and off screen readers without the layout jump
            `display: none` would cause mid-transition. */}
        <div
          inert={maximised}
          style={{ flexGrow: maximised ? 0 : 1, flexShrink: maximised ? 0 : 1 }}
          className={cn(
            "flex min-h-0 min-w-0 basis-0 overflow-hidden transition-[flex-grow,opacity] duration-[180ms] ease-out",
            maximised ? "opacity-0" : "opacity-100",
          )}
        >
          {/* One wrapper for both states, so folding tweens this width
              rather than swapping two differently-sized panels — decision
              27, "the navigation panel fold". `Nav` itself still swaps
              which content it renders (an icon strip is not a narrowed copy
              of the full panel), but that content now fills whatever width
              it is given instead of sizing itself, and fades in under the
              width change rather than popping. */}
          <div
            style={{ width: ui.navFolded ? NAV_FOLDED : ui.navWidth }}
            className={cn(
              "flex min-h-0 shrink-0 flex-col overflow-hidden",
              !navResizing && "transition-[width] duration-[180ms] ease-out",
            )}
          >
            <Nav
              folders={library.folders}
              scope={scope}
              onScope={library.setScope}
              statuses={statuses}
              archetypes={archetypes}
              folded={ui.navFolded}
              onFoldedChange={(folded) => ui.set("navFolded", folded)}
              onEditDetails={editFolderDetails}
              favouriteCount={favouriteCount}
              sortingCount={sortingCount}
              progress={library.progress}
              failureCount={library.failures.length}
              showingFailures={showFailures}
              onToggleFailures={() => setShowFailures((open) => !open)}
              onOpenSettings={() => setShowSettings(true)}
            />
          </div>
          {!ui.navFolded && (
            <Resizer
              label="Navigation panel width"
              side="left"
              value={ui.navWidth}
              min={NAV_MIN}
              max={NAV_MAX}
              onChange={(width) => ui.set("navWidth", width)}
              onReset={ui.resetNavWidth}
              onDraggingChange={setNavResizing}
            />
          )}

          <main className="flex min-h-0 min-w-0 flex-1 flex-col">
            <FolderBand
              folder={folder}
              scopeLabel={scopeLabel}
              itemCount={library.items.length}
              statuses={statuses}
              archetypes={archetypes}
              expanded={ui.bandExpanded}
              onExpandedChange={(expanded) => ui.set("bandExpanded", expanded)}
              thumbsDir={info.thumbsDir}
              refreshToken={library.refreshToken}
              onOpen={openFolder}
              tileHeight={ui.tileHeight}
              onTileHeightChange={(height) => ui.set("tileHeight", height)}
              recursive={scope.kind === "folder" ? scope.recursive : true}
              onRecursiveChange={(recursive) => {
                if (scope.kind === "folder") {
                  library.setScope({ kind: "folder", folder: scope.folder, recursive });
                }
              }}
            />

            <Banners
              library={library}
              showFailures={showFailures}
              onCloseFailures={() => setShowFailures(false)}
            />

            <Grid
              items={library.items}
              thumbsDir={info.thumbsDir}
              spritesDir={info.spritesDir}
              tileHeight={ui.tileHeight}
              selection={selection}
              refreshToken={library.refreshToken}
              onActivate={showInPane}
              empty={emptyLabel(scope)}
              renderMenu={(target) =>
                target.itemId === null ? (
                  <EmptyMenu
                    folder={folder}
                    hasItems={library.items.length > 0}
                    hasSelection={selection.count > 0}
                    onSelectAll={selection.selectAll}
                    onInvert={selection.invert}
                    onClear={selection.clear}
                    onNewFolder={() => dialogs.newFolder(folder)}
                    bandExpanded={ui.bandExpanded}
                    onToggleBand={() => ui.set("bandExpanded", !ui.bandExpanded)}
                    paneOpen={ui.paneOpen}
                    onTogglePane={() => ui.set("paneOpen", !ui.paneOpen)}
                  />
                ) : (
                  <ItemMenu
                    itemIds={
                      selection.count > 1 && selection.isSelected(target.itemId)
                        ? [...selection.selected]
                        : [target.itemId]
                    }
                    item={
                      library.items.find((item) => item.id === target.itemId) ?? null
                    }
                    folder={folder}
                    onPreview={showInPane}
                  />
                )
              }
            />

            {/* The selection bar, rebuilt on the primitives — and the
                scrubber's width is reserved beside it rather than run
                underneath, so the strip's line reaches the bottom edge
                (DESIGN.md §2 "Timeline scrubber").

                *Revert* is gone from the bar. Inverting a selection lives
                in the right-click menu, where the rest of the selection
                operations already are, so nothing is lost — the bar carries
                the two destructive-adjacent actions and a count. */}
            {selection.count > 0 && (
              <div className="flex h-11 shrink-0 border-t border-line bg-panel">
                <footer className="flex h-full min-w-0 flex-1 items-center gap-2 px-3">
                  <span className="font-mono tabular-nums text-fg">
                    {formatCount(selection.count)} selected
                  </span>
                  <Separator />
                  <Button size="sm" onClick={selection.selectAll}>
                    Select all
                  </Button>
                  <Button size="sm" onClick={selection.clear}>
                    Clear
                  </Button>
                  <Separator />
                  <Button
                    size="sm"
                    onClick={() => dialogs.moveItems([...selection.selected])}
                  >
                    Move to…
                  </Button>
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => dialogs.deleteItems([...selection.selected])}
                  >
                    Delete
                  </Button>
                  <span className="ml-auto truncate pl-2 text-fg-dim">
                    Right-click for more
                  </span>
                </footer>
                {/* The scrubber's channel, continued to the window edge, so
                    the groove does not stop short above the bar. */}
                <div
                  aria-hidden
                  style={{ width: SCRUBBER_WIDTH }}
                  className="scrubber shrink-0"
                />
              </div>
            )}
          </main>
        </div>

        {ui.paneOpen ? (
          <>
            {!maximised && (
              <Resizer
                label="Pane width"
                side="right"
                value={ui.paneWidth}
                min={PANE_MIN}
                max={1200}
                onChange={(width) => ui.setPaneWidth(width)}
                onReset={ui.resetPaneWidth}
                onDraggingChange={setPaneResizing}
              />
            )}

            <div
              style={{
                flexGrow: maximised ? 1 : 0,
                flexShrink: maximised ? 1 : 0,
                flexBasis: maximised ? "0%" : `${ui.paneWidth}px`,
              }}
              className={cn(
                "flex min-h-0 overflow-hidden",
                !paneResizing && "transition-[flex-grow,flex-basis] duration-[180ms] ease-out",
              )}
            >
              {pane}
            </div>
          </>
        ) : (
          // No "Open pane" button — a closed pane folds to a strip of its
          // mode icons, the same gesture the nav rail uses for its own fold.
          <PaneStrip
            mode={ui.paneMode}
            onOpen={(mode) => {
              ui.set("paneMode", mode);
              ui.set("paneOpen", true);
            }}
          />
        )}
      </div>

      {showSettings && (
        <SettingsPanel
          libraryRoot={info.root}
          onChooseLibrary={library.choose}
          onClose={() => setShowSettings(false)}
          onNormaliseFilenames={() => {
            setShowSettings(false);
            setShowNormalise(true);
          }}
          onArchetypesChanged={() => {
            loadVocabulary();
            library.refreshFolders();
          }}
          onStatusesChanged={() => {
            loadVocabulary();
            library.refreshFolders();
          }}
          onTagsChanged={library.reload}
        />
      )}

      {showNormalise && (
        <NormaliseFilenamesModal onClose={() => setShowNormalise(false)} />
      )}
    </div>
  );
}

function emptyLabel(scope: Scope): string {
  switch (scope.kind) {
    case "favourites":
      return "Nothing favourited yet.";
    case "sorting":
      // The library root *is* the Sorting Box, so empty means filed, not
      // missing — DESIGN.md §4.
      return "Nothing to sort.";
    default:
      return "Nothing here yet.";
  }
}

/** Index errors, verification problems and the missing-ffmpeg notice. */
function Banners({
  library,
  showFailures,
  onCloseFailures,
}: {
  library: LibraryController;
  showFailures: boolean;
  onCloseFailures: () => void;
}) {
  const info = library.info;
  return (
    <>
      {library.error && (
        <div className="flex items-center gap-3 border-b border-line bg-raised px-3 py-2 text-danger">
          {library.error}
          <Button size="sm" className="ml-auto" onClick={library.dismissError}>
            Dismiss
          </Button>
        </div>
      )}

      {library.verifyIssue && (
        <div className="flex items-center gap-3 border-b border-line bg-raised px-3 py-2 text-danger">
          Import verification found a problem:{" "}
          {formatCount(library.verifyIssue.countRenamed)} of{" "}
          {formatCount(library.verifyIssue.countTotal)} items carry a UUID name
          {library.verifyIssue.mismatches.length > 0 && (
            <>
              , {formatCount(library.verifyIssue.mismatches.length)} sampled file
              {library.verifyIssue.mismatches.length === 1 ? "" : "s"} did not match
              its recorded hash
            </>
          )}
          {library.verifyIssue.missing.length > 0 && (
            <>
              , {formatCount(library.verifyIssue.missing.length)} sampled file
              {library.verifyIssue.missing.length === 1 ? "" : "s"} could not be found
              at its new path
            </>
          )}
          .
          <Button size="sm" className="ml-auto" onClick={library.dismissVerifyIssue}>
            Dismiss
          </Button>
        </div>
      )}

      {showFailures && library.failures.length > 0 && (
        <FailureList
          failures={library.failures}
          onRetry={library.retry}
          onClose={onCloseFailures}
        />
      )}

      {info && !info.ffmpeg && (
        <div className="border-b border-line bg-raised px-3 py-2 text-[13px] text-fg-mid">
          No ffmpeg in <span className="font-mono">tools/</span> or on PATH — videos
          index, but get no poster frame or scrub strip.
        </div>
      )}
    </>
  );
}

function Welcome({ library }: { library: LibraryController }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-5 bg-ground px-8 text-center">
      <h1 className="text-[24px] font-semibold tracking-tight">GGallery</h1>
      <p className="max-w-[46ch] leading-relaxed text-fg-mid">
        Choose the folder that holds your media. Everything the app writes goes into a{" "}
        <span className="font-mono text-fg">.gallery</span> folder inside it.
      </p>

      <Button
        variant="accent"
        size="lg"
        onClick={library.choose}
        disabled={library.loading}
      >
        {library.loading ? "Opening…" : "Choose library folder"}
      </Button>

      {library.remembered && (
        <Button size="sm" onClick={() => library.open(library.remembered as string)}>
          <span className="font-mono">reopen {library.remembered}</span>
        </Button>
      )}

      {library.error && <p className="text-danger">{library.error}</p>}
    </div>
  );
}
