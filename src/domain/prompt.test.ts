import { expect, it } from "vitest";
import { buildSystemPrompt } from "./prompt";

it("tells the engine to keep placeholders and to output only the translation", () => {
  const prompt = buildSystemPrompt("followSource");
  expect(prompt).toContain("⟦K0⟧");
  expect(prompt).toMatch(/Copy every placeholder verbatim/);
  expect(prompt).toMatch(/Output only the Japanese translation/);
});

it("pins the style when the user chose one", () => {
  expect(buildSystemPrompt("desuMasu")).toContain("です・ます調 throughout");
  expect(buildSystemPrompt("daDearu")).toContain("だ・である調 throughout");
});
