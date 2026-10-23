// Runs around every state test (`vitest.config.ts`): each test imports the
// stores afresh (they are module singletons), starts with empty storage,
// and fails if a store invoked a command the test didn't answer.

import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, expect, vi } from "vitest";
import { endBackend } from "./backend";

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
});

afterEach(() => {
  const unanswered = endBackend();
  clearMocks();
  vi.useRealTimers();
  expect(unanswered, "commands the test didn't answer").toEqual([]);
});
