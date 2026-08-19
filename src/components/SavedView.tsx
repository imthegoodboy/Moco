import { Bookmark, Clipboard, Trash2 } from "lucide-react";
import type { Message } from "../types";

interface SavedViewProps { messages: Message[]; onCopy: (text: string) => void; onRemove: (message: Message) => void }

export function SavedView({ messages, onCopy, onRemove }: SavedViewProps) {
  const saved = messages.filter((message) => message.saved && message.role === "assistant");
  return (
    <main className="page page-scroll">
      <header className="page-heading"><p className="eyebrow">KEEP THE GOOD PARTS</p><h1>Saved responses</h1><p>Useful answers and notes, available offline.</p></header>
      {saved.length ? <div className="saved-grid">{saved.map((message) => <article className="saved-card" key={message.id}><Bookmark size={16} fill="currentColor" /><p>{message.content.slice(0, 420)}{message.content.length > 420 ? "…" : ""}</p><div><button type="button" onClick={() => onCopy(message.content)}><Clipboard size={14} /> Copy</button><button type="button" onClick={() => onRemove(message)}><Trash2 size={14} /> Remove</button></div></article>)}</div> : <div className="blank-panel"><span><Bookmark size={24} /></span><h2>Nothing saved yet</h2><p>Use the bookmark action beneath any Moco response to keep it here.</p></div>}
    </main>
  );
}

