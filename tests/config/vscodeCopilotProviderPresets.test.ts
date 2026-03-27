import { describe, expect, it } from "vitest";
import { vscodeCopilotProviderPresets } from "@/config/vscodeCopilotProviderPresets";

describe("vscodeCopilotProviderPresets", () => {
  it("only exposes the custom model template", () => {
    expect(vscodeCopilotProviderPresets).toHaveLength(1);
    expect(vscodeCopilotProviderPresets[0]).toMatchObject({
      name: "Custom Model",
      category: "custom",
      isCustomTemplate: true,
    });
  });
});
