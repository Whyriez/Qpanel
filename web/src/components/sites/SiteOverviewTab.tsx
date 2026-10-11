import React, { useState, useEffect, useCallback } from 'react';
import {
  Activity,
  AlertTriangle,
  Archive,
  ArrowRight,
  CheckCircle2,
  Cpu,
  ExternalLink,
  Flame,
  Folder,
  GitBranch,
  HardDrive,
  Layers,
  Lock,
  Pause,
  Play,
  RefreshCw,
  RotateCcw,
  Server,
  ShieldAlert,
  ShieldCheck,
  Terminal,
  Zap,
} from 'lucide-react';
import {
  Site,
  SiteOverviewResponse,
  SiteContextualWarning,
} from '../../types/api';
import { fetchSiteOverview, executeSiteQuickAction } from '../../api/client';
import { StatusPill } from '../ui/StatusPill';

export interface SiteOverviewTabProps {
  site: Site;
  onNavigateTab: (tab: string) => void;
  onOpenTerminal?: () => void;
}

export const SiteOverviewTab: React.FC<SiteOverviewTabProps> = ({
  site,
  onNavigateTab,
  onOpenTerminal,
}) => {
  const [overview, setOverview] = useState<SiteOverviewResponse | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(true);
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  const [actionMessage, setActionMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  const loadOverview = useCallback(async () => {
    try {
      setIsLoading(true);
      const data = await fetchSiteOverview(site.id);
      setOverview(data);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Gagal memuat ringkasan situs';
      setActionMessage({ type: 'error', text: msg });
    } finally {
      setIsLoading(false);
    }
  }, [site.id]);

  useEffect(() => {
    loadOverview();
  }, [loadOverview]);

  const handleQuickAction = async (action: string) => {
    try {
      setActionInProgress(action);
      setActionMessage(null);
      const res = await executeSiteQuickAction(site.id, action);
      setActionMessage({ type: 'success', text: res.message });
      // Refresh overview data to show updated runtime state
      await loadOverview();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : `Gagal mengeksekusi aksi '${action}'`;
      setActionMessage({ type: 'error', text: msg });
    } finally {
      setActionInProgress(null);
    }
  };

  const formatBytes = (bytes?: number, decimals = 1): string => {
    if (!bytes || bytes <= 0) return '0 B';
    const k = 1024;
    const dm = decimals < 0 ? 0 : decimals;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(dm)) + ' ' + sizes[i];
  };

  const formatUptime = (seconds?: number | null): string => {
    if (!seconds || seconds <= 0) return 'Baru dimulai / Tidak aktif';
    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    if (days > 0) return `${days} hari ${hours} jam`;
    if (hours > 0) return `${hours} jam ${mins} menit`;
    return `${mins} menit`;
  };

  const formatDate = (timestamp?: number | null): string => {
    if (!timestamp) return '-';
    return new Date(timestamp * 1000).toLocaleString('id-ID', {
      dateStyle: 'medium',
      timeStyle: 'short',
    });
  };

  if (isLoading && !overview) {
    return (
      <div className="flex flex-col items-center justify-center p-12 text-slate-500 space-y-3" role="status">
        <RefreshCw className="w-8 h-8 animate-spin text-blue-500" />
        <span className="text-sm font-medium">Memuat ringkasan performa & status situs...</span>
      </div>
    );
  }

  const live = overview?.status;
  const resources = overview?.resources;
  const ssl = overview?.ssl;
  const runtime = overview?.runtime;
  const storage = overview?.storage;
  const latestDeploy = overview?.latest_deployment;
  const latestBackup = overview?.latest_backup;
  const warnings = overview?.warnings || [];

  const ramUsedBytes = resources?.memory_bytes || 0;
  const ramLimitBytes = resources?.memory_limit_bytes || 1024 * 1024 * 1024;
  const ramPercentage = Math.min(100, Math.round((ramUsedBytes / ramLimitBytes) * 100));

  const pidsCurrent = resources?.pids_current || 0;
  const pidsLimit = resources?.pids_limit || 100;
  const pidsPercentage = Math.min(100, Math.round((pidsCurrent / pidsLimit) * 100));

  const diskUsedBytes = storage?.total_site_bytes || 0;
  const diskLimitBytes = storage?.plan_disk_limit_bytes || 10 * 1024 * 1024 * 1024;
  const diskPercentage = storage?.usage_percentage || Math.min(100, Math.round((diskUsedBytes / diskLimitBytes) * 100));

  return (
    <div className="space-y-6" data-testid="site-overview-tab">
      {/* Action Notification Toast/Banner */}
      {actionMessage && (
        <div
          role="alert"
          className={`p-4 rounded-xl flex items-start justify-between border ${
            actionMessage.type === 'success'
              ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-300'
              : 'bg-rose-500/10 border-rose-500/30 text-rose-300'
          }`}
        >
          <div className="flex items-center space-x-2">
            {actionMessage.type === 'success' ? (
              <CheckCircle2 className="w-5 h-5 flex-shrink-0" />
            ) : (
              <AlertTriangle className="w-5 h-5 flex-shrink-0" />
            )}
            <span className="text-sm font-medium">{actionMessage.text}</span>
          </div>
          <button
            onClick={() => setActionMessage(null)}
            className="text-xs opacity-75 hover:opacity-100 underline ml-4"
          >
            Tutup
          </button>
        </div>
      )}

      {/* Contextual Warnings List (Peringatan Kontekstual) */}
      {warnings.length > 0 && (
        <div className="space-y-3" data-testid="contextual-warnings-section">
          {warnings.map((w: SiteContextualWarning) => {
            const isDanger = w.level === 'danger';
            return (
              <div
                key={w.id}
                className={`p-4 rounded-xl border flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 ${
                  isDanger
                    ? 'bg-rose-500/10 border-rose-500/30 text-rose-200'
                    : 'bg-amber-500/10 border-amber-500/30 text-amber-200'
                }`}
              >
                <div className="flex items-start space-x-3">
                  {isDanger ? (
                    <ShieldAlert className="w-5 h-5 text-rose-400 mt-0.5 flex-shrink-0" />
                  ) : (
                    <AlertTriangle className="w-5 h-5 text-amber-400 mt-0.5 flex-shrink-0" />
                  )}
                  <div>
                    <h4 className="text-sm font-semibold">{w.title}</h4>
                    <p className="text-xs text-slate-300 mt-0.5">{w.message}</p>
                  </div>
                </div>
                {w.action_tab && (
                  <button
                    onClick={() => onNavigateTab(w.action_tab!)}
                    className={`inline-flex items-center px-3 py-1.5 rounded-lg text-xs font-semibold whitespace-nowrap transition-colors ${
                      isDanger
                        ? 'bg-rose-600 hover:bg-rose-500 text-white shadow-sm'
                        : 'bg-amber-600 hover:bg-amber-500 text-white shadow-sm'
                    }`}
                  >
                    Periksa Masalah
                    <ArrowRight className="w-3.5 h-3.5 ml-1" />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}

      {/* Top Banner: Status & Quick Actions Bar */}
      <div className="bg-slate-900/60 backdrop-blur-md border border-slate-800 rounded-2xl p-5 shadow-lg space-y-4">
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-slate-800/80">
          <div className="flex items-center space-x-3">
            <div className="relative">
              <span
                className={`flex h-4 w-4 rounded-full ${
                  live?.healthy && live?.webserver_status === 'active'
                    ? 'bg-emerald-500'
                    : 'bg-rose-500'
                }`}
              />
              {live?.healthy && live?.webserver_status === 'active' && (
                <span className="animate-ping absolute top-0 left-0 h-4 w-4 rounded-full bg-emerald-400 opacity-75" />
              )}
            </div>
            <div>
              <div className="flex items-center space-x-2">
                <h3 className="text-lg font-bold text-white tracking-tight">
                  {overview?.domain || site.primary_domain}
                </h3>
                <StatusPill
                  variant={live?.webserver_status === 'active' ? 'success' : 'neutral'}
                  label={live?.webserver_status === 'active' ? 'Aktif' : 'Dinonaktifkan'}
                />
                <StatusPill
                  variant={
                    live?.runtime_status === 'running'
                      ? 'success'
                      : live?.runtime_status === 'inactive'
                      ? 'neutral'
                      : 'danger'
                  }
                  label={`Runtime: ${live?.runtime_status || 'unknown'}`}
                />
              </div>
              <p className="text-xs text-slate-400 mt-0.5">
                Uptime: {formatUptime(live?.uptime_secs)}
                {live?.pid ? ` • PID: ${live.pid}` : ''}
              </p>
            </div>
          </div>

          <div className="flex items-center space-x-2">
            <button
              onClick={() => loadOverview()}
              disabled={isLoading}
              title="Segarkan Ringkasan"
              className="p-2 text-slate-400 hover:text-white bg-slate-800/80 hover:bg-slate-700/80 rounded-xl border border-slate-700/60 transition-colors"
            >
              <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
            </button>
            <a
              href={`https://${overview?.domain || site.primary_domain}`}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center space-x-1.5 px-3 py-2 bg-blue-600/20 hover:bg-blue-600/30 text-blue-400 border border-blue-500/30 rounded-xl text-xs font-semibold transition-colors"
            >
              <span>Kunjungi Situs</span>
              <ExternalLink className="w-3.5 h-3.5" />
            </a>
          </div>
        </div>

        {/* Quick Action Bar (Aksi Cepat) */}
        <div>
          <p className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2.5">
            Aksi Cepat Layanan & Situs
          </p>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 lg:grid-cols-9 gap-2">
            {/* Start Button */}
            <button
              onClick={() => handleQuickAction('start')}
              disabled={actionInProgress !== null || (live?.webserver_status === 'active' && live?.runtime_status === 'running')}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-emerald-500/50 transition-all disabled:opacity-40 disabled:pointer-events-none group"
            >
              <Play className="w-4 h-4 text-emerald-400 mb-1 group-hover:scale-110 transition-transform" />
              <span className="text-[11px] font-medium text-center">Mulai</span>
            </button>

            {/* Stop Button */}
            <button
              onClick={() => handleQuickAction('stop')}
              disabled={actionInProgress !== null || (live?.webserver_status === 'suspended' && live?.runtime_status === 'inactive')}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-rose-500/50 transition-all disabled:opacity-40 disabled:pointer-events-none group"
            >
              <Pause className="w-4 h-4 text-rose-400 mb-1 group-hover:scale-110 transition-transform" />
              <span className="text-[11px] font-medium text-center">Hentikan</span>
            </button>

            {/* Restart Button */}
            <button
              onClick={() => handleQuickAction('restart')}
              disabled={actionInProgress !== null}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-blue-500/50 transition-all disabled:opacity-40 disabled:pointer-events-none group"
            >
              <RotateCcw className={`w-4 h-4 text-blue-400 mb-1 group-hover:scale-110 transition-transform ${actionInProgress === 'restart' ? 'animate-spin' : ''}`} />
              <span className="text-[11px] font-medium text-center">Restart</span>
            </button>

            {/* Reload Nginx Button */}
            <button
              onClick={() => handleQuickAction('reload')}
              disabled={actionInProgress !== null}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-indigo-500/50 transition-all disabled:opacity-40 disabled:pointer-events-none group"
            >
              <RefreshCw className={`w-4 h-4 text-indigo-400 mb-1 group-hover:scale-110 transition-transform ${actionInProgress === 'reload' ? 'animate-spin' : ''}`} />
              <span className="text-[11px] font-medium text-center">Reload Nginx</span>
            </button>

            {/* Clear Cache Button */}
            <button
              onClick={() => handleQuickAction('clear_cache')}
              disabled={actionInProgress !== null}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-amber-500/50 transition-all disabled:opacity-40 disabled:pointer-events-none group"
            >
              <Flame className={`w-4 h-4 text-amber-400 mb-1 group-hover:scale-110 transition-transform ${actionInProgress === 'clear_cache' ? 'animate-pulse' : ''}`} />
              <span className="text-[11px] font-medium text-center">Hapus Cache</span>
            </button>

            {/* Open Files */}
            <button
              onClick={() => onNavigateTab('files')}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-amber-400/50 transition-all group"
            >
              <Folder className="w-4 h-4 text-amber-300 mb-1 group-hover:scale-110 transition-transform" />
              <span className="text-[11px] font-medium text-center">Buka Berkas</span>
            </button>

            {/* Open Terminal */}
            <button
              onClick={() => (onOpenTerminal ? onOpenTerminal() : onNavigateTab('settings'))}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-emerald-400/50 transition-all group"
            >
              <Terminal className="w-4 h-4 text-emerald-300 mb-1 group-hover:scale-110 transition-transform" />
              <span className="text-[11px] font-medium text-center">Terminal</span>
            </button>

            {/* Backup Now */}
            <button
              onClick={() => onNavigateTab('backups')}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-cyan-400/50 transition-all group"
            >
              <Archive className="w-4 h-4 text-cyan-300 mb-1 group-hover:scale-110 transition-transform" />
              <span className="text-[11px] font-medium text-center">Cadangkan</span>
            </button>

            {/* Quick Deploy */}
            <button
              onClick={() => onNavigateTab('deploy')}
              className="flex flex-col items-center justify-center p-2.5 rounded-xl bg-slate-800/60 hover:bg-slate-800 text-slate-200 border border-slate-700/60 hover:border-purple-400/50 transition-all group"
            >
              <Zap className="w-4 h-4 text-purple-300 mb-1 group-hover:scale-110 transition-transform" />
              <span className="text-[11px] font-medium text-center">Deploy</span>
            </button>
          </div>
        </div>
      </div>

      {/* Grid: Resources, SSL, Runtime */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {/* Resource Usage Card (Penggunaan Sumber Daya Cgroup) */}
        <div className="bg-slate-900/60 backdrop-blur-md border border-slate-800 rounded-2xl p-5 shadow-lg flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-2.5">
                <div className="p-2 bg-blue-500/10 rounded-xl text-blue-400 border border-blue-500/20">
                  <Activity className="w-5 h-5" />
                </div>
                <div>
                  <h4 className="text-sm font-bold text-white">Resource Tenant</h4>
                  <p className="text-xs text-slate-400">cgroup slice isolasi</p>
                </div>
              </div>
              <span className="text-xs px-2 py-0.5 rounded-full bg-slate-800 text-slate-300 border border-slate-700">
                Live
              </span>
            </div>

            <div className="space-y-4">
              {/* RAM */}
              <div>
                <div className="flex justify-between text-xs mb-1.5">
                  <span className="text-slate-400 flex items-center gap-1">
                    <Cpu className="w-3.5 h-3.5" /> Memori (RAM)
                  </span>
                  <span className="text-slate-200 font-semibold">
                    {formatBytes(ramUsedBytes)} / {formatBytes(ramLimitBytes)} ({ramPercentage}%)
                  </span>
                </div>
                <div className="w-full bg-slate-800 rounded-full h-2 overflow-hidden">
                  <div
                    className={`h-2 rounded-full transition-all duration-500 ${
                      ramPercentage > 85
                        ? 'bg-rose-500'
                        : ramPercentage > 70
                        ? 'bg-amber-500'
                        : 'bg-blue-500'
                    }`}
                    style={{ width: `${Math.max(4, ramPercentage)}%` }}
                  />
                </div>
              </div>

              {/* Tasks / PIDs */}
              <div>
                <div className="flex justify-between text-xs mb-1.5">
                  <span className="text-slate-400 flex items-center gap-1">
                    <Layers className="w-3.5 h-3.5" /> Proses (Tasks)
                  </span>
                  <span className="text-slate-200 font-semibold">
                    {pidsCurrent} / {pidsLimit} ({pidsPercentage}%)
                  </span>
                </div>
                <div className="w-full bg-slate-800 rounded-full h-2 overflow-hidden">
                  <div
                    className={`h-2 rounded-full transition-all duration-500 ${
                      pidsPercentage > 85
                        ? 'bg-rose-500'
                        : pidsPercentage > 70
                        ? 'bg-amber-500'
                        : 'bg-emerald-500'
                    }`}
                    style={{ width: `${Math.max(4, pidsPercentage)}%` }}
                  />
                </div>
              </div>

              {/* CPU utilization */}
              <div className="pt-1 flex items-center justify-between text-xs border-t border-slate-800/80">
                <span className="text-slate-400">Penggunaan CPU:</span>
                <span className="text-slate-200 font-semibold">
                  {resources?.cpu_percent ? `${resources.cpu_percent}%` : '0.0%'}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-3 border-t border-slate-800/80 flex items-center justify-between text-xs text-slate-400">
            <span>Kebijakan OOM: cgroup kill</span>
            <span className="text-emerald-400 font-medium">Terkontrol</span>
          </div>
        </div>

        {/* SSL Expiry Card (Kedaluwarsa SSL) */}
        <div className="bg-slate-900/60 backdrop-blur-md border border-slate-800 rounded-2xl p-5 shadow-lg flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-2.5">
                <div
                  className={`p-2 rounded-xl border ${
                    ssl?.status === 'valid'
                      ? 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20'
                      : ssl?.status === 'expiring_soon'
                      ? 'bg-amber-500/10 text-amber-400 border-amber-500/20'
                      : 'bg-rose-500/10 text-rose-400 border-rose-500/20'
                  }`}
                >
                  {ssl?.has_certificate ? (
                    <ShieldCheck className="w-5 h-5" />
                  ) : (
                    <ShieldAlert className="w-5 h-5" />
                  )}
                </div>
                <div>
                  <h4 className="text-sm font-bold text-white">Sertifikat SSL</h4>
                  <p className="text-xs text-slate-400">TLS 1.2 / 1.3 Encryption</p>
                </div>
              </div>
              <StatusPill
                variant={
                  ssl?.status === 'valid'
                    ? 'success'
                    : ssl?.status === 'expiring_soon'
                    ? 'warning'
                    : 'danger'
                }
                label={
                  ssl?.status === 'valid'
                    ? 'Aktif Valid'
                    : ssl?.status === 'expiring_soon'
                    ? 'Hampir Habis'
                    : ssl?.status === 'expired'
                    ? 'Kedaluwarsa'
                    : 'Belum Terpasang'
                }
              />
            </div>

            <div className="space-y-2.5 text-xs">
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-400">Penerbit (Issuer):</span>
                <span className="text-slate-200 font-medium">
                  {ssl?.issuer || 'Belum ada'}
                </span>
              </div>
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-400">Masa Berlaku Sampai:</span>
                <span className="text-slate-200 font-medium">
                  {formatDate(ssl?.valid_until)}
                </span>
              </div>
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-400">Hitung Mundur Sisa:</span>
                <span
                  className={`font-bold ${
                    (ssl?.days_remaining ?? 0) <= 0
                      ? 'text-rose-400'
                      : (ssl?.days_remaining ?? 0) < 14
                      ? 'text-amber-400'
                      : 'text-emerald-400'
                  }`}
                >
                  {ssl?.days_remaining !== null && ssl?.days_remaining !== undefined
                    ? `${ssl.days_remaining} hari`
                    : '-'}
                </span>
              </div>
              <div className="flex justify-between py-1">
                <span className="text-slate-400">Perpanjangan Otomatis:</span>
                <span className="text-slate-200 font-medium flex items-center gap-1">
                  {ssl?.auto_renew ? (
                    <span className="text-emerald-400 font-semibold">Aktif (ACME)</span>
                  ) : (
                    <span className="text-slate-400">Nonaktif</span>
                  )}
                </span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-3 border-t border-slate-800/80">
            <button
              onClick={() => onNavigateTab('ssl')}
              className="w-full py-2 px-3 rounded-xl bg-slate-800 hover:bg-slate-700/80 text-blue-400 text-xs font-semibold flex items-center justify-center space-x-1.5 transition-colors border border-slate-700/60"
            >
              <Lock className="w-3.5 h-3.5" />
              <span>Kelola & Terbitkan SSL</span>
              <ArrowRight className="w-3.5 h-3.5 ml-1" />
            </button>
          </div>
        </div>

        {/* Runtime Version Card (Versi Runtime) */}
        <div className="bg-slate-900/60 backdrop-blur-md border border-slate-800 rounded-2xl p-5 shadow-lg flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-2.5">
                <div className="p-2 bg-purple-500/10 rounded-xl text-purple-400 border border-purple-500/20">
                  <Server className="w-5 h-5" />
                </div>
                <div>
                  <h4 className="text-sm font-bold text-white">Versi Runtime</h4>
                  <p className="text-xs text-slate-400">Eksekusi lingkungan aplikasi</p>
                </div>
              </div>
              <span className="text-xs font-mono uppercase px-2 py-0.5 rounded-full bg-purple-500/20 text-purple-300 border border-purple-500/30">
                {runtime?.kind || site.kind}
              </span>
            </div>

            <div className="space-y-2.5 text-xs">
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-400">Versi Engine:</span>
                <span className="text-slate-200 font-semibold">
                  {runtime?.version || site.version || 'Default'}
                </span>
              </div>
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-400">Framework / Preset:</span>
                <span className="text-slate-200 font-medium capitalize">
                  {runtime?.framework || runtime?.preset || 'Generic'}
                </span>
              </div>
              <div className="flex justify-between py-1 border-b border-slate-800/60">
                <span className="text-slate-400">Socket / Port Loopback:</span>
                <span className="text-slate-200 font-mono text-[11px] truncate max-w-[150px]" title={runtime?.entrypoint_or_target}>
                  {runtime?.entrypoint_or_target || '-'}
                </span>
              </div>
              <div className="flex justify-between py-1">
                <span className="text-slate-400">FastCGI Cache:</span>
                <span className="text-slate-200 font-medium">Nginx Reverse Proxy</span>
              </div>
            </div>
          </div>

          <div className="mt-4 pt-3 border-t border-slate-800/80">
            <button
              onClick={() => {
                const targetTab =
                  runtime?.kind === 'nodejs'
                    ? 'node'
                    : runtime?.kind === 'python'
                    ? 'python'
                    : runtime?.kind === 'php'
                    ? 'php'
                    : runtime?.kind === 'proxy'
                    ? 'proxy'
                    : 'settings';
                onNavigateTab(targetTab);
              }}
              className="w-full py-2 px-3 rounded-xl bg-slate-800 hover:bg-slate-700/80 text-purple-400 text-xs font-semibold flex items-center justify-center space-x-1.5 transition-colors border border-slate-700/60"
            >
              <span>Konfigurasi Runtime</span>
              <ArrowRight className="w-3.5 h-3.5 ml-1" />
            </button>
          </div>
        </div>
      </div>

      {/* Storage Breakdown Card (Rincian Penyimpanan Disk) */}
      <div className="bg-slate-900/60 backdrop-blur-md border border-slate-800 rounded-2xl p-5 shadow-lg space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2">
          <div className="flex items-center space-x-2.5">
            <div className="p-2 bg-amber-500/10 rounded-xl text-amber-400 border border-amber-500/20">
              <HardDrive className="w-5 h-5" />
            </div>
            <div>
              <h4 className="text-sm font-bold text-white">Rincian Penyimpanan Disk</h4>
              <p className="text-xs text-slate-400">
                Total terpakai: <span className="font-semibold text-slate-200">{formatBytes(diskUsedBytes)}</span> dari batas paket <span className="font-semibold text-slate-200">{formatBytes(diskLimitBytes)}</span> ({diskPercentage}%)
              </p>
            </div>
          </div>
          <button
            onClick={() => onNavigateTab('files')}
            className="text-xs font-semibold text-blue-400 hover:text-blue-300 flex items-center space-x-1"
          >
            <span>Buka File Manager</span>
            <ArrowRight className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Segmented Bar */}
        <div className="w-full bg-slate-800 rounded-full h-3 overflow-hidden flex">
          <div
            title={`Berkas Situs: ${formatBytes(storage?.docroot_bytes)}`}
            className="bg-blue-500 h-full transition-all duration-500"
            style={{
              width: `${Math.max(
                2,
                ((storage?.docroot_bytes || 0) / (diskLimitBytes || 1)) * 100
              )}%`,
            }}
          />
          <div
            title={`Database: ${formatBytes(storage?.databases_bytes)}`}
            className="bg-emerald-500 h-full transition-all duration-500"
            style={{
              width: `${Math.max(
                1,
                ((storage?.databases_bytes || 0) / (diskLimitBytes || 1)) * 100
              )}%`,
            }}
          />
          <div
            title={`Arsip Cadangan: ${formatBytes(storage?.backups_bytes)}`}
            className="bg-purple-500 h-full transition-all duration-500"
            style={{
              width: `${Math.max(
                1,
                ((storage?.backups_bytes || 0) / (diskLimitBytes || 1)) * 100
              )}%`,
            }}
          />
        </div>

        {/* Legend */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-4 text-xs pt-1">
          <div className="flex items-center space-x-2">
            <span className="w-3 h-3 rounded-full bg-blue-500 flex-shrink-0" />
            <div>
              <p className="text-slate-400">Berkas Dokumen</p>
              <p className="font-semibold text-slate-200">{formatBytes(storage?.docroot_bytes)}</p>
            </div>
          </div>
          <div className="flex items-center space-x-2">
            <span className="w-3 h-3 rounded-full bg-emerald-500 flex-shrink-0" />
            <div>
              <p className="text-slate-400">Database</p>
              <p className="font-semibold text-slate-200">{formatBytes(storage?.databases_bytes)}</p>
            </div>
          </div>
          <div className="flex items-center space-x-2">
            <span className="w-3 h-3 rounded-full bg-purple-500 flex-shrink-0" />
            <div>
              <p className="text-slate-400">Arsip Cadangan</p>
              <p className="font-semibold text-slate-200">{formatBytes(storage?.backups_bytes)}</p>
            </div>
          </div>
          <div className="flex items-center space-x-2">
            <span className="w-3 h-3 rounded-full bg-slate-700 flex-shrink-0" />
            <div>
              <p className="text-slate-400">Sisa Kuota Bebas</p>
              <p className="font-semibold text-slate-200">
                {formatBytes(Math.max(0, diskLimitBytes - diskUsedBytes))}
              </p>
            </div>
          </div>
        </div>
      </div>

      {/* Grid: Latest Deployment & Latest Backup */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Latest Deployment (Deploy Terakhir) */}
        <div className="bg-slate-900/60 backdrop-blur-md border border-slate-800 rounded-2xl p-5 shadow-lg flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-2.5">
                <div className="p-2 bg-purple-500/10 rounded-xl text-purple-400 border border-purple-500/20">
                  <GitBranch className="w-5 h-5" />
                </div>
                <div>
                  <h4 className="text-sm font-bold text-white">Deployment Terakhir</h4>
                  <p className="text-xs text-slate-400">Status rilis Git & webhook</p>
                </div>
              </div>
              {latestDeploy && (
                <StatusPill
                  variant={
                    latestDeploy.status === 'success'
                      ? 'success'
                      : latestDeploy.status === 'running'
                      ? 'warning'
                      : 'danger'
                  }
                  label={latestDeploy.status}
                />
              )}
            </div>

            {latestDeploy ? (
              <div className="space-y-2.5 text-xs">
                <div className="flex justify-between py-1 border-b border-slate-800/60">
                  <span className="text-slate-400">ID Rilis:</span>
                  <span className="text-slate-200 font-mono font-medium">{latestDeploy.release_id}</span>
                </div>
                <div className="flex justify-between py-1 border-b border-slate-800/60">
                  <span className="text-slate-400">Commit:</span>
                  <div className="text-right">
                    <span className="font-mono text-purple-400 bg-purple-500/10 px-1.5 py-0.5 rounded border border-purple-500/20">
                      {latestDeploy.commit_hash.substring(0, 7) || 'HEAD'}
                    </span>
                    <span className="text-slate-300 ml-1.5">[{latestDeploy.branch}]</span>
                  </div>
                </div>
                <div className="flex justify-between py-1 border-b border-slate-800/60">
                  <span className="text-slate-400">Pesan:</span>
                  <span className="text-slate-200 truncate max-w-[200px]" title={latestDeploy.commit_message}>
                    {latestDeploy.commit_message || '-'}
                  </span>
                </div>
                <div className="flex justify-between py-1">
                  <span className="text-slate-400">Waktu & Durasi:</span>
                  <span className="text-slate-200">
                    {formatDate(latestDeploy.created_at)}
                    {latestDeploy.duration_ms ? ` (${(latestDeploy.duration_ms / 1000).toFixed(1)}s)` : ''}
                  </span>
                </div>
              </div>
            ) : (
              <div className="py-6 text-center text-slate-500 space-y-2">
                <p className="text-xs">Belum ada riwayat deployment Git terkonfigurasi.</p>
              </div>
            )}
          </div>

          <div className="mt-4 pt-3 border-t border-slate-800/80 flex items-center justify-between">
            <button
              onClick={() => onNavigateTab('deploy')}
              className="text-xs font-semibold text-purple-400 hover:text-purple-300 flex items-center space-x-1"
            >
              <span>Kelola Repositori & Rilis</span>
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
            {latestDeploy?.can_rollback && (
              <span className="text-[11px] font-medium text-emerald-400 bg-emerald-500/10 px-2 py-0.5 rounded-full border border-emerald-500/20">
                Rollback Siap
              </span>
            )}
          </div>
        </div>

        {/* Latest Backup (Cadangan Terakhir) */}
        <div className="bg-slate-900/60 backdrop-blur-md border border-slate-800 rounded-2xl p-5 shadow-lg flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-2.5">
                <div className="p-2 bg-cyan-500/10 rounded-xl text-cyan-400 border border-cyan-500/20">
                  <Archive className="w-5 h-5" />
                </div>
                <div>
                  <h4 className="text-sm font-bold text-white">Cadangan Terakhir</h4>
                  <p className="text-xs text-slate-400">Arsip docroot & database</p>
                </div>
              </div>
              {latestBackup && (
                <StatusPill
                  variant={
                    latestBackup.status === 'completed' || latestBackup.status === 'success'
                      ? 'success'
                      : 'danger'
                  }
                  label={latestBackup.status}
                />
              )}
            </div>

            {latestBackup ? (
              <div className="space-y-2.5 text-xs">
                <div className="flex justify-between py-1 border-b border-slate-800/60">
                  <span className="text-slate-400">Berkas Arsip:</span>
                  <span className="text-slate-200 font-mono text-[11px] truncate max-w-[200px]" title={latestBackup.filename}>
                    {latestBackup.filename}
                  </span>
                </div>
                <div className="flex justify-between py-1 border-b border-slate-800/60">
                  <span className="text-slate-400">Ukuran Arsip:</span>
                  <span className="text-slate-200 font-semibold">{formatBytes(latestBackup.size_bytes)}</span>
                </div>
                <div className="flex justify-between py-1 border-b border-slate-800/60">
                  <span className="text-slate-400">Cakupan Data:</span>
                  <span className="text-slate-200">
                    {latestBackup.includes_files && 'Berkas'}
                    {latestBackup.includes_files && latestBackup.includes_databases && ' + '}
                    {latestBackup.includes_databases && 'Database'}
                  </span>
                </div>
                <div className="flex justify-between py-1">
                  <span className="text-slate-400">Dibuat Pada:</span>
                  <span className="text-slate-200">{formatDate(latestBackup.created_at)}</span>
                </div>
              </div>
            ) : (
              <div className="py-6 text-center text-slate-500 space-y-2">
                <p className="text-xs">Belum ada cadangan yang dibuat untuk situs ini.</p>
              </div>
            )}
          </div>

          <div className="mt-4 pt-3 border-t border-slate-800/80">
            <button
              onClick={() => onNavigateTab('backups')}
              className="text-xs font-semibold text-cyan-400 hover:text-cyan-300 flex items-center space-x-1"
            >
              <span>Buka Menu Cadangan Lengkap</span>
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
