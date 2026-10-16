import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { JobQueueNotice } from '../JobQueueNotice';
import { RuntimeJobProgress } from '../../runtime/RuntimeJobProgress';
import { jobProgressMessage, jobSnapshotEvent } from '../../../lib/jobProgress';
import type { JobItem, JobStatus } from '../../../types/api';

const job: JobItem = {
  id: 'php-repo', user_id: 1, name: 'php.repo.setup', status: 'queued', progress: 0,
  payload_json: '{}', result_json: null, error_message: null,
  retry_count: 0, max_retries: 3, created_at: 1, updated_at: 1,
};

describe('Queue and monitor progress', () => {
  it('explains waiting without implying installation has started', () => {
    const html = renderToString(<JobQueueNotice jobId={job.id} />);
    expect(html).toContain('Tugas ini masih antre. Eksekusi belum dimulai.');
    expect(html).toContain('Worker memproses satu tugas pada satu waktu.');
  });

  it('shows queued repository progress at its actual value and links to jobs', () => {
    const html = renderToString(<RuntimeJobProgress job={job} runtime="repositori PHP" />);
    expect(html).toContain('Menunggu giliran worker. Tugas belum dimulai.');
    expect(html).toContain('value="0"');
    expect(html).toContain('href="/jobs"');
    expect(html).toContain('Progres tugas repositori PHP');
  });

  it('does not label running jobs as waiting', () => {
    const html = renderToString(<RuntimeJobProgress job={{ ...job, status: 'running', progress: 10 }} runtime="repositori PHP" />);
    expect(html).not.toContain('Tugas belum dimulai');
    expect(html).toContain('value="10"');
  });

  it.each<JobStatus>(['queued', 'running', 'succeeded', 'failed', 'cancelled'])('provides a meaningful fallback for %s snapshots', status => {
    expect(jobProgressMessage(status, null)).toBeTruthy();
    expect(jobProgressMessage(status, null)).not.toMatch(/0%|Inisialisasi/);
  });

  it('preserves complete failure and retry diagnostics', () => {
    const diagnostic = 'E: package missing\n' + 'long diagnostic '.repeat(400) + 'END';
    for (const status of ['queued', 'failed'] as const) {
      expect(jobSnapshotEvent({ ...job, status, error_message: diagnostic }).message).toBe(diagnostic);
    }
  });

  it('does not reuse stale attempt errors for running or successful snapshots', () => {
    for (const status of ['running', 'succeeded'] as const) {
      const event = jobSnapshotEvent({ ...job, status, error_message: 'Previous attempt failed' });
      expect(event.message).not.toContain('Previous attempt failed');
    }
  });
});
