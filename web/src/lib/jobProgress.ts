import type { JobItem, JobProgressEvent } from '../types/api';

export function canCancelJob(job: JobItem): boolean {
  return job.status === 'queued' || (job.status === 'running' && !['php.install', 'php.repo.setup', 'database.server.install', 'cloudflare.install', 'cloudflare.change', 'email.install', 'email.mailbox', 'email.send', 'email.deliver'].includes(job.name));
}

export function jobProgressMessage(status: JobProgressEvent['status'], message: string | null): string {
  if (message) return message;
  switch (status) {
    case 'queued': return 'Menunggu giliran eksekusi. Tugas belum dimulai.';
    case 'running': return 'Tugas sedang berjalan. Menunggu pembaruan progres.';
    case 'succeeded': return 'Tugas selesai.';
    case 'failed': return 'Tugas gagal.';
    case 'cancelled': return 'Tugas dibatalkan.';
  }
}

export function jobSnapshotEvent(job: JobItem): JobProgressEvent {
  return {
    job_id: job.id, status: job.status, progress: job.progress,
    message: jobProgressMessage(job.status, job.status === 'running' || job.status === 'succeeded' ? null : job.error_message), timestamp: job.updated_at,
  };
}

/** Stable server sequences preserve equal-timestamp output and deduplicate reconnects. */
export function mergeJobEvents(previous: JobProgressEvent[], incoming: JobProgressEvent[]): JobProgressEvent[] {
  const all = [...previous, ...incoming];
  const persisted = all.some(event => event.sequence !== undefined);
  const map = new Map<string, JobProgressEvent>();
  for (const event of persisted ? all.filter(item => item.sequence !== undefined) : all) {
    const key = event.sequence !== undefined ? `seq:${event.sequence}` : JSON.stringify([event.job_id, event.status, event.progress, event.timestamp, event.message]);
    map.set(key, event);
  }
  return [...map.values()].sort((a, b) => a.sequence !== undefined && b.sequence !== undefined ? a.sequence - b.sequence : a.timestamp - b.timestamp).slice(-5000);
}
