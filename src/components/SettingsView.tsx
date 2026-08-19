import { Bot, Database, HardDrive, KeyRound, Monitor, Save, Shield, SlidersHorizontal } from "lucide-react";
import { useEffect, useState } from "react";
import type { AppSettings, HardwareInfo } from "../types";

interface SettingsViewProps {
  settings: AppSettings;
  hardware: HardwareInfo;
  dataDirectory: string;
  onSave: (settings: AppSettings) => Promise<void>;
  onClear: (scope: "chats" | "documents" | "all") => void;
}

const tabs = [
  { id: "general", label: "General", icon: Monitor },
  { id: "ai", label: "AI & provider", icon: Bot },
  { id: "generation", label: "Generation", icon: SlidersHorizontal },
  { id: "privacy", label: "Privacy & data", icon: Shield },
];

export function SettingsView(props: SettingsViewProps) {
  const [tab, setTab] = useState("general");
  const [draft, setDraft] = useState(props.settings);
  const [saving, setSaving] = useState(false);
  useEffect(() => setDraft(props.settings), [props.settings]);
  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => setDraft((current) => ({ ...current, [key]: value }));
  const saveSettings = async () => { setSaving(true); try { await props.onSave(draft); } finally { setSaving(false); } };

  return (
    <main className="settings-page">
      <aside className="settings-nav"><div><p className="eyebrow">PREFERENCES</p><h1>Settings</h1></div>{tabs.map(({ id, label, icon: Icon }) => <button key={id} type="button" className={tab === id ? "active" : ""} onClick={() => setTab(id)}><Icon size={16} /> {label}</button>)}</aside>
      <div className="settings-content page-scroll">
        {tab === "general" && <section className="settings-section"><header><h2>General</h2><p>Adjust how Moco looks and starts.</p></header><div className="setting-row"><div><strong>Appearance</strong><span>A pure monochrome interface in light or dark.</span></div><select value={draft.theme} onChange={(event) => update("theme", event.target.value as AppSettings["theme"])}><option value="dark">Dark</option><option value="light">Light</option><option value="system">Use system</option></select></div><div className="setting-row"><div><strong>Local storage</strong><span>{props.dataDirectory}</span></div><span className="static-value"><HardDrive size={15} /> On this device</span></div><div className="setting-row"><div><strong>Startup status</strong><span>Load the model only when you first send a message.</span></div><span className="static-value">On demand</span></div></section>}

        {tab === "ai" && <section className="settings-section"><header><h2>AI & provider</h2><p>Local mode is private and needs no key. API mode is optional.</p></header><div className="provider-choice"><button type="button" className={draft.provider === "local" ? "active" : ""} onClick={() => update("provider", "local")}><Shield size={19} /><strong>Local model</strong><span>Runs entirely on this device</span></button><button type="button" className={draft.provider === "api" ? "active" : ""} onClick={() => update("provider", "api")}><KeyRound size={19} /><strong>API provider</strong><span>OpenAI-compatible endpoint</span></button></div>{draft.provider === "api" && <div className="provider-fields"><div className="privacy-warning"><Shield size={17} /><p><strong>Cloud disclosure</strong> Messages, conversation context, and selected document excerpts will be sent to the endpoint below.</p></div><label>Base URL<input value={draft.apiEndpoint} onChange={(event) => update("apiEndpoint", event.target.value)} placeholder="https://api.openai.com/v1" /></label><label>Model<input value={draft.apiModel} onChange={(event) => update("apiModel", event.target.value)} placeholder="gpt-4.1-mini" /></label><label>API key<input type="password" autoComplete="off" value={draft.apiKey} onChange={(event) => update("apiKey", event.target.value)} placeholder="Enter your key" /></label><label className="check-row"><input type="checkbox" checked={draft.rememberApiKey} onChange={(event) => update("rememberApiKey", event.target.checked)} /><span><strong>Remember on this device</strong><small>Leave off on a shared computer.</small></span></label></div>}<label className="full-field">Custom instructions<textarea value={draft.customInstructions} onChange={(event) => update("customInstructions", event.target.value)} placeholder="For example: Explain technical ideas in plain language." rows={5} /></label></section>}

        {tab === "generation" && <section className="settings-section"><header><h2>Generation</h2><p>Sensible defaults work well. Change these only when needed.</p></header><label className="slider-row"><span><strong>Temperature</strong><small>Lower is focused; higher is more varied.</small></span><input type="range" min="0" max="2" step="0.05" value={draft.temperature} onChange={(event) => update("temperature", Number(event.target.value))} /><output>{draft.temperature.toFixed(2)}</output></label><label className="slider-row"><span><strong>Top P</strong><small>Nucleus sampling threshold.</small></span><input type="range" min="0.1" max="1" step="0.05" value={draft.topP} onChange={(event) => update("topP", Number(event.target.value))} /><output>{draft.topP.toFixed(2)}</output></label><div className="setting-row"><div><strong>Maximum output</strong><span>Upper limit for one answer.</span></div><select value={draft.maxTokens} onChange={(event) => update("maxTokens", Number(event.target.value))}><option value={512}>512 tokens</option><option value={1024}>1,024 tokens</option><option value={2048}>2,048 tokens</option><option value={4096}>4,096 tokens</option></select></div><div className="setting-row"><div><strong>Context size</strong><span>More context uses more memory.</span></div><select value={draft.contextSize} onChange={(event) => update("contextSize", Number(event.target.value))}><option value={4096}>4K</option><option value={8192}>8K</option><option value={16384}>16K</option><option value={32768}>32K</option></select></div><div className="split-fields"><label>Response style<select value={draft.responseStyle} onChange={(event) => update("responseStyle", event.target.value)}><option value="balanced">Balanced</option><option value="simple">Simple</option><option value="professional">Professional</option><option value="technical">Technical</option><option value="academic">Academic</option></select></label><label>Response length<select value={draft.responseLength} onChange={(event) => update("responseLength", event.target.value)}><option value="short">Short</option><option value="normal">Normal</option><option value="detailed">Detailed</option></select></label></div><details className="advanced-settings"><summary>Advanced runtime settings</summary><div className="split-fields"><label>CPU threads<input type="number" min="0" max={props.hardware.logicalCores} value={draft.cpuThreads} onChange={(event) => update("cpuThreads", Number(event.target.value))} /><small>0 uses automatic detection.</small></label><label>GPU layers<input type="number" min="0" max="999" value={draft.gpuLayers} onChange={(event) => update("gpuLayers", Number(event.target.value))} /><small>Use 0 for the bundled CPU runtime.</small></label></div></details></section>}

        {tab === "privacy" && <section className="settings-section"><header><h2>Privacy & data</h2><p>Moco has no telemetry and no online account.</p></header><div className="privacy-summary"><Shield size={22} /><div><strong>Local-first by design</strong><p>Chat history, documents, vectors, settings, feedback, and logs stay in your private application directory. Local mode makes no model API request.</p></div></div><div className="setting-row"><div><strong>Telemetry</strong><span>Usage analytics and crash reporting.</span></div><span className="static-value"><Shield size={14} /> Always off</span></div><div className="danger-zone"><div><Database size={18} /><div><strong>Delete local data</strong><p>These actions cannot be undone. Installed models are preserved.</p></div></div><div><button type="button" onClick={() => props.onClear("chats")}>Delete chats</button><button type="button" onClick={() => props.onClear("documents")}>Delete documents</button><button type="button" className="danger" onClick={() => props.onClear("all")}>Clear everything</button></div></div></section>}

        <div className="settings-save"><button className="primary-button" type="button" disabled={saving} onClick={() => void saveSettings()}><Save size={16} /> {saving ? "Saving…" : "Save settings"}</button></div>
      </div>
    </main>
  );
}

