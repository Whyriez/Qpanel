import React, { useState, useEffect, useCallback } from 'react';
import {
  BarChart3,
  Server,
  Globe,
  RefreshCw,
} from 'lucide-react';
import { useAuth } from '../context/AuthContext';
import { fetchSites, fetchSystemAnalyticsTimeseries } from '../api/client';
import { Site, SystemTimeseriesResponse } from '../types/api';
import { SiteAnalyticsTab } from '../components/sites/SiteAnalyticsTab';
import { ServerSystemMetrics } from '../components/analytics/ServerSystemMetrics';

type AnalyticsScope = 'site' | 'server';

export const AnalyticsPage: React.FC = () => {
  const { user } = useAuth();
  const isAdmin = user?.role === 'admin';

  const [scope, setScope] = useState<AnalyticsScope>('site');
  const [sites, setSites] = useState<Site[]>([]);
  const [selectedSiteId, setSelectedSiteId] = useState<number | null>(null);
  const [isLoadingSites, setIsLoadingSites] = useState(true);

  // Server timeseries state
  const [serverMetrics, setServerMetrics] = useState<SystemTimeseriesResponse | null>(null);
  const [isLoadingServer, setIsLoadingServer] = useState(false);
  const [serverError, setServerError] = useState<string | null>(null);
  const [serverRange, setServerRange] = useState<'1h' | '6h' | '24h' | '7d'>('24h');

  // Load sites for selector
  useEffect(() => {
    let isMounted = true;
    async function loadSites() {
      setIsLoadingSites(true);
      try {
        const list = await fetchSites();
        if (!isMounted) return;
        setSites(list);
        if (list.length > 0 && selectedSiteId === null) {
          setSelectedSiteId(list[0].id);
        }
      } catch {
        // ignore error
      } finally {
        if (isMounted) setIsLoadingSites(false);
      }
    }
    loadSites();
    return () => {
      isMounted = false;
    };
  }, [selectedSiteId]);

  // Load server metrics when scope is server
  const loadServerMetrics = useCallback(async () => {
    if (!isAdmin) return;
    setIsLoadingServer(true);
    setServerError(null);

    const now = Math.floor(Date.now() / 1000);
    let from = now - 24 * 3600;
    if (serverRange === '1h') from = now - 3600;
    else if (serverRange === '6h') from = now - 6 * 3600;
    else if (serverRange === '24h') from = now - 24 * 3600;
    else if (serverRange === '7d') from = now - 7 * 24 * 3600;

    try {
      const res = await fetchSystemAnalyticsTimeseries({ from, to: now });
      setServerMetrics(res);
    } catch (err: unknown) {
      const e = err as Error;
      setServerError(e.message || 'Gagal memuat metrik sistem server');
    } finally {
      setIsLoadingServer(false);
    }
  }, [isAdmin, serverRange]);

  useEffect(() => {
    if (scope === 'server') {
      loadServerMetrics();
    }
  }, [scope, loadServerMetrics]);

  const selectedSite = sites.find((s: Site) => s.id === selectedSiteId) || sites[0];

  return (
    <div className="space-y-6">
      {/* Top Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-4 border-b border-[var(--line)]">
        <div>
          <div className="flex items-center gap-2.5">
            <h1 className="text-xl font-bold tracking-tight text-[var(--ink)] flex items-center gap-2">
              <BarChart3 className="w-5 h-5 text-[var(--primary)]" />
              Observabilitas & Analitik
            </h1>
          </div>
          <p className="text-xs text-[var(--ink-muted)] mt-1">
            Wawasan mendalam lalu lintas web, latensi respons p50/p95/p99, PHP Slowlog, dan beban server
          </p>
        </div>

        {/* Scope Switcher (Situs vs Server Host) */}
        <div className="flex items-center gap-1.5 p-1 bg-[var(--surface)] border border-[var(--line)] rounded-xl text-xs">
          <button
            type="button"
            onClick={() => setScope('site')}
            className={`px-3 py-1.5 rounded-lg font-medium transition-colors flex items-center gap-1.5 ${
              scope === 'site'
                ? 'bg-[var(--primary)] text-white shadow-xs'
                : 'text-[var(--ink-muted)] hover:text-[var(--ink)]'
            }`}
          >
            <Globe className="w-3.5 h-3.5" />
            <span>Per Situs</span>
          </button>

          {isAdmin && (
            <button
              type="button"
              onClick={() => setScope('server')}
              className={`px-3 py-1.5 rounded-lg font-medium transition-colors flex items-center gap-1.5 ${
                scope === 'server'
                  ? 'bg-[var(--primary)] text-white shadow-xs'
                  : 'text-[var(--ink-muted)] hover:text-[var(--ink)]'
              }`}
            >
              <Server className="w-3.5 h-3.5" />
              <span>Server Host (Admin)</span>
            </button>
          )}
        </div>
      </div>

      {/* Scope: Per Site */}
      {scope === 'site' && (
        <div className="space-y-6">
          {/* Site Picker Bar */}
          <div className="flex flex-wrap items-center justify-between gap-3 p-3 bg-[var(--surface)] border border-[var(--line)] rounded-xl">
            <div className="flex items-center gap-2">
              <span className="text-xs font-semibold text-[var(--ink-muted)]">Pilih Situs:</span>
              {isLoadingSites ? (
                <span className="text-xs text-[var(--ink-muted)]">Memuat daftar...</span>
              ) : sites.length === 0 ? (
                <span className="text-xs text-[var(--ink-muted)]">Belum ada situs terdaftar</span>
              ) : (
                <select
                  value={selectedSiteId || ''}
                  onChange={(e) => setSelectedSiteId(Number(e.target.value))}
                  className="px-3 py-1.5 text-xs font-mono font-semibold bg-[var(--bg)] border border-[var(--line)] rounded-lg text-[var(--ink)] focus:outline-none focus:ring-1 focus:ring-[var(--primary)]"
                >
                  {sites.map((s: Site) => (
                    <option key={s.id} value={s.id}>
                      {s.primary_domain} ({s.kind.toUpperCase()})
                    </option>
                  ))}
                </select>
              )}
            </div>

            {selectedSite && (
              <div className="text-xs text-[var(--ink-muted)] flex items-center gap-2">
                <span>Tenant #{selectedSite.user_id}</span>
                <span>•</span>
                <span className="font-mono text-[var(--ink)]">{selectedSite.docroot}</span>
              </div>
            )}
          </div>

          {/* Render SiteAnalyticsTab */}
          {selectedSite ? (
            <SiteAnalyticsTab
              siteId={selectedSite.id}
              siteDomain={selectedSite.primary_domain}
              siteRuntime={selectedSite.kind}
            />
          ) : (
            <div className="p-12 text-center text-xs text-[var(--ink-muted)] border border-[var(--line)] rounded-xl bg-[var(--surface)]">
              Pilih situs untuk melihat analitik dan metrik performa.
            </div>
          )}
        </div>
      )}

      {/* Scope: Server Host (Admin Only) */}
      {scope === 'server' && isAdmin && (
        <div className="space-y-6">
          {/* Range Picker for Server */}
          <div className="flex flex-wrap items-center justify-between gap-3 p-3 bg-[var(--surface)] border border-[var(--line)] rounded-xl">
            <div className="flex items-center gap-1.5">
              {(['1h', '6h', '24h', '7d'] as const).map((r) => (
                <button
                  key={r}
                  type="button"
                  onClick={() => setServerRange(r)}
                  className={`px-3 py-1.5 text-xs font-semibold rounded-lg transition-colors ${
                    serverRange === r
                      ? 'bg-[var(--primary)] text-white shadow-xs'
                      : 'bg-[var(--bg)] border border-[var(--line)] text-[var(--ink-muted)] hover:text-[var(--ink)]'
                  }`}
                >
                  {r === '1h' && '1 Jam Terakhir'}
                  {r === '6h' && '6 Jam Terakhir'}
                  {r === '24h' && '24 Jam Terakhir'}
                  {r === '7d' && '7 Hari Terakhir'}
                </button>
              ))}
            </div>

            <button
              type="button"
              onClick={loadServerMetrics}
              disabled={isLoadingServer}
              className="p-1.5 rounded-lg border border-[var(--line)] bg-[var(--bg)] text-[var(--ink-muted)] hover:text-[var(--ink)] transition-colors"
              title="Perbarui"
            >
              <RefreshCw className={`w-4 h-4 ${isLoadingServer ? 'animate-spin text-[var(--primary)]' : ''}`} />
            </button>
          </div>

          {/* Error */}
          {serverError && (
            <div className="p-4 bg-rose-500/10 border border-rose-500/20 rounded-xl text-xs text-rose-500">
              {serverError}
            </div>
          )}

          {/* Server Timeseries Chart */}
          {serverMetrics && (
            <ServerSystemMetrics points={serverMetrics.points} />
          )}
        </div>
      )}
    </div>
  );
};
