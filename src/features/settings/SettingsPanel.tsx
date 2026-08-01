interface SettingsPanelProps {
  onClose: () => void;
  onNormaliseFilenames: () => void;
}

/**
 * Deliberately minimal — the real Settings screen (command palette, keyboard
 * reference, blur toggle, backup verification) is M9's job. This exists now
 * only because M1.6 needs a permanent home for the repair-case action: see
 * docs/DESIGN.md#first-import, "Settings keeps a Normalise filenames action".
 */
export function SettingsPanel({
  onClose,
  onNormaliseFilenames,
}: SettingsPanelProps) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-[420px] rounded-[6px] border border-line bg-panel shadow-xl">
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <span className="text-[14px] font-semibold">Settings</span>
          <button
            type="button"
            onClick={onClose}
            className="ml-auto rounded-[3px] px-1.5 text-fg-dim hover:bg-hover hover:text-fg"
          >
            ✕
          </button>
        </header>

        <div className="p-4">
          <h3 className="mb-2 text-[11px] font-semibold uppercase tracking-wide text-fg-dim">
            Library
          </h3>
          <button
            type="button"
            onClick={onNormaliseFilenames}
            className="w-full rounded-[3px] border border-line px-3 py-2 text-left hover:bg-hover"
          >
            <span className="block text-fg">Normalise filenames</span>
            <span className="block text-[12px] text-fg-dim">
              Find any file that is not UUID-named and rename it — for when
              something outside the app has renamed a file back.
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
