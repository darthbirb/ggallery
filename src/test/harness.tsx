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
  FolderNode,
  GridItem,
  ItemDetail,
  LibraryInfo,
} from "../lib/types";
import type { LibraryController, Scope } from "../state/library";
import type { SelectionController } from "../state/selection";
import { ToastProvider } from "../state/toasts";
import { UiProvider } from "../state/ui";

export const EVERYTHING_SCOPE: Scope = {
  kind: "everything",
  folder: null,
  recursive: true,
};

export function folderNode(over: Partial<FolderNode> = {}): FolderNode {
  return {
    id: 1,
    relPath: "trips",
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

export function rootNode(): FolderNode {
  return {
    id: 100,
    relPath: "",
    title: "Library",
    parentId: null,
    depth: 0,
    directCount: 0,
    totalCount: 12,
    status: "active",
    favorite: false,
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

export function itemDetail(over: Partial<ItemDetail> = {}): ItemDetail {
  return {
    id: 7,
    kind: "image",
    path: "D:/Library/trips/uuid.jpg",
    thumb: "ab/cd/uuid.webp",
    diskName: "uuid.jpg",
    origName: "beach.jpg",
    folderId: 1,
    folderRel: "trips",
    folderTitle: "Trips",
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

export function libraryInfo(): LibraryInfo {
  return {
    root: "D:/Library",
    name: "Library",
    thumbsDir: "D:/Library/.gallery/cache/thumbs",
    spritesDir: "D:/Library/.gallery/cache/sprites",
    itemCount: 12,
    folderCount: 2,
    ffmpeg: "ffmpeg",
  };
}

/** A `LibraryController` whose every method is a spy. */
export function fakeLibrary(over: Partial<LibraryController> = {}): LibraryController {
  return {
    info: libraryInfo(),
    remembered: null,
    folders: [rootNode(), folderNode()],
    items: [gridItem()],
    progress: null,
    failures: [],
    loading: false,
    error: null,
    scope: EVERYTHING_SCOPE,
    refreshToken: 0,
    pendingReview: null,
    flowPhase: "idle",
    renameProgress: null,
    verifyIssue: null,
    choose: vi.fn(),
    open: vi.fn(),
    confirmImport: vi.fn(),
    cancelImport: vi.fn(),
    dismissVerifyIssue: vi.fn(),
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
              <DialogsProvider folders={library.folders}>{ui}</DialogsProvider>
            </OperationsProvider>
          </ToastProviderRoot>
        </TooltipProvider>
      </ToastProvider>
    </UiProvider>,
  );

  return { ...result, library, selection };
}
