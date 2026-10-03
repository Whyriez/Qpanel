import React, { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Search,
  RefreshCw,
  ChevronLeft,
  ChevronRight,
  Eye,
  X,
  FileCode,
  AlertCircle,
} from 'lucide-react';
import { AuditLogItem } from '../types/api';
import { fetchAuditLogs } from '../api/client';
import { StatusPill } from '../components/ui/StatusPill';
import { EmptyState } from '../components/ui/EmptyState';

export const AuditLogPage: React.FC = () => {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<AuditLogItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Filters & Pagination
  const [page, setPage] = useState(1);
  const [perPage] = useState(15);
  const [totalPages, setTotalPages] = useState(1);
  const [totalItems, setTotalItems] = useState(0);
  const [selectedStatus, setSelectedStatus] = useState<string>('');
  const [searchAction, setSearchAction] = useState<string>('');

  // Selected log for JSON modal
  const [inspectLog, setInspectLog] = useState<AuditLogItem | null>(null);

  const loadLogs = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetchAuditLogs({
        page,
        per_page: perPage,
        status: selectedStatus || undefined,
        action: searchAction.trim() || undefined,
      });
      setLogs(res.items);
      setTotalPages(res.total_pages || 1);
      setTotalItems(res.total);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Gagal memuat log audit.');
    } finally {
      setLoading(false);
    }
  }, [page, perPage, selectedStatus, searchAction]);

  useEffect(() => {
    loadLogs();
  }, [loadLogs]);

  const formatTimestamp = (ts: number) => {
    return new Date(ts * 1000).toLocaleString('id-ID', {
      dateStyle: 'medium',
      timeStyle: 'medium',
    });
  };

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-[var(--ink)]">
            {t('audit.title')}
          </h1>
          <p className="text-xs text-[var(--ink-muted)] mt-1">
            {t('audit.subtitle')}
          </p>
        </div>

        <button
          onClick={loadLogs}
          disabled={loading}
          className="self-start sm:self-auto py-1.5 px-3 rounded-[var(--radius-sm)] border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] hover:bg-[var(--raised)] text-xs font-medium flex items-center gap-1.5 cursor-pointer"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
          <span>{t('common.refresh')}</span>
        </button>
      </div>

      {error && (
        <div className="p-3 rounded-[var(--radius-sm)] bg-[var(--danger)]/10 border border-[var(--danger)]/30 flex items-center gap-2 text-xs text-[var(--danger)]">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Filter Toolbar */}
      <div className="flex flex-col sm:flex-row gap-3 items-center justify-between bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-md)] p-3">
        <div className="flex items-center gap-2 w-full sm:w-auto">
          <div className="relative w-full sm:w-64">
            <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ink-muted)]" />
            <input
              type="text"
              value={searchAction}
              onChange={(e) => {
                setSearchAction(e.target.value);
                setPage(1);
              }}
              placeholder="Cari aksi (cth: auth.login)..."
              className="w-full bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] pl-9 pr-3 py-1.5 text-xs text-[var(--ink)] placeholder-[var(--ink-muted)] focus:outline-none focus:border-[var(--primary)]"
            />
          </div>

          <select
            value={selectedStatus}
            onChange={(e) => {
              setSelectedStatus(e.target.value);
              setPage(1);
            }}
            className="bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] px-2.5 py-1.5 text-xs text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
          >
            <option value="">{t('audit.filterStatus')}</option>
            <option value="success">Sukses</option>
            <option value="failure">Gagal</option>
          </select>
        </div>

        <div className="text-xs text-[var(--ink-muted)] self-end sm:self-auto">
          Total: <span className="font-semibold text-[var(--ink)]">{totalItems}</span> entri
        </div>
      </div>

      {/* Logs Table */}
      <div className="bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-md)] overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="border-b border-[var(--line)] bg-[var(--raised)]/50 text-[var(--ink-muted)] font-medium">
                <th className="py-2.5 px-4">{t('audit.timestamp')}</th>
                <th className="py-2.5 px-4">{t('audit.actor')}</th>
                <th className="py-2.5 px-4">{t('audit.action')}</th>
                <th className="py-2.5 px-4">{t('audit.target')}</th>
                <th className="py-2.5 px-4">{t('audit.ip')}</th>
                <th className="py-2.5 px-4">{t('audit.status')}</th>
                <th className="py-2.5 px-4 text-right">Detail</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--line)] text-[var(--ink)]">
              {logs.length === 0 ? (
                <tr>
                  <td colSpan={7} className="py-12 text-center text-xs text-[var(--ink-muted)]">
                    <EmptyState
                      title={t('audit.noLogs')}
                      description="Belum ada aktivitas audit log yang tercatat atau cocok dengan filter."
                    />
                  </td>
                </tr>
              ) : (
                logs.map((item) => (
                  <tr
                    key={item.id}
                    className="hover:bg-[var(--raised)]/40 transition-colors"
                  >
                    <td className="py-2.5 px-4 font-mono text-[11px] text-[var(--ink-muted)] whitespace-nowrap">
                      {formatTimestamp(item.created_at)}
                    </td>
                    <td className="py-2.5 px-4 font-semibold whitespace-nowrap">
                      {item.actor_username}
                    </td>
                    <td className="py-2.5 px-4 font-mono text-[11px] text-[var(--primary)] whitespace-nowrap">
                      {item.action}
                    </td>
                    <td className="py-2.5 px-4 text-[var(--ink-muted)] truncate max-w-xs">
                      {item.target_resource}
                    </td>
                    <td className="py-2.5 px-4 font-mono text-[11px] text-[var(--ink-muted)] whitespace-nowrap">
                      {item.ip_address || '-'}
                    </td>
                    <td className="py-2.5 px-4 whitespace-nowrap">
                      <StatusPill
                        label={item.status === 'success' ? 'Sukses' : 'Gagal'}
                        variant={item.status === 'success' ? 'success' : 'danger'}
                      />
                    </td>
                    <td className="py-2.5 px-4 text-right whitespace-nowrap">
                      <button
                        onClick={() => setInspectLog(item)}
                        className="p-1 rounded text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer"
                        title="Lihat detail JSON"
                      >
                        <Eye className="w-3.5 h-3.5" />
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        {/* Pagination Footer */}
        {totalPages > 1 && (
          <div className="flex items-center justify-between px-4 py-3 border-t border-[var(--line)] bg-[var(--raised)]/30 text-xs">
            <span className="text-[var(--ink-muted)]">
              Halaman {page} dari {totalPages}
            </span>
            <div className="flex items-center gap-1.5">
              <button
                disabled={page <= 1}
                onClick={() => setPage((p) => Math.max(1, p - 1))}
                className="p-1 rounded border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] hover:bg-[var(--raised)] disabled:opacity-40 cursor-pointer"
              >
                <ChevronLeft className="w-3.5 h-3.5" />
              </button>
              <button
                disabled={page >= totalPages}
                onClick={() => setPage((p) => Math.min(totalPages, p + 1))}
                className="p-1 rounded border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] hover:bg-[var(--raised)] disabled:opacity-40 cursor-pointer"
              >
                <ChevronRight className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        )}
      </div>

      {/* JSON Payload Inspector Modal */}
      {inspectLog && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm"
          onClick={() => setInspectLog(null)}
        >
          <div
            className="w-full max-w-lg bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-lg)] shadow-[var(--shadow-floating)] p-5 space-y-3"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-[var(--line)] pb-2.5">
              <div className="flex items-center gap-2">
                <FileCode className="w-4 h-4 text-[var(--primary)]" />
                <h3 className="font-semibold text-xs text-[var(--ink)]">
                  Detail Audit #{inspectLog.id} ({inspectLog.action})
                </h3>
              </div>
              <button
                onClick={() => setInspectLog(null)}
                className="p-1 rounded text-[var(--ink-muted)] hover:text-[var(--ink)] cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="space-y-2 text-xs">
              <div className="grid grid-cols-2 gap-2 text-[11px] text-[var(--ink-muted)]">
                <div>Aktor: <span className="font-semibold text-[var(--ink)]">{inspectLog.actor_username}</span></div>
                <div>Status: <span className="font-semibold text-[var(--ink)]">{inspectLog.status}</span></div>
                <div>Target: <span className="font-semibold text-[var(--ink)]">{inspectLog.target_resource}</span></div>
                <div>Waktu: <span className="font-mono text-[var(--ink)]">{formatTimestamp(inspectLog.created_at)}</span></div>
              </div>

              <div>
                <span className="block font-medium text-[var(--ink)] mb-1">
                  {t('audit.details')}
                </span>
                <pre className="p-3 bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] font-mono text-[11px] text-[var(--ink)] overflow-x-auto max-h-60 whitespace-pre-wrap">
                  {inspectLog.details_json
                    ? JSON.stringify(JSON.parse(inspectLog.details_json), null, 2)
                    : '// Tidak ada rincian metadata'}
                </pre>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
