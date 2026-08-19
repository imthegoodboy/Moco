import { ChevronDown, MoreHorizontal, PanelLeft, Share2 } from "lucide-react";
import type { Conversation, ModelInfo } from "../types";

interface TopbarProps {
  title: string;
  conversation?: Conversation;
  model?: ModelInfo;
  sidebarCollapsed: boolean;
  provider: "local" | "api";
  onToggleSidebar: () => void;
  onModels: () => void;
  onExport?: () => void;
}

export function Topbar(props: TopbarProps) {
  return (
    <div className="topbar">
      <div className="topbar-leading">
        {props.sidebarCollapsed && <button type="button" className="icon-button" onClick={props.onToggleSidebar} aria-label="Open sidebar"><PanelLeft size={18} /></button>}
        <div className="topbar-title">
          <strong>{props.conversation?.title ?? props.title}</strong>
          {props.conversation && <span>{props.provider === "local" ? "Local AI agent" : "Connected AI agent"}</span>}
        </div>
      </div>
      <div className="topbar-actions">
        <button className="model-pill" type="button" onClick={props.onModels}>
          <span className={`status-dot ${props.model?.status === "error" ? "error" : ""}`} />
          <span>{props.provider === "local" ? props.model?.name ?? "Local model" : "API model"}</span>
          <ChevronDown size={14} />
        </button>
        {props.onExport && <button className="icon-button" type="button" onClick={props.onExport} title="Export conversation"><Share2 size={17} /></button>}
        <button className="icon-button" type="button" title="Conversation options"><MoreHorizontal size={18} /></button>
      </div>
    </div>
  );
}
