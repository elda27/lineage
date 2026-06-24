import { defineConfig } from "vitest/config";

// core(domain/application/infrastructure) の純 TS をテストする。
// crypto.subtle は Node18+ のグローバルで利用可能。
export default defineConfig({
  test: {
    include: ["core/**/*.test.ts"],
    environment: "node",
  },
});
