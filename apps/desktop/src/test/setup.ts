import { afterEach } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";

afterEach(async () => {
  cleanup();
  // Let listeners removed on unmount finish unregistering before the mocks go away.
  await new Promise((resolve) => setTimeout(resolve, 0));
  clearMocks();
});
