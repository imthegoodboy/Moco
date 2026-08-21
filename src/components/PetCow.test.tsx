// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PetCow } from "./PetCow";

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("Pinky pet cow", () => {
  it("shows an accessible draggable pet without extra controls", () => {
    render(<PetCow />);

    expect(screen.getByLabelText("Pinky, Moco's pet cow")).toBeInTheDocument();
    expect(screen.getByText("Moo! I’m Moco.")).toBeInTheDocument();
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

  it("plays Pinky's real moo and starts her speaking reaction when tapped", () => {
    const play = vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue();
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
    render(<PetCow />);

    fireEvent.click(screen.getByRole("button", { name: /tap pinky to moo/i }));

    expect(play).toHaveBeenCalledOnce();
    expect(screen.getByText("Moooo! 💗")).toBeInTheDocument();
    expect(screen.getByLabelText("Pinky, Moco's pet cow")).toHaveClass("is-speaking");
  });
});
