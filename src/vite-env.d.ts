/// <reference types="vite/client" />

declare module "*.sql?raw" {
  const content: string;
  export default content;
}

interface ImportMetaEnv {
  readonly VITE_APP_MODE?: "local" | "cloud";
  readonly VITE_API_BASE_URL?: string;
  readonly VITE_WORKSPACE_ID?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
