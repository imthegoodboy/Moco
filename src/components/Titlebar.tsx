import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X } from "lucide-react";
import { isDesktop } from "../lib/desktop";

export function Titlebar() {
  const run = async (action: "minimize" | "maximize" | "close") => {
    if (!isDesktop()) return;
    const window = getCurrentWindow();
    if (action === "minimize") await window.minimize();
    if (action === "maximize") await window.toggleMaximize();
    if (action === "close") await window.close();
  };

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="titlebar-brand" data-tauri-drag-region>
        <span className="mini-mark">M</span>
        <span>Moco</span>
      </div>
      <div className="window-controls">
        <button type="button" aria-label="Minimize" title="Minimize" onClick={() => void run("minimize")}><Minus size={15} /></button>
        <button type="button" aria-label="Maximize" title="Maximize" onClick={() => void run("maximize")}><Square size={12} /></button>
        <button type="button" className="window-close" aria-label="Close" title="Close" onClick={() => void run("close")}><X size={15} /></button>
      </div>
    </header>
  );
}

