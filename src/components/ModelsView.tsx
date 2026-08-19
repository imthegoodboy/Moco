import { Check, Cpu, Download, Gauge, HardDrive, MemoryStick, PackagePlus, Pause, Play, Sparkles, Square, Trash2, X } from "lucide-react";
import { useMemo, useState } from "react";
import { formatBytes } from "../lib/format";
import type { HardwareInfo, ModelDownloadProgress, ModelInfo } from "../types";

interface ModelsViewProps {
  models: ModelInfo[];
  hardware: HardwareInfo;
  downloads: Record<string, ModelDownloadProgress>;
  busyModelId?: string;
  onImport: () => void;
  onDownload: (model: ModelInfo) => void;
  onPause: (model: ModelInfo) => void;
  onCancel: (model: ModelInfo) => void;
  onLoad: (model: ModelInfo) => void;
  onUnload: () => void;
  onDelete: (model: ModelInfo) => void;
}

type Fit = { label: string; rank: number; detail: string };

function modelFit(model: ModelInfo, hardware: HardwareInfo): Fit {
  if (hardware.availableDiskBytes < model.sizeBytes * 1.12) return { label: "Needs storage", rank: 0, detail: `${formatBytes(model.sizeBytes * 1.12 - hardware.availableDiskBytes)} more free space needed` };
  const ratio = hardware.totalRamBytes / Math.max(model.requiredRamBytes, 1);
  if (ratio < 1) return { label: "Not recommended", rank: 0, detail: `Needs about ${formatBytes(model.requiredRamBytes)} RAM` };
  if (ratio < 1.3) return { label: "May run slowly", rank: 1, detail: "Close other apps before loading" };
  if (ratio < 2) return { label: "Good fit", rank: 2, detail: "Supported on this computer" };
  return { label: "Excellent fit", rank: 3, detail: "Plenty of memory headroom" };
}

function capabilityLevel(tier: string): number {
  return ({ Essential: 1, Everyday: 2, Balanced: 3, Strong: 4, Advanced: 5, Expert: 6 } as Record<string, number>)[tier] ?? 3;
}

