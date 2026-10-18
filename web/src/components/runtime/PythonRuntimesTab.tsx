import { useCallback, useEffect, useRef, useState } from 'react';
import { Download, Loader2, RefreshCw, Trash2 } from 'lucide-react';
import { fetchPythonCatalog, installPythonVersion, removePythonVersion } from '../../api/client';
import type { JobItem, PythonRuntimeListApiResponse } from '../../types/api';
import { runtimeJobVersion, useRuntimeJobs } from './useRuntimeJobs';
import { RuntimeJobProgress } from './RuntimeJobProgress';
import { ConfirmDangerDialog } from '../ui/ConfirmDangerDialog';
import { toast } from 'sonner';

const names = new Set(['python.install']);
const secondaryButton = 'inline-flex min-h-10 items-center justify-center gap-2 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] px-3 py-2 text-xs font-medium text-[var(--ink)] hover:bg-[var(--bg)] disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--primary)]';
const primaryButton = 'inline-flex min-h-10 items-center justify-center gap-2 rounded-[var(--radius-md)] bg-[var(--primary)] px-3 py-2 text-xs font-semibold text-white hover:opacity-90 disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--primary)]';
export function PythonRuntimesTab() {
  const [data, setData] = useState<PythonRuntimeListApiResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState<string | null>(null);
  const [remove, setRemove] = useState<string | null>(null);
  const submitting = useRef(false);
  const load = useCallback(async () => {
    setLoading(true); setError('');
    try { const result = await fetchPythonCatalog(); setData(result); return result; }
    catch (cause) { setError(cause instanceof Error ? cause.message : 'Gagal memuat Python.'); return null; }
    finally { setLoading(false); }
  }, []);
  const finished = useCallback(async (job: JobItem) => {
    const result = await load();
    return Boolean(result && (job.status !== 'succeeded' || result.versions.some(v => v.version === runtimeJobVersion(job) && v.is_installed)));
  }, [load]);
  const jobs = useRuntimeJobs(finished, names);
  useEffect(() => { void load(); }, [load]);
  const install = async (version: string) => {
    if (submitting.current || jobs.loading || jobs.error) return;
    submitting.current = true; setBusy(version);
    try {
      const job = await installPythonVersion({ version }); jobs.track(job);
      toast.success(`Pemasangan Python ${version} masuk antrean`, { description: 'Progres tetap tersedia di halaman Tugas.' });
    } catch (cause) { toast.error(cause instanceof Error ? cause.message : 'Gagal membuat tugas Python.'); }
    finally { submitting.current = false; setBusy(null); }
  };
  const confirmRemove = async () => {
    if (!remove || submitting.current) return;
    const version = remove; setRemove(null); setBusy(version); submitting.current = true;
    try { await removePythonVersion(version); await load(); toast.success(`Python ${version} dihapus`); }
    catch (cause) { toast.error(cause instanceof Error ? cause.message : 'Gagal menghapus Python.'); }
    finally { submitting.current = false; setBusy(null); }
  };
  const versions = data?.versions.filter(v => v.is_installed || data.catalog_ready !== false) ?? [];
  return <section className="space-y-4" aria-label="Runtime Python">
    <div className="flex flex-wrap items-center justify-between gap-3">
      <div><h2 className="text-sm font-semibold">Versi Python</h2><p className="text-xs text-[var(--ink-muted)]">Pasang CPython untuk dipilih situs Django, Flask, FastAPI, atau aplikasi sendiri.</p></div>
      <button type="button" onClick={() => void load()} disabled={loading} className={secondaryButton}><RefreshCw size={14} />Muat ulang Python</button>
    </div>
    {(error || jobs.error) && <p role="alert" className="text-sm text-[var(--danger)]">{error || jobs.error}</p>}
    {data?.catalog_ready === false && <p role="status" className="text-sm text-[var(--ink-muted)]">Katalog Python belum tersedia atau sudah kedaluwarsa. Pilih Perbarui katalog sekarang untuk memuat versi yang dapat dipasang.</p>}
    {loading && !data && <p role="status">Memuat runtime Python…</p>}
    {!loading && data && versions.length === 0 && <p className="text-sm text-[var(--ink-muted)]">Belum ada runtime Python yang terpasang.</p>}
    <div className="divide-y divide-[var(--line)] rounded-[var(--radius-lg)] border border-[var(--line)] bg-[var(--surface)]">
      {versions.map(version => {
        const job = jobs.jobs.find(j => runtimeJobVersion(j) === version.version);
        const disabled = Boolean(busy || job || loading || jobs.loading || jobs.error || error);
        return <article key={version.version} className="p-4 space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div><h3 className="text-sm font-semibold">Python {version.version}</h3><p className="text-xs text-[var(--ink-muted)]">{version.is_installed ? 'Terpasang' : 'Tersedia untuk dipasang'} · {version.description}</p>{version.install_path && <p className="mt-1 break-all text-xs font-mono text-[var(--ink-muted)]">{version.install_path}</p>}</div>
            {version.is_installed ? <button type="button" disabled={disabled} onClick={() => setRemove(version.version)} className={secondaryButton}><Trash2 size={14} />Hapus Python {version.version}</button>
              : <button type="button" disabled={disabled} onClick={() => void install(version.version)} className={primaryButton}>{busy === version.version || job ? <Loader2 size={14} className="animate-spin" /> : <Download size={14} />}{job ? 'Sedang dipasang' : `Pasang Python ${version.version}`}</button>}
          </div>
          {job && <RuntimeJobProgress job={job} runtime={`Python ${version.version}`} />}
        </article>;
      })}
    </div>
    {jobs.jobs.filter(job => !versions.some(v => v.version === runtimeJobVersion(job))).map(job => <div key={job.id} className="rounded border border-[var(--line)] p-4"><p className="mb-2 text-sm">Pemasangan Python {runtimeJobVersion(job)}</p><RuntimeJobProgress job={job} runtime={`Python ${runtimeJobVersion(job) ?? ''}`} /></div>)}
    <ConfirmDangerDialog isOpen={Boolean(remove)} title={`Hapus Python ${remove ?? ''}?`} description="Runtime yang masih dipakai situs tidak dapat dihapus. File proyek dan virtual environment situs tetap tersimpan." confirmMatchText={remove ?? ''} confirmButtonLabel="Hapus runtime" onConfirm={() => void confirmRemove()} onClose={() => setRemove(null)} />
  </section>;
}
