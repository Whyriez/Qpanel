import React, { useEffect, useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import {
  Cpu,
  RefreshCw,
  ShieldCheck,
  CheckCircle2,
  AlertCircle,
  Download,
  Trash2,
  Loader2,
  Terminal,
  ExternalLink,
} from 'lucide-react';
import { PhpRuntimeListApiResponse, PhpVersionStatusItem } from '../types/api';
import {
  fetchPhpVersions,
  setupPhpRepo,
  installPhpVersion,
  removePhpVersion,
} from '../api/client';
import { ConfirmDangerDialog } from '../components/ui/ConfirmDangerDialog';
import { toast } from 'sonner';

export const RuntimePage: React.FC = () => {
  const { t } = useTranslation();
  const [data, setData] = useState<PhpRuntimeListApiResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Action states
  const [settingUpRepo, setSettingUpRepo] = useState(false);
  const [processingVersion, setProcessingVersion] = useState<string | null>(null);
  const [versionToRemove, setVersionToRemove] = useState<string | null>(null);

  const loadData = useCallback(async (quiet = false) => {
    if (!quiet) setLoading(true);
    setError(null);
    try {
      const res = await fetchPhpVersions();
      setData(res);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Gagal memuat status runtime PHP.';
      setError(msg);
      if (!quiet) {
        toast.error('Gagal memuat status runtime PHP', { description: msg });
      }
    } finally {
      if (!quiet) setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const handleSetupRepo = async () => {
    setSettingUpRepo(true);
    try {
      const job = await setupPhpRepo();
      toast.success('Tugas konfigurasi repositori PPA telah dibuat', {
        description: `ID Tugas: ${job.id}. Pembaruan paket sedang berjalan di latar belakang.`,
      });
      setTimeout(() => loadData(true), 3000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Gagal memulai konfigurasi repositori.';
      toast.error('Gagal mengatur repositori Sury PPA', { description: msg });
    } finally {
      setSettingUpRepo(false);
    }
  };

  const handleInstall = async (version: string) => {
    setProcessingVersion(version);
    try {
      const job = await installPhpVersion(version);
      toast.success(`Tugas pemasangan PHP ${version} telah dibuat`, {
        description: `ID Tugas: ${job.id}. Paket sedang diunduh dan service OS akan di-mask.`,
      });
      setTimeout(() => loadData(true), 3000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : `Gagal memasang PHP ${version}.`;
      toast.error(`Gagal memasang PHP ${version}`, { description: msg });
    } finally {
      setProcessingVersion(null);
    }
  };

  const handleConfirmRemove = async () => {
    if (!versionToRemove) return;
    const version = versionToRemove;
    setVersionToRemove(null);
    setProcessingVersion(version);
    try {
      const job = await removePhpVersion(version);
      toast.success(`Tugas penghapusan PHP ${version} telah dibuat`, {
        description: `ID Tugas: ${job.id}. Paket sedang dihapus dari sistem.`,
      });
      setTimeout(() => loadData(true), 3000);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : `Gagal menghapus PHP ${version}.`;
      toast.error(`Gagal menghapus PHP ${version}`, { description: msg });
    } finally {
      setProcessingVersion(null);
    }
  };

  return (
    <div className="space-y-6 max-w-6xl mx-auto">
      {/* Page Header */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-[var(--ink)] flex items-center gap-2.5">
            <Cpu className="w-5 h-5 text-[var(--primary)]" />
            {t('runtime.title')}
          </h1>
          <p className="text-xs text-[var(--ink-muted)] mt-1">
            {t('runtime.subtitle')}
          </p>
        </div>
        <button
          onClick={() => loadData()}
          disabled={loading}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] hover:bg-[var(--line)] transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
          {t('common.refresh')}
        </button>
      </div>

      {/* Security Architecture Notice */}
      <div className="p-4 rounded-[var(--radius-md)] border border-emerald-500/20 bg-emerald-500/5 flex items-start gap-3">
        <ShieldCheck className="w-5 h-5 text-emerald-500 shrink-0 mt-0.5" />
        <div className="text-xs space-y-1">
          <p className="font-semibold text-[var(--ink)]">
            Isolasi FPM Multi-Tenant & Kebijakan Masking OS
          </p>
          <p className="text-[var(--ink-muted)] leading-relaxed">
            {t('runtime.maskingNote')}
          </p>
        </div>
      </div>

      {/* Sury PPA Status Card */}
      <div className="p-5 rounded-[var(--radius-lg)] border border-[var(--line)] bg-[var(--surface)] shadow-[var(--shadow-card)] space-y-4">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-4 border-b border-[var(--line)]">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <span className="text-xs font-medium text-[var(--ink-muted)]">
                {t('runtime.ppaStatus')}:
              </span>
              {data?.ppa_configured ? (
                <span className="inline-flex items-center gap-1 text-[11px] font-semibold text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 px-2 py-0.5 rounded-full">
                  <CheckCircle2 className="w-3 h-3" />
                  {t('runtime.ppaConfigured')}
                </span>
              ) : (
                <span className="inline-flex items-center gap-1 text-[11px] font-semibold text-amber-600 dark:text-amber-400 bg-amber-500/10 px-2 py-0.5 rounded-full">
                  <AlertCircle className="w-3 h-3" />
                  {t('runtime.ppaNotConfigured')}
                </span>
              )}
            </div>
            <p className="text-xs text-[var(--ink-muted)]">
              {t('runtime.setupRepoDesc')}
            </p>
          </div>

          <button
            onClick={handleSetupRepo}
            disabled={settingUpRepo || loading}
            className="inline-flex items-center gap-1.5 px-3.5 py-2 text-xs font-semibold rounded-[var(--radius-md)] bg-[var(--primary)] text-white hover:opacity-90 transition-opacity disabled:opacity-50 shrink-0"
          >
            {settingUpRepo ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <ExternalLink className="w-3.5 h-3.5" />
            )}
            {t('runtime.setupRepo')}
          </button>
        </div>
      </div>

      {/* Error Banner */}
      {error && (
        <div className="p-4 rounded-[var(--radius-md)] border border-rose-500/20 bg-rose-500/5 text-xs text-rose-600 dark:text-rose-400 flex items-center gap-2">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* PHP Versions List */}
      <div className="rounded-[var(--radius-lg)] border border-[var(--line)] bg-[var(--surface)] shadow-[var(--shadow-card)] overflow-hidden">
        <div className="px-5 py-4 border-b border-[var(--line)] flex items-center justify-between">
          <h2 className="text-sm font-semibold text-[var(--ink)]">
            Daftar Versi PHP yang Didukung
          </h2>
          <span className="text-xs text-[var(--ink-muted)]">
            {data?.versions?.filter((v) => v.installed).length || 0} dari{' '}
            {data?.versions?.length || 6} terpasang
          </span>
        </div>

        <div className="divide-y divide-[var(--line)]">
          {loading && !data ? (
            <div className="p-12 text-center text-xs text-[var(--ink-muted)] flex flex-col items-center gap-2">
              <Loader2 className="w-5 h-5 animate-spin text-[var(--primary)]" />
              <span>Memeriksa binary dan status service PHP di sistem...</span>
            </div>
          ) : (
            data?.versions.map((v: PhpVersionStatusItem) => {
              const isProcessing = processingVersion === v.version;

              return (
                <div
                  key={v.version}
                  className="p-5 flex flex-col sm:flex-row sm:items-center justify-between gap-4 hover:bg-[var(--bg)]/50 transition-colors"
                >
                  <div className="space-y-2">
                    <div className="flex items-center gap-3">
                      <div className="w-8 h-8 rounded-[var(--radius-md)] bg-[var(--primary)]/10 text-[var(--primary)] font-bold text-xs flex items-center justify-center">
                        {v.version}
                      </div>
                      <div>
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-semibold text-[var(--ink)]">
                            PHP {v.version}
                          </span>
                          {v.installed ? (
                            <span className="inline-flex items-center gap-1 text-[10px] font-semibold text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 px-2 py-0.5 rounded-full">
                              <CheckCircle2 className="w-2.5 h-2.5" />
                              {t('runtime.installed')}
                            </span>
                          ) : (
                            <span className="inline-flex items-center text-[10px] font-medium text-[var(--ink-muted)] bg-[var(--line)]/50 px-2 py-0.5 rounded-full">
                              {t('runtime.available')}
                            </span>
                          )}
                          {v.installed && v.service_masked && (
                            <span className="inline-flex items-center gap-1 text-[10px] font-semibold text-violet-600 dark:text-violet-400 bg-violet-500/10 px-2 py-0.5 rounded-full">
                              <ShieldCheck className="w-2.5 h-2.5" />
                              {t('runtime.masked')}
                            </span>
                          )}
                        </div>
                        <div className="text-[11px] text-[var(--ink-muted)] flex items-center gap-3 mt-1">
                          <span>
                            Service:{' '}
                            <code className="px-1 py-0.5 rounded bg-[var(--line)] font-mono text-[10px]">
                              {v.service_name}
                            </code>
                          </span>
                        </div>
                      </div>
                    </div>

                    {v.installed && (
                      <div className="pl-11 flex flex-wrap gap-x-4 gap-y-1 text-[11px] text-[var(--ink-muted)] font-mono">
                        <span className="flex items-center gap-1">
                          <Terminal className="w-3 h-3 text-[var(--primary)]" />
                          CLI: {v.cli_binary_path}
                        </span>
                        <span className="flex items-center gap-1">
                          <Cpu className="w-3 h-3 text-emerald-500" />
                          FPM: {v.fpm_binary_path}
                        </span>
                      </div>
                    )}
                  </div>

                  <div className="flex items-center gap-2 sm:self-center pl-11 sm:pl-0">
                    {v.installed ? (
                      <button
                        onClick={() => setVersionToRemove(v.version)}
                        disabled={isProcessing}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-[var(--radius-md)] border border-rose-500/20 text-rose-600 dark:text-rose-400 hover:bg-rose-500/10 transition-colors disabled:opacity-50"
                      >
                        {isProcessing ? (
                          <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        ) : (
                          <Trash2 className="w-3.5 h-3.5" />
                        )}
                        {t('runtime.remove')}
                      </button>
                    ) : (
                      <button
                        onClick={() => handleInstall(v.version)}
                        disabled={isProcessing}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-semibold rounded-[var(--radius-md)] bg-[var(--primary)] text-white hover:opacity-90 transition-opacity disabled:opacity-50"
                      >
                        {isProcessing ? (
                          <Loader2 className="w-3.5 h-3.5 animate-spin" />
                        ) : (
                          <Download className="w-3.5 h-3.5" />
                        )}
                        {t('runtime.install')}
                      </button>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* Confirmation Modal for Removal */}
      <ConfirmDangerDialog
        isOpen={Boolean(versionToRemove)}
        onClose={() => setVersionToRemove(null)}
        onConfirm={handleConfirmRemove}
        title={`Hapus Runtime PHP ${versionToRemove}?`}
        description={`Penghapusan akan mencopot paket PHP ${versionToRemove} dari sistem. Tindakan ini hanya dapat dilakukan jika tidak ada situs aktif yang menggunakannya.`}
        confirmMatchText={`hapus php ${versionToRemove}`}
        confirmButtonLabel="Hapus Runtime PHP"
      />
    </div>
  );
};

export default RuntimePage;
