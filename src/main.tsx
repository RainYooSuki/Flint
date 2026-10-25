import React from "react";
import ReactDOM from "react-dom/client";
import App from "./app/App";
import { createTauriHostServices } from "./core/platform/tauriServices";
import "./styles.css";

const hostServices = createTauriHostServices();

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App services={hostServices} />
  </React.StrictMode>
);
