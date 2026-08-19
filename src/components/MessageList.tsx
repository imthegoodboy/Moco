import { Bookmark, Check, Clipboard, Pencil, RefreshCw, ThumbsDown, ThumbsUp, Trash2, Volume2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { formatTime } from "../lib/format";
import type { Message, SourceRef } from "../types";

interface MessageListProps {
  messages: Message[];
  streamingContent: string;
  streamingSources: SourceRef[];
  phase?: string;
  error?: string;
  onCopy: (text: string) => void;
  onDelete: (message: Message) => void;
  onRetry: (message: Message) => void;
  onFeedback: (message: Message, value?: "up" | "down") => void;
  onSave: (message: Message) => void;
}

const phaseLabels: Record<string, string> = {
  understanding: "Understanding your request",
  "reading-documents": "Reading relevant document sections",
  "loading-model": "Preparing the local model",
  connecting: "Connecting to your API provider",
  generating: "Writing the response",
  stopped: "Generation stopped",
};

function Markdown({ children }: { children: string }) {
  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm]}
      components={{
        a: ({ children: label, ...props }) => <a {...props} target="_blank" rel="noreferrer">{label}</a>,
        code: ({ children: code, className, ...props }) => {
          const block = className?.startsWith("language-");
          return block ? <code className={className}>{code}</code> : <code className="inline-code" {...props}>{code}</code>;
        },
      }}
    >
      {children}
    </ReactMarkdown>
  );
}

function Sources({ sources }: { sources: SourceRef[] }) {
  const [open, setOpen] = useState(false);
  if (!sources.length) return null;
  return (
    <div className="sources-block">
      <button type="button" onClick={() => setOpen((value) => !value)}>
        <span>{sources.length} local source{sources.length === 1 ? "" : "s"}</span><span>{open ? "Hide" : "View"}</span>
      </button>
      {open && (
        <div className="source-list">
          {sources.map((source, index) => (
            <article key={`${source.documentId}-${index}`}>
              <span className="source-index">{index + 1}</span>
              <div><strong>{source.documentName}{source.page ? ` · Page ${source.page}` : ""}</strong><p>{source.excerpt.slice(0, 260)}{source.excerpt.length > 260 ? "…" : ""}</p></div>
            </article>
          ))}
        </div>
      )}
    </div>
  );
}

export function MessageList(props: MessageListProps) {
  const end = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState<string>();
  useEffect(() => {
    end.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [props.messages, props.streamingContent, props.phase]);

  const copy = (message: Message) => {
    props.onCopy(message.content);
    setCopied(message.id);
    setTimeout(() => setCopied(undefined), 1400);
  };

  return (
    <div className="message-scroll">
      <div className="message-list" aria-live="polite">
        {props.messages.map((message) => (
          <article key={message.id} className={`message message-${message.role}`}>
            <div className="message-meta"><span>{message.role === "user" ? "You" : "Moco"}</span><time>{formatTime(message.createdAt)}</time></div>
            <div className="message-body"><Markdown>{message.content}</Markdown></div>
            <Sources sources={message.sources} />
            <div className="message-actions">
              <button type="button" onClick={() => copy(message)} title="Copy">{copied === message.id ? <Check size={14} /> : <Clipboard size={14} />}</button>
              {message.role === "assistant" ? (
                <>
                  <button type="button" className={message.feedback === "up" ? "active" : ""} onClick={() => props.onFeedback(message, message.feedback === "up" ? undefined : "up")} title="Helpful"><ThumbsUp size={14} /></button>
                  <button type="button" className={message.feedback === "down" ? "active" : ""} onClick={() => props.onFeedback(message, message.feedback === "down" ? undefined : "down")} title="Not helpful"><ThumbsDown size={14} /></button>
                  <button type="button" onClick={() => props.onRetry(message)} title="Regenerate"><RefreshCw size={14} /></button>
                  <button type="button" className={message.saved ? "active" : ""} onClick={() => props.onSave(message)} title="Save response"><Bookmark size={14} fill={message.saved ? "currentColor" : "none"} /></button>
                  <button type="button" onClick={() => speechSynthesis.speak(new SpeechSynthesisUtterance(message.content))} title="Read aloud"><Volume2 size={14} /></button>
                </>
              ) : <button type="button" title="Edit and retry"><Pencil size={14} /></button>}
              <button type="button" onClick={() => props.onDelete(message)} title="Delete"><Trash2 size={14} /></button>
            </div>
          </article>
        ))}

        {(props.phase || props.streamingContent || props.error) && (
          <article className="message message-assistant streaming-message">
            <div className="message-meta"><span>Moco</span><span className="live-label"><i /> Live</span></div>
            {props.phase && !props.streamingContent && !props.error && (
              <div className="activity-line"><span className="activity-spinner" /><span>{phaseLabels[props.phase] ?? props.phase}</span></div>
            )}
            {props.streamingContent && <div className="message-body"><Markdown>{props.streamingContent}</Markdown><span className="stream-cursor" /></div>}
            {props.error && <div className="inline-error"><strong>Generation stopped</strong><p>{props.error}</p></div>}
            <Sources sources={props.streamingSources} />
          </article>
        )}
        <div ref={end} />
      </div>
    </div>
  );
}
