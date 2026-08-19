import { AlertTriangle, CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useEffect, useState } from "react";

export interface Toast { id: string; type: "success" | "error" | "info"; message: string }
export interface ConfirmState { title: string; message: string; confirmLabel: string; danger?: boolean; onConfirm: () => void }
export interface PromptState { title: string; message: string; value: string; confirmLabel: string; onConfirm: (value: string) => void }

export function ToastStack({ toasts, onDismiss }: { toasts: Toast[]; onDismiss: (id: string) => void }) {
  return <div className="toast-stack" aria-live="polite">{toasts.map((toast) => { const Icon = toast.type === "success" ? CheckCircle2 : toast.type === "error" ? XCircle : Info; return <div className={`toast toast-${toast.type}`} key={toast.id}><Icon size={17} /><span>{toast.message}</span><button type="button" aria-label="Dismiss" onClick={() => onDismiss(toast.id)}><X size={14} /></button></div>; })}</div>;
}

export function ConfirmDialog({ state, onClose }: { state?: ConfirmState; onClose: () => void }) {
  if (!state) return null;
  return <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}><div className="dialog" role="alertdialog" aria-modal="true"><span className={`dialog-icon ${state.danger ? "danger" : ""}`}><AlertTriangle size={20} /></span><h2>{state.title}</h2><p>{state.message}</p><div className="dialog-actions"><button className="secondary-button" type="button" onClick={onClose}>Cancel</button><button className={state.danger ? "danger-button" : "primary-button"} type="button" onClick={() => { state.onConfirm(); onClose(); }}>{state.confirmLabel}</button></div></div></div>;
}

export function PromptDialog({ state, onClose }: { state?: PromptState; onClose: () => void }) {
  const [value, setValue] = useState(state?.value ?? "");
  useEffect(() => setValue(state?.value ?? ""), [state]);
  if (!state) return null;
  return <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}><form className="dialog" role="dialog" aria-modal="true" onSubmit={(event) => { event.preventDefault(); if (value.trim()) { state.onConfirm(value.trim()); onClose(); } }}><h2>{state.title}</h2><p>{state.message}</p><input autoFocus value={value} onChange={(event) => setValue(event.target.value)} /><div className="dialog-actions"><button className="secondary-button" type="button" onClick={onClose}>Cancel</button><button className="primary-button" type="submit" disabled={!value.trim()}>{state.confirmLabel}</button></div></form></div>;
}

