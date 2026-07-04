import "./index.css";

import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";

const darkModeMediaQuery = window.matchMedia("(prefers-color-scheme: dark)");

function syncSystemColorScheme() {
  document.documentElement.classList.toggle("dark", darkModeMediaQuery.matches);
}

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("Root element not found");
}

syncSystemColorScheme();
darkModeMediaQuery.addEventListener("change", syncSystemColorScheme);

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
