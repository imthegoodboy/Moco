import { ArrowUpRight, FileCheck2, FileSearch, FlaskConical, GitCompareArrows, GraduationCap, Languages, Newspaper, PenLine, ScanText } from "lucide-react";
import type { ToolMode } from "../types";

interface ToolsViewProps { onChoose: (mode: ToolMode) => void }

const tools = [
  { mode: "summarize" as const, name: "Summarize", detail: "Short, detailed, executive, or key-takeaway summaries.", icon: ScanText, ready: true },
  { mode: "research" as const, name: "Research analyzer", detail: "Methods, datasets, findings, limits, and future work.", icon: FlaskConical, ready: true },
  { mode: "news" as const, name: "News & editorial", detail: "Neutral summaries with events, claims, and author position.", icon: Newspaper, ready: true },
  { mode: "grammar" as const, name: "Grammar", detail: "Correct writing while protecting its meaning and voice.", icon: FileCheck2, ready: true },
  { mode: "rewrite" as const, name: "Rewrite", detail: "Professional, academic, concise, simple, or custom styles.", icon: PenLine, ready: true },
  { mode: "explain" as const, name: "Explain", detail: "Break difficult ideas into clear steps and examples.", icon: GraduationCap, ready: true },
  { mode: "compare" as const, name: "Compare documents", detail: "Find similarities, differences, tradeoffs, and missing evidence.", icon: GitCompareArrows, ready: true },
  { mode: "chat" as const, name: "Ask documents", detail: "Answer across your library with local source references.", icon: FileSearch, ready: true },
  { mode: "chat" as const, name: "Translate", detail: "Translate selected text while preserving structure.", icon: Languages, ready: false },
];

export function ToolsView({ onChoose }: ToolsViewProps) {
  return (
    <main className="page page-scroll">
      <header className="page-heading"><p className="eyebrow">FOCUSED WORKFLOWS</p><h1>AI tools</h1><p>Start with a purpose-built prompt, then refine it in a normal conversation.</p></header>
      <div className="tool-grid">
        {tools.map(({ mode, name, detail, icon: Icon, ready }) => (
          <button key={name} className="tool-card" type="button" onClick={() => ready && onChoose(mode)} disabled={!ready}>
            <span className="tool-icon"><Icon size={21} /></span>
            <span className="tool-copy"><strong>{name}</strong><small>{detail}</small></span>
            {ready ? <ArrowUpRight size={17} /> : <span className="soon-tag">Soon</span>}
          </button>
        ))}
      </div>
      <section className="page-note"><strong>Built for local work</strong><p>Every ready tool works with the bundled model and your local documents. API mode is optional.</p></section>
    </main>
  );
}

