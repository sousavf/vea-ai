import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App.js";

describe("desktop shell", () => {
  it("labels browser preview as an unprivileged mock", async () => {
    render(<App />);

    expect(await screen.findByText("Browser demo")).not.toBeNull();
    expect(screen.getByText("Browser mock")).not.toBeNull();
    expect(screen.getByText("No privileged host")).not.toBeNull();
    expect(screen.getByText("Demo state only")).not.toBeNull();
  });

  it("keeps project task state independently selectable", async () => {
    render(<App />);
    await screen.findByText("Browser demo");

    fireEvent.click(screen.getByRole("button", { name: /Storefront/ }));

    expect(screen.getByRole("heading", { name: "Storefront" })).not.toBeNull();
    expect(screen.getAllByText("Fix checkout state race").length).toBeGreaterThan(0);
    expect(screen.queryByText("Implement deterministic task scheduler")).toBeNull();
  });
});