export function ModelsView(props: ModelsViewProps) {
  const [tab, setTab] = useState<"discover" | "mine">("discover");
  const [showRecommendation, setShowRecommendation] = useState(false);
  const installed = props.models.filter((model) => model.builtIn || model.status !== "not-downloaded");
  const recommended = useMemo(() => [...props.models]
    .filter((model) => modelFit(model, props.hardware).rank >= 2)
    .sort((a, b) => capabilityLevel(b.capabilityTier) - capabilityLevel(a.capabilityTier) || a.sizeBytes - b.sizeBytes)[0] ?? props.models[0], [props.models, props.hardware]);
  const shown = tab === "discover" ? props.models : installed;

  return (
    <main className="page page-scroll">
      <header className="page-heading page-heading-row">
        <div><p className="eyebrow">LOCAL INFERENCE</p><h1>Models</h1><p>Discover verified GGUF models and find the best fit for this computer.</p></div>
        <div className="heading-actions"><button className="secondary-button" type="button" onClick={() => setShowRecommendation(true)}><Sparkles size={16} /> Find my model</button><button className="primary-button" type="button" onClick={props.onImport}><PackagePlus size={16} /> Import GGUF</button></div>
      </header>

      <section className="hardware-strip">
        <div><Cpu size={18} /><span><small>Processor</small><strong>{props.hardware.cpu}</strong></span></div>
        <div><MemoryStick size={18} /><span><small>Memory</small><strong>{formatBytes(props.hardware.totalRamBytes)}</strong></span></div>
        <div><Gauge size={18} /><span><small>Graphics</small><strong>{props.hardware.gpu}</strong></span></div>
        <div><HardDrive size={18} /><span><small>Free storage</small><strong>{formatBytes(props.hardware.availableDiskBytes)}</strong></span></div>
      </section>

      {showRecommendation && recommended && <section className="recommendation-banner"><div className="recommendation-icon"><Sparkles size={21} /></div><div><p className="eyebrow">BEST MATCH FOR THIS PC</p><h2>{recommended.name}</h2><p>{recommended.description} Moco selected the strongest catalog model with comfortable memory and storage headroom.</p><div><span>{recommended.capabilityTier} capability</span><span>{modelFit(recommended, props.hardware).label}</span><span>{formatBytes(recommended.sizeBytes)} download</span></div></div><button className="icon-button" type="button" onClick={() => setShowRecommendation(false)} aria-label="Close recommendation"><X size={16} /></button></section>}

      <div className="model-tabs"><button type="button" className={tab === "discover" ? "active" : ""} onClick={() => setTab("discover")}>Discover <span>{props.models.length}</span></button><button type="button" className={tab === "mine" ? "active" : ""} onClick={() => setTab("mine")}>My models <span>{installed.length}</span></button></div>
      <div className="section-title"><div><h2>{tab === "discover" ? "Model catalog" : "On this device"}</h2><p>{tab === "discover" ? "Verified publishers · Integrity checked after download" : `${installed.length} model${installed.length === 1 ? "" : "s"} ready or in progress`}</p></div><span className="compatibility"><Check size={14} /> Hardware checked locally</span></div>

      <div className="model-list">
        {shown.map((model) => {
          const fit = modelFit(model, props.hardware);
          const progress = props.downloads[model.id];
          const downloaded = model.builtIn || ["unloaded", "loaded", "loading"].includes(model.status);
          return <article className={`model-card catalog-card ${model.isDefault ? "selected" : ""}`} key={model.id}>
            <div className="model-symbol">{model.name.slice(0, 1)}</div>
            <div className="model-main">
              <div className="model-name"><h3>{model.name}</h3>{model.builtIn && <span>Included</span>}{model.isDefault && <span>Default</span>}<span className={`fit-tag fit-${fit.rank}`}>{fit.label}</span></div>
              <p>{model.description}</p>
              <div className="capability-row"><span>Capability</span><div aria-label={`${model.capabilityTier} capability`}>{[1,2,3,4,5,6].map((level) => <i className={level <= capabilityLevel(model.capabilityTier) ? "filled" : ""} key={level} />)}</div><strong>{model.capabilityTier}</strong><span className="best-for">Best for: {model.bestFor}</span></div>
              <div className="model-stats"><span>Parameters <strong>{model.parameters}</strong></span><span>Download <strong>{model.builtIn ? "Included" : formatBytes(model.sizeBytes)}</strong></span><span>RAM <strong>{formatBytes(model.requiredRamBytes)}</strong></span><span>Context <strong>{(model.contextLength / 1024).toFixed(0)}K</strong></span></div>
              {progress && ["downloading", "paused"].includes(progress.status) && <div className="download-progress"><div><span>{progress.status === "paused" ? "Paused" : `Downloading · ${formatBytes(progress.bytesPerSecond)}/s`}</span><strong>{progress.percent.toFixed(0)}%</strong></div><progress max="100" value={progress.percent} /><small>{formatBytes(progress.downloadedBytes)} of {formatBytes(progress.totalBytes)}</small></div>}
            </div>
            <div className="model-actions">
              {progress?.status === "downloading" ? <><button className="secondary-button" type="button" onClick={() => props.onPause(model)}><Pause size={14} /> Pause</button><button className="icon-button" type="button" onClick={() => props.onCancel(model)} title="Cancel download"><X size={16} /></button></> :
               progress?.status === "paused" || model.status === "paused" ? <><button className="primary-button" type="button" onClick={() => props.onDownload(model)}><Download size={14} /> Resume</button><button className="icon-button" type="button" onClick={() => props.onCancel(model)} title="Cancel download"><X size={16} /></button></> :
               !downloaded ? <button className="primary-button" type="button" disabled={fit.rank === 0} onClick={() => props.onDownload(model)}><Download size={14} /> Download</button> :
               model.status === "loaded" ? <button className="secondary-button" type="button" onClick={props.onUnload}><Square size={14} /> Unload</button> : <button className="primary-button" type="button" disabled={Boolean(props.busyModelId)} onClick={() => props.onLoad(model)}><Play size={14} /> Load</button>}
              {!model.builtIn && downloaded && <button className="icon-button" type="button" onClick={() => props.onDelete(model)} title="Remove downloaded model"><Trash2 size={16} /></button>}
            </div>
          </article>;
        })}
      </div>
      {tab === "mine" && !shown.length && <section className="download-note"><Download size={19} /><div><strong>No optional models yet</strong><p>The default LFM model is included with Moco. Use Discover to add more models with one click.</p></div></section>}
    </main>
  );
}
