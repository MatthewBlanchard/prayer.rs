import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App.js";
import { PrayerProvider } from "./prayer/PrayerProvider.js";
import "./style.css";

const root = document.getElementById("root");
if (!root) throw new Error("No #root element");
createRoot(root).render(
  <StrictMode>
    <PrayerProvider>
      <App />
    </PrayerProvider>
  </StrictMode>,
);
