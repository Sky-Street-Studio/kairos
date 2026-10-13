import { describe, expect, it } from "vitest";

import { validateCharacter } from "./character-manager";

describe("characters", () => {
  it("validates fields", () => {
    expect(validateCharacter({ id: "1x", name: "A" })).toMatch(/id/);
    expect(validateCharacter({ id: "x", name: " " })).toMatch(/name/);
    expect(validateCharacter({ id: "x", name: "A", color: "red" })).toMatch(/#/);
    expect(validateCharacter({ id: "x", name: "A", color: "#aabbcc" })).toBeNull();
  });
});
