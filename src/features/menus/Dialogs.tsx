/**
 * The dialogs the right-click menus open, hosted once at the top of the app.
 *
 * A menu unmounts the moment an item is chosen, so a dialog rendered inside
 * one would disappear with it. Menus ask for a dialog through `useDialogs()`;
 * this host owns the state and renders it.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { ConfirmDialog, Dialog } from "../../components/Dialog";
import * as ipc from "../../lib/ipc";
import type { ArchetypeInfo, FolderNode } from "../../lib/types";
import { useOperations } from "./operations";

export interface Dialogs {
  /** Create a folder inside `parent` (null being the top level). */
  newFolder: (parent: FolderNode | null) => void;
  renameFolder: (folder: FolderNode) => void;
  /** Move a folder, or a set of items, into a folder chosen from the tree. */
  moveFolder: (folder: FolderNode) => void;
  moveItems: (itemIds: number[]) => void;
  deleteFolder: (folder: FolderNode) => void;
  deleteItems: (itemIds: number[]) => void;
  tagItems: (itemIds: number[]) => void;
}

const DialogsContext = createContext<Dialogs | null>(null);

type Request =
  | { kind: "newFolder"; parent: FolderNode | null }
  | { kind: "renameFolder"; folder: FolderNode }
  | { kind: "moveFolder"; folder: FolderNode }
  | { kind: "moveItems"; itemIds: number[] }
  | { kind: "deleteFolder"; folder: FolderNode }
  | { kind: "deleteItems"; itemIds: number[] }
  | { kind: "tagItems"; itemIds: number[] };

export function DialogsProvider({
  folders,
  children,
}: {
  folders: FolderNode[];
  children: ReactNode;
}) {
  const ops = useOperations();
  const [request, setRequest] = useState<Request | null>(null);
  const close = useCallback(() => setRequest(null), []);

  const value = useMemo<Dialogs>(
    () => ({
      newFolder: (parent) => setRequest({ kind: "newFolder", parent }),
      renameFolder: (folder) => setRequest({ kind: "renameFolder", folder }),
      moveFolder: (folder) => setRequest({ kind: "moveFolder", folder }),
      moveItems: (itemIds) => setRequest({ kind: "moveItems", itemIds }),
      deleteFolder: (folder) => setRequest({ kind: "deleteFolder", folder }),
      deleteItems: (itemIds) => setRequest({ kind: "deleteItems", itemIds }),
      tagItems: (itemIds) => setRequest({ kind: "tagItems", itemIds }),
    }),
    [],
  );

  return (
    <DialogsContext.Provider value={value}>
      {children}

      {request?.kind === "newFolder" && (
        <NewFolderDialog
          parent={request.parent}
          onClose={close}
          onCreate={(name, archetypeId) =>
            ops.createFolder(
              request.parent?.id ?? null,
              request.parent?.title ?? "the top level",
              name,
              archetypeId,
            )
          }
        />
      )}

      {request?.kind === "renameFolder" && (
        <TextDialog
          title={`Rename ${request.folder.title}`}
          description="The folder on disk is renamed to match. There is one name."
          label="Title"
          initial={request.folder.title}
          confirmLabel="Rename"
          onClose={close}
          onSubmit={(title) => ops.renameFolder(request.folder, title)}
        />
      )}

      {request?.kind === "moveFolder" && (
        <FolderPickerDialog
          title={`Move ${request.folder.title} to…`}
          folders={folders}
          // A folder cannot be moved inside itself, and the picker says so by
          // not offering it rather than by failing afterwards.
          exclude={descendants(folders, request.folder)}
          allowTopLevel
          onClose={close}
          onPick={(dest) => ops.moveFolder(request.folder, dest)}
        />
      )}

      {request?.kind === "moveItems" && (
        <FolderPickerDialog
          title={`Move ${request.itemIds.length === 1 ? "1 item" : `${request.itemIds.length} items`} to…`}
          folders={folders}
          onClose={close}
          onPick={(dest) => {
            if (dest) void ops.moveItems(request.itemIds, dest);
          }}
        />
      )}

      {request?.kind === "deleteFolder" && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && close()}
          title={`Delete ${request.folder.title}?`}
          body={`${request.folder.title} and everything in it moves to .gallery/trash, keeping its paths. Nothing is erased, and the toast that follows can put it back.`}
          confirmLabel="Delete"
          danger
          onConfirm={() => ops.deleteFolder(request.folder)}
        />
      )}

      {request?.kind === "deleteItems" && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && close()}
          title={
            request.itemIds.length === 1
              ? "Delete this item?"
              : `Delete ${request.itemIds.length} items?`
          }
          body="They move to .gallery/trash with their paths preserved. Nothing is erased, and the toast that follows can put them back."
          confirmLabel="Delete"
          danger
          onConfirm={() => ops.deleteItems(request.itemIds)}
        />
      )}

      {request?.kind === "tagItems" && (
        <TagDialog
          count={request.itemIds.length}
          onClose={close}
          onSubmit={(key, value) => ops.addItemTag(request.itemIds, key, value)}
        />
      )}
    </DialogsContext.Provider>
  );
}

