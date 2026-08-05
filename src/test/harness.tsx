/**
 * Shared scaffolding for the interaction tests: fake library data, and a
 * renderer that puts a component inside the providers the app gives it.
 */

import { render, type RenderResult } from "@testing-library/react";
import type { ReactNode } from "react";
import { vi } from "vitest";

import { ToastProviderRoot } from "../components/Toaster";
import { TooltipProvider } from "../components/Tooltip";
import { DialogsProvider } from "../features/menus/Dialogs";
import { OperationsProvider } from "../features/menus/operations";
import type {
  BreadcrumbCrumb,
  FolderDetail,
  FolderNode,
  GridItem,
  ItemDetail,
  LibraryInfo,
} from "../lib/types";
import { DndProvider } from "../state/dnd";
import type { LibraryController, Scope } from "../state/library";
import type { SelectionController } from "../state/selection";
import { ToastProvider } from "../state/toasts";
import { UiProvider } from "../state/ui";

export const EVERYTHING_SCOPE: Scope = {
  kind: "everything",
  folderId: null,
  recursive: true,
};

/** A top-level folder — PLAN.md decision 30 dropped the library-root row
 *  that `parentId: null` used to be reserved for; every folder with no
 *  parent is a real top-level tree node now. */
export function folderNode(over: Partial<FolderNode> = {}): FolderNode {
  return {
    id: 1,
    title: "Trips",
    parentId: 100,
    depth: 1,
    directCount: 4,
    totalCount: 12,
    status: "active",
    favorite: false,
    ...over,
  };
}

/** A top-level folder, distinct from `folderNode()`'s default nested one —
 *  useful as the parent in tests that build a small tree. Not "the root":
 *  there is no such thing any more. */
export function topLevelNode(over: Partial<FolderNode> = {}): FolderNode {
  return {
    id: 100,
    title: "Library",
    parentId: null,
    depth: 0,
    directCount: 0,
    totalCount: 12,
    status: "active",
    favorite: false,
    ...over,
  };
}

export function gridItem(over: Partial<GridItem> = {}): GridItem {
  return {
    id: 7,
    thumb: "ab/cd/uuid.webp",
    kind: "image",
    w: 1200,
    h: 800,
    durationMs: null,
    favorite: false,
    at: 1_700_000_000,
    name: "beach.jpg",
    ...over,
  };
}

const DEFAULT_BREADCRUMB: BreadcrumbCrumb[] = [{ id: 1, title: "Trips" }];

export function itemDetail(over: Partial<ItemDetail> = {}): ItemDetail {
  return {
    id: 7,
    kind: "image",
    path: "D:/Library/trips/uuid.jpg",
    thumb: "ab/cd/uuid.webp",
    diskName: "uuid.jpg",
    origName: "beach.jpg",
    folderId: 1,
    folderBreadcrumb: DEFAULT_BREADCRUMB,
    sizeBytes: 2_400_000,
    width: 1200,
    height: 800,
    durationMs: null,
    codec: null,
    bitrate: null,
    capturedAt: 1_700_000_000,
    capturedSrc: "exif",
    addedAt: 1_700_000_500,
    favorite: false,
    notes: null,
    hash: "abc",
    sourceUrl: null,
    ...over,
  };
}

export function folderDetail(over: Partial<FolderDetail> = {}): FolderDetail {
  return {
    id: 1,
    title: "Trips",
    parentId: 100,
    status: "active",
    favorite: false,
    notes: null,
    lastAddedAt: null,
    directCount: 4,
    totalCount: 12,
    subfolderCount: 2,
    archetypeId: null,
    archetypeName: null,
    fields: [],
    flags: [],
    coverThumb: null,
    coverItemId: null,
    ...over,
  };
}

export function libraryInfo(): LibraryInfo {
  return {
    root: "D:/Library",
    name: "Library",
    thumbsDir: "D:/Library/.ggallery/cache/thumbs",
    spritesDir: "D:/Library/.ggallery/cache/sprites",
    itemCount: 12,
    folderCount: 2,
    ffmpeg: "ffmpeg",
    lowercaseMergeReport: null,
  };
}

/** A `LibraryController` whose every method is a spy. */
export function fakeLibrary(over: Partial<LibraryController> = {}): LibraryController {
  return {
    info: libraryInfo(),
    remembered: null,
    folders: [topLevelNode(), folderNode()],
    items: [gridItem()],
    unsortedCount: 0,
    progress: null,
    failures: [],
    loading: false,
    error: null,
    scope: EVERYTHING_SCOPE,
    refreshToken: 0,
    pendingReview: null,
    flowPhase: "idle",
    renameProgress: null,
    storageMigration: null,
    lowercaseMergeReport: null,
    choose: vi.fn(),
    open: vi.fn(),
    confirmImport: vi.fn(),
    cancelImport: vi.fn(),
    confirmStorageMigration: vi.fn(),
    cancelStorageMigration: vi.fn(),
    dismissLowercaseMergeReport: vi.fn(),
    retry: vi.fn(),
    setScope: vi.fn(),
    reload: vi.fn(),
    dismissError: vi.fn(),
    refreshFolders: vi.fn(),
    ...over,
  };
}

export function fakeSelection(
  over: Partial<SelectionController> = {},
): SelectionController {
  return {
    selected: new Set<number>(),
    count: 0,
    current: null,
    isSelected: () => false,
    click: vi.fn(),
    focus: vi.fn(),
    step: vi.fn(),
    selectAll: vi.fn(),
    invert: vi.fn(),
    clear: vi.fn(),
    ...over,
  };
}

export interface HarnessOptions {
  library?: LibraryController;
  selection?: SelectionController;
}

/** Render inside the providers, so components under test get the real
 *  operations, dialogs and toast queue rather than mocks of them. */
export function renderWithProviders(
  ui: ReactNode,
  options: HarnessOptions = {},
): RenderResult & { library: LibraryController; selection: SelectionController } {
  const library = options.library ?? fakeLibrary();
  const selection = options.selection ?? fakeSelection();

  const result = render(
    <UiProvider>
      <ToastProvider>
        <TooltipProvider>
          <ToastProviderRoot>
            <OperationsProvider library={library} selection={selection}>
              <DialogsProvider folders={library.folders}>
                <DndProvider>{ui}</DndProvider>
              </DialogsProvider>
            </OperationsProvider>
          </ToastProviderRoot>
        </TooltipProvider>
      </ToastProvider>
    </UiProvider>,
  );

  return { ...result, library, selection };
}
