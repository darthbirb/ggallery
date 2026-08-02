interface SettingsPanelProps {
  onClose: () => void;
  onNormaliseFilenames: () => void;
  onManageArchetypes: () => void;
  onManageStatuses: () => void;
  onManageTags: () => void;
}

/**
 * Deliberately minimal — the real Settings screen (command palette, keyboard
 * reference, blur toggle, backup verification) is M9's job. This exists now
 * only because M1.6 needs a permanent home for the repair-case action: see
 * docs/DESIGN.md#first-import, "Settings keeps a Normalise filenames action".
 * M2.1 adds the archetype/status/tag editors that removing the seeded
 * vocabulary (PLAN.md decision 21) makes mandatory.
 */
export function SettingsPanel({
  onClose,
  onNormaliseFilenames,
  onManageArchetypes,
  onManageStatuses,
  onManageTags,
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

          <h3 className="mb-2 mt-4 text-[11px] font-semibold uppercase tracking-wide text-fg-dim">
            Vocabulary
          </h3>
          <button
            type="button"
            onClick={onManageArchetypes}
            className="w-full rounded-[3px] border border-line px-3 py-2 text-left hover:bg-hover"
          >
            <span className="block text-fg">Archetypes</span>
            <span className="block text-[12px] text-fg-dim">
              Create and edit the folder templates used across your library.
            </span>
          </button>

          <button
            type="button"
            onClick={onManageStatuses}
            className="mt-2 w-full rounded-[3px] border border-line px-3 py-2 text-left hover:bg-hover"
          >
            <span className="block text-fg">Folder statuses</span>
            <span className="block text-[12px] text-fg-dim">
              Rename, recolour, reorder, add or remove status values.
            </span>
          </button>

          <button
            type="button"
            onClick={onManageTags}
            className="mt-2 w-full rounded-[3px] border border-line px-3 py-2 text-left hover:bg-hover"
          >
            <span className="block text-fg">Tags</span>
            <span className="block text-[12px] text-fg-dim">
              Rename or delete a tag across the whole library.
            </span>
          </button>
        </div>
      </div>
    </div>
  );
}