export function useDialogs(): Dialogs {
  const value = useContext(DialogsContext);
  if (!value) throw new Error("useDialogs must be used inside <DialogsProvider>");
  return value;
}

/** A folder and everything under it, by `relPath` prefix. */
function descendants(folders: FolderNode[], folder: FolderNode): Set<number> {
  const ids = new Set<number>([folder.id]);
  for (const candidate of folders) {
    if (
      candidate.relPath === folder.relPath ||
      candidate.relPath.startsWith(`${folder.relPath}/`)
    ) {
      ids.add(candidate.id);
    }
  }
  return ids;
}

// --- the dialogs themselves ------------------------------------------------

function TextDialog({
  title,
  description,
  label,
  initial,
  confirmLabel,
  onClose,
  onSubmit,
}: {
  title: string;
  description?: string;
  label: string;
  initial: string;
  confirmLabel: string;
  onClose: () => void;
  onSubmit: (value: string) => void;
}) {
  const [value, setValue] = useState(initial);
  const trimmed = value.trim();

  const submit = () => {
    if (!trimmed) return;
    onClose();
    onSubmit(trimmed);
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={title}
      description={description}
      width={430}
      footer={
        <>
          <button
            type="button"
            onClick={onClose}
            className="rounded-[4px] border border-line px-3 py-1 text-fg-mid hover:bg-hover hover:text-fg"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={!trimmed}
            onClick={submit}
            className="rounded-[4px] border border-accent-d bg-accent/15 px-3 py-1 text-accent hover:bg-accent/25 disabled:opacity-40"
          >
            {confirmLabel}
          </button>
        </>
      }
    >
      <label className="block text-[12px] text-fg-dim">
        {label}
        <input
          autoFocus
          value={value}
          onChange={(event) => setValue(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") submit();
          }}
          className="mt-1 w-full rounded-[4px] border border-line bg-ground px-2 py-1 text-[13px] text-fg focus:border-accent-d"
        />
      </label>
    </Dialog>
  );
}

function NewFolderDialog({
  parent,
  onClose,
  onCreate,
}: {
  parent: FolderNode | null;
  onClose: () => void;
  onCreate: (name: string, archetypeId: number | null) => void;
}) {
  const [name, setName] = useState("");
  const [archetypeId, setArchetypeId] = useState<number | null>(null);
  const [archetypes, setArchetypes] = useState<ArchetypeInfo[]>([]);
  const trimmed = name.trim();

  // The picker is empty until the user has made an archetype — the app ships
  // with none (PLAN.md decision 21), so this is the normal case, not an edge.
  useEffect(() => {
    let cancelled = false;
    void ipc
      .listArchetypes()
      .then((list) => !cancelled && setArchetypes(list))
      .catch(() => !cancelled && setArchetypes([]));
    return () => {
      cancelled = true;
    };
  }, []);

  const submit = () => {
    if (!trimmed) return;
    onClose();
    onCreate(trimmed, archetypeId);
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={`New folder in ${parent ? parent.title : "the top level"}`}
      width={430}
      footer={
        <>
          <button
            type="button"
            onClick={onClose}
            className="rounded-[4px] border border-line px-3 py-1 text-fg-mid hover:bg-hover hover:text-fg"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={!trimmed}
            onClick={submit}
            className="rounded-[4px] border border-accent-d bg-accent/15 px-3 py-1 text-accent hover:bg-accent/25 disabled:opacity-40"
          >
            Create
          </button>
        </>
      }
    >
      <label className="block text-[12px] text-fg-dim">
        Title
        <input
          autoFocus
          value={name}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") submit();
          }}
          className="mt-1 w-full rounded-[4px] border border-line bg-ground px-2 py-1 text-[13px] text-fg focus:border-accent-d"
        />
      </label>

      {archetypes.length > 0 && (
        <label className="mt-3 block text-[12px] text-fg-dim">
          Archetype
          <select
            value={archetypeId ?? ""}
            onChange={(event) =>
              setArchetypeId(event.target.value ? Number(event.target.value) : null)
            }
            className="mt-1 w-full rounded-[4px] border border-line bg-ground px-2 py-1 text-[13px] text-fg"
          >
            <option value="">None</option>
            {archetypes.map((archetype) => (
              <option key={archetype.id} value={archetype.id}>
                {archetype.name}
              </option>
            ))}
          </select>
        </label>
      )}
    </Dialog>
  );
}

