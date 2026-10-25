import { describe, expect, it } from "vitest";

// M1 红路径验收专用（临时分支，验证后随分支删除）：
// 此用例必须失败，用于确认 frontend 红 → ci-required 红 → 合并被分支保护阻止。
describe("CI acceptance: red path", () => {
  it("intentionally fails to validate the required check", () => {
    expect(1).toBe(2);
  });
});
