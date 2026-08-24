import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";

import App from "@/app/App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

// 最初の画面が実際に1フレーム描けるまでは、Rust 側の起動ダイアログを表示しておく。
requestAnimationFrame(() =>
  requestAnimationFrame(() => void invoke("browser_rendered")),
);