function FolderPickerDialog({
  title,
  folders,
  exclude,
  allowTopLevel,
  onClose,
  onPick,
}: {
  title: string;
  folders: FolderNode[];
  exclude?: Set<number>;
  allowTopLevel?: boolean;
  onClose: () => void;
  onPick: (dest: FolderNode | null) => void;
}) {
  const [filter, setFilter] = useState("");
  const needle = filter.trim().toLowerCase();

  // The library root is not a folder in the interface (DESIGN.md §2), so it
  // is never a row here; "the top level" is offered as its own choice.
  const rows = folders
    .filter((folder) => folder.parentId !== null)
    .filter((folder) => !exclude?.has(folder.id))
    .filter(
      (folder) =>
        !needle ||
        folder.title.toLowerCase().includes(needle) ||
        folder.relPath.includes(needle),
    );

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()} title={title} width={470}>
      <input
        autoFocus
        value={filter}
        placeholder="Filter by title or path…"
        onChange={(event) => setFilter(event.target.value)}
        className="mb-2 w-full rounded-[4px] border border-line bg-ground px-2 py-1 text-[13px] text-fg focus:border-accent-d"
      />

      <div className="max-h-[46vh] overflow-y-auto rounded-[4px] border border-line-soft">
        {allowTopLevel && !needle && (
          <button
            type="button"
            onClick={() => {
              onClose();
              onPick(null);
            }}
            className="flex w-full items-center gap-2 border-b border-line-soft px-2 py-1.5 text-left text-fg-mid hover:bg-hover hover:text-fg"
          >
            The top level
          </button>
        )}

        {rows.map((folder) => (
          <button
            key={folder.id}
            type="button"
            onClick={() => {
              onClose();
              onPick(folder);
            }}
            className="flex w-full items-center gap-2 px-2 py-1.5 text-left hover:bg-hover"
            style={{ paddingLeft: 8 + Math.max(folder.depth - 1, 0) * 12 }}
          >
            <span className="truncate text-fg">{folder.title}</span>
            <span className="ml-auto shrink-0 truncate font-mono text-[11px] text-fg-dim">
              {folder.relPath}
            </span>
          </button>
        ))}

        {rows.length === 0 && (
          <p className="px-2 py-3 text-center text-fg-dim">
            {folders.length <= 1
              ? "There are no folders yet."
              : "Nothing matches that."}
          </p>
        )}
      </div>
    </Dialog>
  );
}

function TagDialog({
  count,
  onClose,
  onSubmit,
}: {
  count: number;
  onClose: () => void;
  onSubmit: (key: string | null, value: string) => void;
}) {
  const [text, setText] = useState("");

  // One field, two shapes: `key: value` is a label, anything else a flag.
  // Same syntax the query language uses, so there is one thing to learn.
  const [key, value] = (() => {
    const at = text.indexOf(":");
    if (at === -1) return [null, text.trim()] as const;
    return [text.slice(0, at).trim(), text.slice(at + 1).trim()] as const;
  })();

  const submit = () => {
    if (!value) return;
    onClose();
    onSubmit(key || null, value);
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={count === 1 ? "Tag this item" : `Tag ${count} items`}
      description="A flag is a word on its own. A label is key: value."
      width={430}
      footer={
        <>
          <button
            type="button"
            onClick={onClose}
            className="rounded-[4px] border border-line px-3 py-1 text-fg-mid hover:bg-hover hover:text-fg"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={!value}
            onClick={submit}
            className="rounded-[4px] border border-accent-d bg-accent/15 px-3 py-1 text-accent hover:bg-accent/25 disabled:opacity-40"
          >
            Add tag
          </button>
        </>
      }
    >
      <input
        autoFocus
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") submit();
        }}
        className="w-full rounded-[4px] border border-line bg-ground px-2 py-1 text-[13px] text-fg focus:border-accent-d"
      />
      {key && (
        <p className="mt-1.5 text-[12px] text-fg-dim">
          Label <span className="text-fg-mid">{key}</span> ={" "}
          <span className="text-fg-mid">{value || "—"}</span>
        </p>
      )}
    </Dialog>
  );
}
