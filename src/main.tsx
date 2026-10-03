import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./style.css";
// apply the saved appearance before React renders, so there is no flash of the wrong theme
{
  const root = document.documentElement; const t = localStorage.getItem("theme"); const c = localStorage.getItem("contrast");
  if (t === "light" || t === "dark") root.dataset.theme = t;
  // "system" (the default, also when nothing is saved) follows the OS; "on"/"high" and "off" are explicit choices
  if (c === "on" || c === "high" || (c !== "off" && matchMedia("(prefers-contrast: more)").matches)) root.dataset.contrast = "high";
}
ReactDOM.createRoot(document.getElementById("root")!).render(<App />);
