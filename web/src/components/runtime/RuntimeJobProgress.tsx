import type { JobItem } from '../../types/api';

export function RuntimeJobProgress({ job, runtime }: { job: JobItem; runtime: string }) {
  return (
    <div className="space-y-2">
      {job.status === 'queued' && <p className="text-xs text-[var(--ink-muted)]">Menunggu giliran worker. Tugas belum dimulai.</p>}
      <div className="flex flex-wrap items-center gap-3">
      <progress
        max={100}
        value={job.progress}
        aria-label={`Progres tugas ${runtime}`}
        className="block h-1.5 w-40 max-w-full accent-[var(--primary)]"
      >
        {job.progress}%
      </progress>
      <a href="/jobs" className="text-xs text-[var(--primary)] underline underline-offset-2">
        Lihat progres
      </a>
      </div>
    </div>
  );
}
