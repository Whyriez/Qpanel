import { useEffect, useId, useRef, useState } from 'react';
import { Download, Loader2, RefreshCw, X } from 'lucide-react';
import type { PythonVersionSummaryDto } from '../../types/api';
import { ModalOverlay } from '../ui/ModalOverlay';
import { toast } from 'sonner';

export function isFullPythonVersion(version: string): boolean {
  return /^3\.(0|[1-9]\d{0,4})\.(0|[1-9]\d{0,4})$/.test(version) && version.split('.').every(part => Number(part) <= 65535);
}

interface Props {
  isOpen: boolean;
  versions: PythonVersionSummaryDto[];
  installedVersions: string[];
  busyVersions: string[];
  unavailableReason: string;
  refreshing: boolean;
  onClose: () => void;
  onInstall: (version: string) => Promise<void>;
  onRefresh: () => Promise<void>;
}
const button = 'inline-flex min-h-10 items-center justify-center gap-2 rounded-[var(--radius-md)] border border-[var(--line)] px-3 py-2 text-xs font-medium disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--primary)]';
const input = 'w-full rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] px-3 py-2 text-sm text-[var(--ink)]';

export function PythonInstallModal({ isOpen, versions, installedVersions, busyVersions, unavailableReason, refreshing, onClose, onInstall, onRefresh }: Props) {
  const id = useId();
  const [selected, setSelected] = useState(versions[0]?.version || '');
  const [custom, setCustom] = useState(false);
  const [customVersion, setCustomVersion] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState('');
  const inFlight = useRef(false);
  const closeButton = useRef<HTMLButtonElement>(null);
  const cycle = useRef(0);
  useEffect(() => { cycle.current++; if (isOpen) { setError(''); setCustom(false); setCustomVersion(''); } }, [isOpen]);
  useEffect(() => { setSelected(value => versions.some(v => v.version === value) ? value : versions[0]?.version || ''); }, [versions]);
  if (!isOpen) return null;
  const version = custom ? customVersion.trim() : selected;
  const installed = installedVersions.includes(version);
  const busy = busyVersions.includes(version);
  const valid = isFullPythonVersion(version) && (custom || versions.some(v => v.version === version));
  const refresh = async () => {
    if (refreshing || inFlight.current) return;
    const current = cycle.current; setError(''); closeButton.current?.focus();
    try { await onRefresh(); }
    catch (cause) { if (cycle.current === current) setError(cause instanceof Error ? cause.message : 'Gagal memuat daftar versi Python.'); }
  };
  return <ModalOverlay aria-labelledby={`${id}-title`} onDismiss={onClose} className="p-4">
    <div className="dialog-panel w-full max-w-lg overflow-hidden rounded-[var(--radius-lg)] border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)]">
      <div className="dialog-header flex items-start justify-between gap-3 border-b border-[var(--line)] p-5">
        <div><h2 id={`${id}-title`} className="text-base font-semibold">Pasang Python</h2><p className="mt-1 text-xs text-[var(--ink-muted)]">Pilih versi untuk dipasang ke server dan digunakan oleh situs.</p></div>
        <button ref={closeButton} type="button" className={button} onClick={onClose} aria-label="Tutup pemasangan Python"><X size={16} /></button>
      </div>
      <form className="dialog-form" onSubmit={event => {
        event.preventDefault();
        if (inFlight.current || !valid || installed || busy || unavailableReason || refreshing) return;
        const current = cycle.current; inFlight.current = true; setSubmitting(true); setError(''); closeButton.current?.focus();
        void onInstall(version).then(() => { if (cycle.current === current) onClose(); })
          .catch(cause => { const message = cause instanceof Error ? cause.message : 'Gagal membuat tugas Python.'; if (cycle.current === current) setError(message); else toast.error(message); })
          .finally(() => { inFlight.current = false; setSubmitting(false); });
      }}>
        <div className="dialog-body space-y-4 p-5">
          <div className="space-y-2">
            <label htmlFor={`${id}-version`} className="block text-xs font-medium">{custom ? 'Nomor versi lengkap' : 'Versi Python'}</label>
            {custom ? <input id={`${id}-version`} className={input} value={customVersion} onChange={event => setCustomVersion(event.target.value)} placeholder="Contoh: 3.13.9" autoComplete="off" required maxLength={13} disabled={submitting} aria-describedby={`${id}-help`} />
              : <select id={`${id}-version`} className={input} value={selected} onChange={event => setSelected(event.target.value)} disabled={submitting || refreshing || !versions.length} required aria-describedby={`${id}-help`}>
                <option value="">{versions.length ? 'Pilih versi Python' : 'Daftar versi belum tersedia'}</option>
                {versions.map(v => <option key={v.version} value={v.version}>Python {v.version}{v.is_default ? ' · Disarankan' : ''}</option>)}
              </select>}
            <p id={`${id}-help`} className="text-xs leading-relaxed text-[var(--ink-muted)]">{custom ? 'Isi tiga bagian angka, misalnya 3.13.9. Pilih rilis stabil yang masih didukung. Ketersediaan versi diperiksa saat tugas berjalan.' : 'Daftar otomatis menampilkan rilis stabil yang masih didukung dan belum terpasang.'}</p>
          </div>
          {!versions.length && !custom && <p role="status" className="text-sm text-[var(--ink-muted)]">Muat daftar versi terbaru atau masukkan nomor versi lengkap untuk memasang Python.</p>}
          <div className="flex flex-wrap gap-3">
            <button type="button" className={button} disabled={submitting || refreshing} onClick={() => void refresh()}><RefreshCw size={14} />{refreshing ? 'Memuat daftar versi…' : 'Muat daftar versi Python'}</button>
            <button type="button" className={`${button} text-[var(--primary)]`} disabled={submitting} onClick={() => { setCustom(value => !value); setError(''); }}>{custom ? 'Pilih dari katalog' : 'Masukkan versi lengkap'}</button>
          </div>
          {custom && version && !valid && <p role="alert" className="text-xs text-[var(--danger)]">Gunakan format versi lengkap 3.x.y, tanpa path atau perintah.</p>}
          {(installed || busy) && <p role="status" className="text-xs text-[var(--ink-muted)]">{installed ? 'Versi ini sudah terpasang.' : 'Versi ini sedang dipasang. Pantau progres pada daftar runtime.'}</p>}
          {unavailableReason && <p role="status" className="text-xs text-[var(--ink-muted)]">{unavailableReason}</p>}
          {error && <p role="alert" className="text-sm text-[var(--danger)]">{error}</p>}
          <p className="text-xs text-[var(--ink-muted)]">Pemasangan berjalan di latar belakang. Dialog dapat ditutup; progres tetap tersedia di Runtime dan Tugas.</p>
        </div>
        <div className="dialog-footer flex flex-wrap justify-end gap-2 border-t border-[var(--line)] p-5">
          <button type="button" className={button} onClick={onClose}>Tutup</button>
          <button type="submit" className={`${button} bg-[var(--primary)] text-[var(--primary-foreground)]`} disabled={!valid || installed || busy || submitting || refreshing || Boolean(unavailableReason)}>{submitting ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}{submitting ? 'Membuat tugas…' : 'Mulai pemasangan'}</button>
        </div>
      </form>
    </div>
  </ModalOverlay>;
}
