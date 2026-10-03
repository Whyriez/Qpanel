import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RefreshCw, Plus, Cpu, HardDrive, Network, Layers } from 'lucide-react';
import { fetchSystemInfo } from '../api/client';
import { SystemInfoResponse } from '../types/api';
import { StatusPill } from '../components/ui/StatusPill';
import { PulseStrip } from '../components/ui/PulseStrip';
import { Skeleton } from '../components/ui/Skeleton';
import { toast } from 'sonner';

export const DashboardPage: React.FC = () => {
  const { t } = useTranslation();
  const [systemInfo, setSystemInfo] = useState<SystemInfoResponse | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadData = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const info = await fetchSystemInfo();
      setSystemInfo(info);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Gagal memuat status sistem';
      setError(msg);
      toast.error('Gagal mengambil data sistem', { description: msg });
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  return (
    <div className="space-y-6">
      {/* Top Banner Status */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-4 border-b border-[var(--line)]">
        <div>
          <div className="flex items-center gap-3">
            <h1 className="text-xl font-bold tracking-tight text-[var(--ink)]">
              {t('nav.dashboard')}
            </h1>
            {systemInfo?.agent_connected ? (
              <StatusPill variant="success" label="Agent Terhubung" pulse />
            ) : (
              <StatusPill variant="danger" label="Agent Terputus" />
            )}
          </div>
          <p className="text-xs text-[var(--ink-muted)] mt-1">
            {t('dashboard.overview')} · {t('dashboard.attentionNeeded', { count: 1 })}
          </p>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={loadData}
            disabled={isLoading}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium border border-[var(--line)] rounded-[var(--radius-sm)] bg-[var(--surface)] text-[var(--ink)] hover:bg-[var(--raised)] disabled:opacity-50 transition-colors"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isLoading ? 'animate-spin' : ''}`} />
            <span>{t('common.refresh')}</span>
          </button>
          <button
            onClick={() => toast.info('Fitur pembuatan situs akan aktif pada Phase 3')}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-[var(--primary)] text-[var(--primary-foreground)] rounded-[var(--radius-sm)] hover:bg-[var(--primary-hover)] transition-colors"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>{t('dashboard.sitesTable.newSite')}</span>
          </button>
        </div>
      </div>

      {/* IPC & System Info Card */}
      <div className="p-4 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)]">
        <h2 className="text-xs font-semibold uppercase tracking-wider text-[var(--ink-muted)] mb-3">
          Status Komponen Platform (Phase 0 Foundation)
        </h2>

        {isLoading ? (
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <Skeleton className="h-14" />
            <Skeleton className="h-14" />
            <Skeleton className="h-14" />
            <Skeleton className="h-14" />
          </div>
        ) : error ? (
          <div className="p-3 bg-[var(--danger)]/10 text-[var(--danger)] rounded-[var(--radius-sm)] text-xs flex items-center justify-between">
            <span>{error}</span>
            <button onClick={loadData} className="underline font-semibold ml-2">Coba lagi</button>
          </div>
        ) : (
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-xs">
            <div className="p-3 rounded-[var(--radius-sm)] bg-[var(--raised)] border border-[var(--line)]">
              <span className="text-[var(--ink-muted)] block text-[11px] mb-1">HTTP API (axum)</span>
              <span className="font-mono font-semibold text-sm">v{systemInfo?.api_version}</span>
            </div>
            <div className="p-3 rounded-[var(--radius-sm)] bg-[var(--raised)] border border-[var(--line)]">
              <span className="text-[var(--ink-muted)] block text-[11px] mb-1">Root Agent (IPC)</span>
              <span className="font-mono font-semibold text-sm">
                {systemInfo?.agent_version ? `v${systemInfo.agent_version}` : 'Offline'}
              </span>
            </div>
            <div className="p-3 rounded-[var(--radius-sm)] bg-[var(--raised)] border border-[var(--line)]">
              <span className="text-[var(--ink-muted)] block text-[11px] mb-1">Protokol RPC</span>
              <span className="font-mono font-semibold text-sm">v{systemInfo?.proto_version}</span>
            </div>
            <div className="p-3 rounded-[var(--radius-sm)] bg-[var(--raised)] border border-[var(--line)]">
              <span className="text-[var(--ink-muted)] block text-[11px] mb-1">Host OS & Arch</span>
              <span className="font-mono font-semibold text-sm capitalize">
                {systemInfo?.os} / {systemInfo?.arch}
              </span>
            </div>
          </div>
        )}
      </div>

      {/* Quick Resource Meters (Section 8.4 Wireframe) */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* CPU */}
        <div className="p-4 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] flex flex-col justify-between">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-medium text-[var(--ink-muted)] flex items-center gap-1.5">
              <Cpu className="w-3.5 h-3.5 text-[var(--primary)]" />
              {t('dashboard.quickStats.cpu')}
            </span>
            <span className="text-xs font-mono text-[var(--ink)]">18%</span>
          </div>
          <div className="flex items-baseline justify-between mt-2">
            <span className="text-lg font-bold tabular-nums">18.4%</span>
            <PulseStrip data={[10, 15, 12, 18, 22, 17, 19, 24, 18]} width={80} height={16} />
          </div>
        </div>

        {/* Memory */}
        <div className="p-4 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] flex flex-col justify-between">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-medium text-[var(--ink-muted)] flex items-center gap-1.5">
              <Layers className="w-3.5 h-3.5 text-[var(--primary)]" />
              {t('dashboard.quickStats.memory')}
            </span>
            <span className="text-xs font-mono text-[var(--ink)]">1.2 / 4 GB</span>
          </div>
          <div className="flex items-baseline justify-between mt-2">
            <span className="text-lg font-bold tabular-nums">30%</span>
            <PulseStrip data={[28, 29, 30, 30, 31, 30, 30, 30, 30]} width={80} height={16} />
          </div>
        </div>

        {/* Disk */}
        <div className="p-4 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] flex flex-col justify-between">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-medium text-[var(--ink-muted)] flex items-center gap-1.5">
              <HardDrive className="w-3.5 h-3.5 text-[var(--primary)]" />
              {t('dashboard.quickStats.disk')}
            </span>
            <span className="text-xs font-mono text-[var(--ink)]">41 / 80 GB</span>
          </div>
          <div className="flex items-baseline justify-between mt-2">
            <span className="text-lg font-bold tabular-nums">51.2%</span>
            <PulseStrip data={[50, 50, 50, 51, 51, 51, 51, 51, 51]} width={80} height={16} />
          </div>
        </div>

        {/* Network */}
        <div className="p-4 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] flex flex-col justify-between">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs font-medium text-[var(--ink-muted)] flex items-center gap-1.5">
              <Network className="w-3.5 h-3.5 text-[var(--primary)]" />
              {t('dashboard.quickStats.network')}
            </span>
            <span className="text-xs font-mono text-[var(--ink)]">naik 2.1 Mb/s</span>
          </div>
          <div className="flex items-baseline justify-between mt-2">
            <span className="text-lg font-bold tabular-nums">2.1 Mb/s</span>
            <PulseStrip data={[5, 12, 8, 20, 15, 30, 22, 18, 25]} width={80} height={16} />
          </div>
        </div>
      </div>

      {/* Sites List (PRD Section 8.4 Wireframe) */}
      <div className="border border-[var(--line)] rounded-[var(--radius-md)] bg-[var(--surface)] overflow-hidden">
        <div className="p-4 border-b border-[var(--line)] flex items-center justify-between">
          <h2 className="text-sm font-semibold text-[var(--ink)]">
            {t('dashboard.sitesTable.title')}
          </h2>
          <span className="text-xs text-[var(--ink-muted)] font-mono">3 situs terdaftar</span>
        </div>

        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="border-b border-[var(--line)] bg-[var(--raised)] text-[var(--ink-muted)]">
                <th className="py-2.5 px-4 font-medium">{t('dashboard.sitesTable.domain')}</th>
                <th className="py-2.5 px-4 font-medium">{t('dashboard.sitesTable.runtime')}</th>
                <th className="py-2.5 px-4 font-medium">{t('dashboard.sitesTable.pulse')}</th>
                <th className="py-2.5 px-4 font-medium">{t('dashboard.sitesTable.requests')}</th>
                <th className="py-2.5 px-4 font-medium">{t('dashboard.sitesTable.latency')}</th>
                <th className="py-2.5 px-4 font-medium text-right">{t('common.actions')}</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--line)]">
              {/* Site 1 */}
              <tr className="hover:bg-[var(--raised)]/60 transition-colors">
                <td className="py-3 px-4 font-medium text-[var(--ink)]">
                  <div className="flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-[var(--success)]" />
                    <span className="font-mono">example.com</span>
                  </div>
                </td>
                <td className="py-3 px-4">
                  <span className="px-2 py-0.5 rounded border border-[var(--line)] bg-[var(--raised)] font-mono text-[11px]">
                    PHP 8.3
                  </span>
                </td>
                <td className="py-3 px-4">
                  <PulseStrip data={[12, 14, 18, 25, 22, 19, 26, 30, 24, 28, 35, 32]} />
                </td>
                <td className="py-3 px-4 tabular-nums">1.2k req/mnt</td>
                <td className="py-3 px-4 tabular-nums font-mono">180 ms</td>
                <td className="py-3 px-4 text-right">
                  <button className="text-[var(--primary)] hover:underline font-medium">
                    {t('dashboard.sitesTable.viewDetails')}
                  </button>
                </td>
              </tr>

              {/* Site 2 */}
              <tr className="hover:bg-[var(--raised)]/60 transition-colors">
                <td className="py-3 px-4 font-medium text-[var(--ink)]">
                  <div className="flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-[var(--success)]" />
                    <span className="font-mono">api.acme.dev</span>
                  </div>
                </td>
                <td className="py-3 px-4">
                  <span className="px-2 py-0.5 rounded border border-[var(--line)] bg-[var(--raised)] font-mono text-[11px]">
                    Node 22
                  </span>
                </td>
                <td className="py-3 px-4">
                  <PulseStrip data={[20, 22, 24, 21, 23, 22, 25, 26, 24, 25, 27, 26]} />
                </td>
                <td className="py-3 px-4 tabular-nums">340 req/mnt</td>
                <td className="py-3 px-4 tabular-nums font-mono">42 ms</td>
                <td className="py-3 px-4 text-right">
                  <button className="text-[var(--primary)] hover:underline font-medium">
                    {t('dashboard.sitesTable.viewDetails')}
                  </button>
                </td>
              </tr>

              {/* Site 3 - Warning */}
              <tr className="hover:bg-[var(--raised)]/60 transition-colors bg-[var(--accent)]/5">
                <td className="py-3 px-4 font-medium text-[var(--ink)]">
                  <div className="flex items-center gap-2">
                    <span className="w-2 h-2 rounded-full bg-[var(--danger)]" />
                    <span className="font-mono">blog.rafi.id</span>
                  </div>
                </td>
                <td className="py-3 px-4">
                  <span className="px-2 py-0.5 rounded border border-[var(--line)] bg-[var(--raised)] font-mono text-[11px]">
                    PHP 8.1
                  </span>
                </td>
                <td className="py-3 px-4">
                  <PulseStrip data={[5, 10, 15, 45, 80, 70, 50, 40, 30, 25]} hasError />
                </td>
                <td className="py-3 px-4 text-[var(--danger)] font-medium tabular-nums">
                  5xx naik ke 4.2%
                </td>
                <td className="py-3 px-4 tabular-nums font-mono">450 ms</td>
                <td className="py-3 px-4 text-right">
                  <button className="text-[var(--danger)] hover:underline font-medium">
                    Lihat penyebab
                  </button>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
