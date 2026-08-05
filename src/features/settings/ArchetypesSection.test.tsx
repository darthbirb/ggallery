/**
 * The archetype editor, which is mandatory now that the app ships no
 * vocabulary at all (PLAN.md decision 21).
 *
 * This is also the exact bug class PLAN.md §M2.5 names when it asks for
 * frontend tests: "does picking an archetype call the right command" — M2
 * shipped a dropdown that focused the notes field instead of registering the
 * selection, and neither Rust tests nor `tsc` could see it.
 */

import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { renderWithProviders } from "../../test/harness";
import { ArchetypesSection } from "./ArchetypesSection";

vi.mock("../../lib/ipc");

import * as ipc from "../../lib/ipc";

const mocked = vi.mocked(ipc);

beforeEach(() => {
  mocked.errorMessage.mockImplementation((err: unknown) => String(err));
  mocked.listArchetypes.mockResolvedValue([
    { id: 3, name: "Trip", fields: [{ key: "city", ordinal: 0 }] },
  ]);
  mocked.countFoldersUsingArchetype.mockResolvedValue(0);
  mocked.archetypeFieldUsage.mockResolvedValue([]);
  mocked.addArchetypeField.mockResolvedValue(undefined);
  mocked.removeArchetypeField.mockResolvedValue(undefined);
  mocked.createArchetype.mockResolvedValue(9);
  mocked.deleteArchetype.mockResolvedValue(undefined);
});

async function open() {
  const onChanged = vi.fn();
  renderWithProviders(<ArchetypesSection onChanged={onChanged} />);
  await userEvent.click(await screen.findByRole("button", { name: "Trip" }));
  return { onChanged };
}

describe("archetypes", () => {
  it("creates one from the name typed in", async () => {
    renderWithProviders(<ArchetypesSection onChanged={vi.fn()} />);
    await userEvent.type(
      await screen.findByLabelText("New archetype name"),
      "Person{Enter}",
    );
    await waitFor(() => expect(mocked.createArchetype).toHaveBeenCalledWith("Person"));
  });

  it("adds a field straight away when no folder uses the archetype", async () => {
    await open();

    await userEvent.type(screen.getByLabelText("New label key"), "country");
    await userEvent.click(screen.getByRole("button", { name: "Add label" }));

    await waitFor(() =>
      expect(mocked.addArchetypeField).toHaveBeenCalledWith(3, "country", false),
    );
  });

  it("asks before touching folders that already use the archetype", async () => {
    mocked.countFoldersUsingArchetype.mockResolvedValue(3);
    await open();

    await userEvent.type(screen.getByLabelText("New label key"), "country");
    await userEvent.click(screen.getByRole("button", { name: "Add label" }));

    // "3 folders use this archetype. Add the new label to them?" — named, and
    // answerable both ways.
    expect(await screen.findByText(/3 folders use/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Add to all 3/ }));

    await waitFor(() =>
      expect(mocked.addArchetypeField).toHaveBeenCalledWith(3, "country", true),
    );
  });

  it("can add the field without applying it to existing folders", async () => {
    mocked.countFoldersUsingArchetype.mockResolvedValue(2);
    await open();

    await userEvent.type(screen.getByLabelText("New label key"), "country");
    await userEvent.click(screen.getByRole("button", { name: "Add label" }));
    await userEvent.click(
      await screen.findByRole("button", { name: /Just the archetype/ }),
    );

    await waitFor(() =>
      expect(mocked.addArchetypeField).toHaveBeenCalledWith(3, "country", false),
    );
  });

  it("names the folders a removed field would empty", async () => {
    mocked.archetypeFieldUsage.mockResolvedValue([
      { folderId: 1, title: "Alps", value: "chamonix" },
      { folderId: 2, title: "Borneo", value: "kuching" },
    ]);
    await open();

    await userEvent.click(screen.getByRole("button", { name: "Remove city" }));

    expect(await screen.findByText(/Alps, Borneo/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Remove the field" }));

    await waitFor(() =>
      expect(mocked.removeArchetypeField).toHaveBeenCalledWith(3, "city"),
    );
  });

  it("removes an unused field without a confirmation", async () => {
    await open();
    await userEvent.click(screen.getByRole("button", { name: "Remove city" }));
    await waitFor(() =>
      expect(mocked.removeArchetypeField).toHaveBeenCalledWith(3, "city"),
    );
    expect(screen.queryByText(/This deletes the value/)).toBeNull();
  });

  it("confirms before deleting an archetype, and says what survives", async () => {
    await open();

    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(await screen.findByText(/keep the labels they already have/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Delete archetype" }));
    await waitFor(() => expect(mocked.deleteArchetype).toHaveBeenCalledWith(3));
  });
});
