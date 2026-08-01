import { useEffect, useRef, useState } from "react";

import { Grid } from "./features/grid/Grid";
import { FailureList } from "./features/indexing/FailureList";
import { IndexStatus } from "./features/indexing/IndexStatus";
import { ImportWizard } from "./features/import/ImportWizard";
import { SettingsPanel } from "./features/settings/SettingsPanel";
import { Sidebar } from "./features/sidebar/Sidebar";
import { formatCount } from "./lib/format";
import * as ipc from "./lib/ipc";
import { useLibrary } from "./state/library";
import { TILE_SIZES, useUi } from "./state/ui";

type ImportWizardMode = "auto" | "manual";

export default function App() {
  const library = useLibrary();
  const ui = useUi();
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [showFailures, setShowFailures] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [importWizardMode, setImportWizardMode] =
    useState<ImportWizardMode | null>(null);

  // A run that fixed everything should not leave the panel open on nothing.
  useEffect(() => {
    if (library.failures.length === 0) setShowFailures(false);
  }, [library.failures.length]);

  // The wizard is a step in opening a library, not a permanent control — see
  // docs/DESIGN.md#first-import. Once the index has settled, check whether
  // this library has ever been imported; if it needs the ceremony, offer it;
  // if there is nothing to rename at all (an empty library, or one already
  // all UUID-named), just stamp it imported and never ask again. Guarded to
  // run once per opened library.
  const checkedImportFor = useRef<string | null>(null);
  // A brand-new library's very first progress read is "idle" too — nothing
  // has been queued yet, before the walk even starts — indistinguishable
  // from "idle because indexing finished" by phase alone. For a library that
  // started empty, an idle reading is only trustworthy once a busy phase has
  // been seen for it first; a library that already had items at open time
  // never had that race, so its first idle reading is trusted immediately.
  const everBusyFor = useRef<string | null>(null);
  useEffect(() => {
    const root = library.info?.root ?? null;
    if (!root || !library.progress) return;

    if (library.progress.phase !== "idle") {
      everBusyFor.current = root;
      return;
    }
    const trustworthy = library.info!.itemCount > 0 || everBusyFor.current === root;
    if (!trustworthy || checkedImportFor.current === root) return;
    checkedImportFor.current = root;

    (async () => {
      try {
        const scan = await ipc.scanImport();
        if (scan.importedAt !== null) return;
        if (scan.toRename === 0) {
          await ipc.markImported();
          return;
        }
        setImportWizardMode("auto");
      } catch {
        // Best-effort — a failed check here should not block using the app.
      }
    })();
  }, [library.info, library.progress]);

  if (!library.info) {
    return <Welcome library={library} />;
  }

  const { info, progress, scope } = library;
  const folder = scope.folder
    ? library.folders.find((node) => node.relPath === scope.folder)
    : null;

  return (
    <div className="grid h-full grid-rows-[38px_1fr] bg-ground text-fg">
      <header className="flex items-center gap-3 border-b border-line bg-panel px-3">
        <span className="font-semibold">{info.name}</span>
        <span
          className="truncate font-mono text-[11px] text-fg-dim"
          title={info.root}
        >
          {info.root}
        </span>
        <button
          type="button"
          onClick={library.choose}
          title="Open a different library folder"
          className="shrink-0 rounded-[3px] border border-line px-2 py-0.5 text-fg-dim hover:bg-hover hover:text-fg"
        >
          Change…
        </button>

        <span className="ml-auto flex items-center gap-3">
          <IndexStatus
            progress={progress}
            failureCount={library.failures.length}
            showingFailures={showFailures}
            onToggleFailures={() => setShowFailures((open) => !open)}
          />

          <label className="flex items-center gap-1.5 text-fg-dim">
            size
            <input
              type="range"
              min={0}
              max={TILE_SIZES.length - 1}
              value={TILE_SIZES.indexOf(ui.tileHeight)}
              onChange={(event) =>
                ui.setTileHeight(TILE_SIZES[Number(event.target.value)])
              }
              className="w-20 accent-accent"
            />
          </label>

          <button
            type="button"
            onClick={library.reindex}
            disabled={progress?.phase !== "idle" && progress !== null}
            className="rounded-[3px] border border-line px-2 py-0.5 text-fg-mid hover:bg-hover hover:text-fg disabled:opacity-40"
          >
            {progress && progress.phase !== "idle" ? "Indexing…" : "Re-index"}
          </button>

          <button
            type="button"
            onClick={() => setShowSettings(true)}
            title="Settings"
            className="rounded-[3px] border border-line px-2 py-0.5 text-fg-mid hover:bg-hover hover:text-fg"
          >
            Settings…
          </button>
        </span>
      </header>

      {importWizardMode && (
        <ImportWizard
          title={importWizardMode === "manual" ? "Normalise filenames" : undefined}
          dismissable={importWizardMode === "manual"}
          onClose={() => setImportWizardMode(null)}
        />
      )}

      {showSettings && (
        <SettingsPanel
          onClose={() => setShowSettings(false)}
          onNormaliseFilenames={() => {
            setShowSettings(false);
            setImportWizardMode("manual");
          }}
        />
      )}

      <div className="grid min-h-0 grid-cols-[214px_1fr]">
        <Sidebar
          folders={library.folders}
          selected={scope.folder}
          onSelect={(relPath) =>
            library.setScope({ folder: relPath, recursive: scope.recursive })
          }
        />

        <main className="flex min-h-0 min-w-0 flex-col">
          <div className="flex items-center gap-3 border-b border-line bg-panel px-3 py-2">
            <span className="text-[14px] font-semibold">
              {folder ? folder.title : "All media"}
            </span>
            <span className="font-mono text-[11px] tabular-nums text-fg-dim">
              {formatCount(library.items.length)} items
            </span>

            {scope.folder && (
              <label className="ml-auto flex items-center gap-1.5 text-fg-mid">
                <input
                  type="checkbox"
                  checked={!scope.recursive}
                  onChange={(event) =>
                    library.setScope({
                      folder: scope.folder,
                      recursive: !event.target.checked,
                    })
                  }
                  className="accent-accent"
                />
                this folder only
              </label>
            )}
          </div>

          {library.error && (
            <div className="flex items-center gap-3 border-b border-line bg-raised px-3 py-1.5 text-danger">
              {library.error}
              <button
                type="button"
                onClick={library.dismissError}
                className="ml-auto text-fg-dim hover:text-fg"
              >
                dismiss
              </button>
            </div>
          )}

          {showFailures && library.failures.length > 0 && (
            <FailureList
              failures={library.failures}
              onRetry={library.retry}
              onClose={() => setShowFailures(false)}
            />
          )}

          {!info.ffmpeg && (
            <div className="border-b border-line bg-raised px-3 py-1.5 text-[12px] text-accent">
              No ffmpeg found in <span className="font-mono">tools/</span> or on
              PATH — videos are indexed, but get no poster frame and no scrub
              strip until one is available.
            </div>
          )}

          <Grid
            items={library.items}
            thumbsDir={info.thumbsDir}
            spritesDir={info.spritesDir}
            tileHeight={ui.tileHeight}
            selectedId={selectedId}
            onSelect={setSelectedId}
            refreshToken={library.refreshToken}
          />
        </main>
      </div>
    </div>
  );
}

function Welcome({ library }: { library: ReturnType<typeof useLibrary> }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 bg-ground px-8 text-center">
      <h1 className="text-[22px] font-semibold tracking-tight">GGallery</h1>
      <p className="max-w-[42ch] text-fg-mid">
        Choose the folder that holds your media. Everything the app writes goes
        into a <span className="font-mono text-fg">.gallery</span> folder inside
        it; nothing else in there is touched, renamed or moved.
      </p>

      <button
        type="button"
        onClick={library.choose}
        disabled={library.loading}
        className="rounded-[5px] border border-accent-d bg-raised px-4 py-1.5 text-accent hover:bg-hover disabled:opacity-40"
      >
        {library.loading ? "Opening…" : "Choose library folder"}
      </button>

      {library.remembered && (
        <button
          type="button"
          onClick={() => library.open(library.remembered as string)}
          className="font-mono text-[11px] text-fg-dim underline hover:text-fg"
        >
          reopen {library.remembered}
        </button>
      )}

      {library.error && <p className="text-danger">{library.error}</p>}
    </div>
  );
}
