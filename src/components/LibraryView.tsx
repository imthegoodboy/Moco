import { File, FilePlus2, FolderOpen, MoreHorizontal, Search, Trash2, UploadCloud } from "lucide-react";
import { useMemo, useState } from "react";
import { formatBytes } from "../lib/format";
import type { DocumentInfo } from "../types";

interface LibraryViewProps {
  documents: DocumentInfo[];
  importing?: string;
  onImport: () => void;
  onFolder: () => void;
  onAsk: (document: DocumentInfo) => void;
  onDelete: (document: DocumentInfo) => void;
}

export function LibraryView(props: LibraryViewProps) {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<"recent" | "name" | "size">("recent");
  const documents = useMemo(() => {
    const filtered = props.documents.filter((item) => item.name.toLowerCase().includes(query.toLowerCase()));
    return [...filtered].sort((a, b) => {
      if (sort === "name") return a.name.localeCompare(b.name);
      if (sort === "size") return b.sizeBytes - a.sizeBytes;
      return +new Date(b.createdAt) - +new Date(a.createdAt);
    });
  }, [props.documents, query, sort]);

  return (
    <main className="page page-scroll">
      <header className="page-heading page-heading-row">
        <div><p className="eyebrow">LOCAL KNOWLEDGE</p><h1>Documents</h1><p>Indexed on this device. Nothing is uploaded in local mode.</p></div>
        <div className="heading-actions"><button className="secondary-button" type="button" onClick={props.onFolder}><FolderOpen size={16} /> Import folder</button><button className="primary-button" type="button" onClick={props.onImport}><FilePlus2 size={16} /> Add files</button></div>
      </header>

      {props.importing && <div className="import-progress"><span className="activity-spinner" /><span>{props.importing}</span></div>}

      <div className="library-toolbar">
        <label className="field-search"><Search size={16} /><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search documents" /></label>
        <label className="compact-select">Sort <select value={sort} onChange={(event) => setSort(event.target.value as typeof sort)}><option value="recent">Recently added</option><option value="name">Name</option><option value="size">Size</option></select></label>
      </div>

      {documents.length ? (
        <div className="document-table">
          <div className="document-row document-header"><span>Name</span><span>Type</span><span>Size</span><span>Pages</span><span>Status</span><span /></div>
          {documents.map((document) => (
            <div className="document-row" key={document.id}>
              <button className="document-name" type="button" onClick={() => props.onAsk(document)}><span className="file-icon"><File size={17} /></span><span><strong>{document.name}</strong><small>Added {new Date(document.createdAt).toLocaleDateString()}</small></span></button>
              <span>{document.fileType}</span><span>{formatBytes(document.sizeBytes)}</span><span>{document.pageCount || "—"}</span>
              <span className="ready-label"><i /> Ready</span>
              <div className="row-actions"><button type="button" onClick={() => props.onAsk(document)}>Ask</button><button className="icon-button" type="button" title="Delete" onClick={() => props.onDelete(document)}><Trash2 size={15} /></button><button className="icon-button" type="button"><MoreHorizontal size={16} /></button></div>
            </div>
          ))}
        </div>
      ) : (
        <div className="blank-panel"><span><UploadCloud size={25} /></span><h2>{query ? "No matching documents" : "Build your local knowledge base"}</h2><p>{query ? "Try a different search." : "Add PDFs, Word files, text, Markdown, CSV, or HTML. Moco will index them locally for search and answers."}</p>{!query && <button className="primary-button" type="button" onClick={props.onImport}><FilePlus2 size={16} /> Choose files</button>}</div>
      )}
    </main>
  );
}

