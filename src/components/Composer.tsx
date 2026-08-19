import {
  ArrowUp,
  BookOpenText,
  Bot,
  ChevronUp,
  FileText,
  Files,
  GraduationCap,
  ListCollapse,
  Paperclip,
  PenLine,
  Search,
  ShieldCheck,
  SpellCheck,
  Square,
  X,
} from "lucide-react";
import { useEffect, useRef, useState, type ComponentType } from "react";
import type { AgentTool, DocumentInfo } from "../types";

const tools: Array<{ id: AgentTool; label: string; description: string; icon: ComponentType<{ size?: number }> }> = [
  { id: "auto", label: "Auto", description: "Moco chooses when a tool is useful", icon: Bot },
  { id: "desktop", label: "Desktop files", description: "Find, read, edit, create, and validate files", icon: Files },
  { id: "documents", label: "My documents", description: "Answer from your local document library", icon: BookOpenText },
  { id: "summarize", label: "Summarize", description: "Condense text while preserving key facts", icon: ListCollapse },
  { id: "research", label: "Research", description: "Build an overview from supplied local sources", icon: Search },
  { id: "grammar", label: "Grammar", description: "Correct language without changing meaning", icon: SpellCheck },
  { id: "rewrite", label: "Rewrite", description: "Reformat or rewrite for a chosen style", icon: PenLine },
  { id: "explain", label: "Explain", description: "Clarify science, technology, or any topic", icon: GraduationCap },
];

interface ComposerProps {
  value: string;
  selectedTool: AgentTool;
  documents: DocumentInfo[];
  selectedDocumentIds: string[];
  documentsOnly: boolean;
  generating: boolean;
  onChange: (value: string) => void;
  onToolChange: (tool: AgentTool) => void;
  onDocumentsOnly: (value: boolean) => void;
  onAttach: () => void;
  onRemoveDocument: (id: string) => void;
  onSubmit: () => void;
  onStop: () => void;
}

export function Composer(props: ComposerProps) {
  const textarea = useRef<HTMLTextAreaElement>(null);
  const toolPicker = useRef<HTMLDivElement>(null);
  const [toolsOpen, setToolsOpen] = useState(false);
  const selected = tools.find((tool) => tool.id === props.selectedTool) ?? tools[0];

  useEffect(() => {
    if (!textarea.current) return;
    textarea.current.style.height = "0px";
    textarea.current.style.height = `${Math.min(textarea.current.scrollHeight, 180)}px`;
  }, [props.value]);

  useEffect(() => {
    const close = (event: PointerEvent) => {
      if (!toolPicker.current?.contains(event.target as Node)) setToolsOpen(false);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, []);

  return (
    <div className="composer-wrap">
      <div className={`composer ${props.generating ? "composer-active" : ""}`}>
        {props.selectedDocumentIds.length > 0 && (
          <div className="attachment-row">
            {props.selectedDocumentIds.map((id) => {
              const document = props.documents.find((item) => item.id === id);
              if (!document) return null;
              return (
                <span className="attachment-chip" key={id}>
                  <FileText size={14} />
                  <span>{document.name}</span>
                  <button
                    type="button"
                    onClick={() => props.onRemoveDocument(id)}
                    aria-label={`Remove ${document.name}`}
                  >
                    <X size={13} />
                  </button>
                </span>
              );
            })}
          </div>
        )}
        <textarea
          ref={textarea}
          value={props.value}
          onChange={(event) => props.onChange(event.target.value)}
          onKeyDown={(event) => {
            if (
              event.key === "Enter" &&
              !event.shiftKey &&
              !event.nativeEvent.isComposing
            ) {
              event.preventDefault();
              if (!props.generating) props.onSubmit();
            }
          }}
          placeholder={
            props.selectedDocumentIds.length
              ? "Ask about your documents…"
              : "Ask Moco anything…"
          }
          aria-label="Message Moco"
          rows={1}
        />
        <div className="composer-toolbar">
          <div className="composer-leading">
            <button
              type="button"
              className="composer-icon"
              onClick={props.onAttach}
              title="Attach documents (Ctrl+O)"
            >
              <Paperclip size={18} />
            </button>
            <div className="tool-picker" ref={toolPicker}>
              <button
                type="button"
                className={`tool-picker-button ${props.selectedTool !== "auto" ? "active" : ""}`}
                onClick={() => setToolsOpen((open) => !open)}
                aria-haspopup="menu"
                aria-expanded={toolsOpen}
              >
                <selected.icon size={15} />
                <span>{selected.label}</span>
                <ChevronUp size={13} />
              </button>
              {toolsOpen && (
                <div className="tool-picker-menu" role="menu" aria-label="Agent tools">
                  <div className="tool-picker-heading">Use a tool</div>
                  {tools.map((tool) => {
                    const Icon = tool.icon;
                    return (
                      <button
                        key={tool.id}
                        type="button"
                        role="menuitemradio"
                        aria-checked={props.selectedTool === tool.id}
                        className={props.selectedTool === tool.id ? "active" : ""}
                        onClick={() => {
                          props.onToolChange(tool.id);
                          setToolsOpen(false);
                          textarea.current?.focus();
                        }}
                      >
                        <span><Icon size={16} /></span>
                        <span><strong>{tool.label}</strong><small>{tool.description}</small></span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
            <button
              type="button"
              className={`document-toggle ${props.documentsOnly ? "active" : ""}`}
              onClick={() => props.onDocumentsOnly(!props.documentsOnly)}
              title="Restrict answers to attached documents"
            >
              <ShieldCheck size={15} /> Docs only
            </button>
          </div>
          {props.generating ? (
            <button
              className="send-button stop-button"
              type="button"
              onClick={props.onStop}
              aria-label="Stop generating"
            >
              <Square size={14} fill="currentColor" />
            </button>
          ) : (
            <button
              className="send-button"
              type="button"
              onClick={props.onSubmit}
              disabled={!props.value.trim()}
              aria-label="Send message"
            >
              <ArrowUp size={18} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
