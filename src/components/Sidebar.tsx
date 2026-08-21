import {
  Archive,
  Bot,
  ChevronLeft,
  FileText,
  Library,
  MessageSquare,
  MoreHorizontal,
  PanelLeft,
  Pencil,
  Pin,
  Plus,
  Search,
  Settings,
  Trash2,
} from "lucide-react";
import { useMemo, useState } from "react";
import { relativeGroup } from "../lib/format";
import type { Conversation, View } from "../types";

interface SidebarProps {
  collapsed: boolean;
  view: View;
  conversations: Conversation[];
  activeConversationId?: string;
  onToggle: () => void;
  onView: (view: View) => void;
  onNewChat: () => void;
  onSelectChat: (id: string) => void;
  onRename: (conversation: Conversation) => void;
  onPin: (conversation: Conversation) => void;
  onArchive: (conversation: Conversation) => void;
  onDelete: (conversation: Conversation) => void;
}

const nav = [
  { id: "chat" as const, label: "Chats", icon: MessageSquare },
  { id: "library" as const, label: "Documents", icon: Library },
  { id: "saved" as const, label: "Saved", icon: FileText },
  { id: "models" as const, label: "Models", icon: Bot },
];

export function Sidebar(props: SidebarProps) {
  const [query, setQuery] = useState("");
  const [menuId, setMenuId] = useState<string>();
  const visible = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return props.conversations.filter((item) => !item.archived && (!normalized || item.title.toLowerCase().includes(normalized)));
  }, [props.conversations, query]);
  const groups = useMemo(() => {
    const output = new Map<string, Conversation[]>();
    for (const conversation of visible) {
      const group = conversation.pinned ? "Pinned" : relativeGroup(conversation.updatedAt);
      output.set(group, [...(output.get(group) ?? []), conversation]);
    }
    return output;
  }, [visible]);

  return (
    <aside className={`sidebar ${props.collapsed ? "sidebar-collapsed" : ""}`}>
      <div className="sidebar-top">
        <button className="new-chat" type="button" onClick={props.onNewChat} title="New chat (Ctrl+N)">
          <Plus size={18} /> {!props.collapsed && <span>New chat</span>}
        </button>
        <button className="icon-button sidebar-toggle" type="button" aria-label="Toggle sidebar" onClick={props.onToggle}>
          {props.collapsed ? <PanelLeft size={18} /> : <ChevronLeft size={18} />}
        </button>
      </div>

      {!props.collapsed && (
        <label className="sidebar-search">
          <Search size={15} aria-hidden="true" />
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search chats" aria-label="Search chats" />
          <kbd>Ctrl K</kbd>
        </label>
      )}

      <nav className="primary-nav" aria-label="Main navigation">
        {nav.map(({ id, label, icon: Icon }) => (
          <button key={id} type="button" className={props.view === id ? "active" : ""} onClick={() => props.onView(id)} title={label}>
            <Icon size={17} /> {!props.collapsed && <span>{label}</span>}
          </button>
        ))}
      </nav>

      {!props.collapsed && (
        <div className="history" aria-label="Conversation history">
          {groups.size === 0 && <p className="sidebar-empty">Your conversations will appear here.</p>}
          {[...groups.entries()].map(([group, conversations]) => (
            <section key={group} className="history-group">
              <h2>{group}</h2>
              {conversations.map((conversation) => (
                <div key={conversation.id} className={`history-row ${props.activeConversationId === conversation.id && props.view === "chat" ? "active" : ""}`}>
                  <button className="history-select" type="button" onClick={() => props.onSelectChat(conversation.id)}>
                    {conversation.pinned && <Pin size={12} />}
                    <span>{conversation.title}</span>
                  </button>
                  <button className="history-menu-button" type="button" aria-label={`Options for ${conversation.title}`} onClick={() => setMenuId(menuId === conversation.id ? undefined : conversation.id)}>
                    <MoreHorizontal size={16} />
                  </button>
                  {menuId === conversation.id && (
                    <div className="context-menu" role="menu">
                      <button type="button" onClick={() => { props.onRename(conversation); setMenuId(undefined); }}><Pencil size={14} /> Rename</button>
                      <button type="button" onClick={() => { props.onPin(conversation); setMenuId(undefined); }}><Pin size={14} /> {conversation.pinned ? "Unpin" : "Pin"}</button>
                      <button type="button" onClick={() => { props.onArchive(conversation); setMenuId(undefined); }}><Archive size={14} /> Archive</button>
                      <button type="button" className="danger" onClick={() => { props.onDelete(conversation); setMenuId(undefined); }}><Trash2 size={14} /> Delete</button>
                    </div>
                  )}
                </div>
              ))}
            </section>
          ))}
        </div>
      )}

      <div className="sidebar-bottom">
        <button type="button" className={props.view === "settings" ? "active" : ""} onClick={() => props.onView("settings")} title="Settings (Ctrl+,)">
          <Settings size={17} /> {!props.collapsed && <span>Settings</span>}
        </button>
      </div>
    </aside>
  );
}
