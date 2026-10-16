import type { JobItem, JobProgressEvent } from '../types/api';

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
