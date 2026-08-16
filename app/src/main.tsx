import React from "react";
import ReactDOM from "react-dom/client";
import "./lib/brandMigration";
import { ensureBackendOriginResolver } from "./lib/backendOrigin";
import { App } from "./App";
import "./styles/global.css";

ensureBackendOriginResolver();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
