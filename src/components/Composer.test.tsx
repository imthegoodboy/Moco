// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Composer } from "./Composer";
import type { DocumentInfo } from "../types";

const document: DocumentInfo = {
  id: "document-1",
  name: "notes.txt",
  fileType: "TXT",
  sizeBytes: 42,
  pageCount: 1,
  status: "ready",
  createdAt: "2026-08-21T00:00:00Z",
  tags: [],
};

const baseProps = {
  value: "",
  selectedTool: "documents" as const,
  documents: [document],
  selectedDocumentIds: [document.id],
  documentsOnly: false,
  generating: false,
  onChange: vi.fn(),
  onToolChange: vi.fn(),
  onDocumentsOnly: vi.fn(),
  onAttach: vi.fn(),
  onRemoveDocument: vi.fn(),
  onSubmit: vi.fn(),
  onStop: vi.fn(),
};

describe("Composer document attachments", () => {
  it("shows an attached document in the chat composer", () => {
    render(<Composer {...baseProps} />);

    expect(screen.getByText("notes.txt")).toBeInTheDocument();
    expect(screen.getByLabelText("Message Moco")).toHaveAttribute(
      "placeholder",
      "Ask about your documents…",
    );
    expect(screen.getByRole("button", { name: "My documents" })).toBeInTheDocument();
  });

  it("shows indexing progress and prevents a second picker", () => {
    render(
      <Composer
        {...baseProps}
        selectedDocumentIds={[]}
        importing="Reading notes.txt · 40%"
      />,
    );

    expect(screen.getByText("Reading notes.txt · 40%")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Indexing document" })).toBeDisabled();
  });
});
