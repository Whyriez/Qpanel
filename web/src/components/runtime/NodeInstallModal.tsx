import { ModalOverlay } from '../ui/ModalOverlay';
import React, { useEffect, useRef, useState } from 'react';
import { Download, Loader2, X, Settings2, ShieldCheck, Check } from 'lucide-react';
import { NodeVersionItem, NodeRuntimeInstallRequest } from '../../types/api';

interface NodeInstallModalProps {
  isOpen: boolean;
  onClose: () => void;
  availableVersions: NodeVersionItem[];
  onInstall: (payload: NodeRuntimeInstallRequest) => Promise<void>;
  preselectedVersion?: string;
  busyVersions?: string[];
  jobsUnavailable?: boolean;
}

export const NodeInstallModal: React.FC<NodeInstallModalProps> = ({
  isOpen,
  onClose,
  availableVersions,
  onInstall,
  preselectedVersion,
  busyVersions = [],
  jobsUnavailable = false,
}) => {
  const [selectedVersion, setSelectedVersion] = useState<string>(
    preselectedVersion || availableVersions.find((v) => !v.is_installed)?.version || ''
  );
  const [customVersion, setCustomVersion] = useState('');
  const [isCustom, setIsCustom] = useState(false);
  const [enableCorepack, setEnableCorepack] = useState(true);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [mirrorUrl, setMirrorUrl] = useState('');
  const [offlinePath, setOfflinePath] = useState('');
  const [offlineChecksum, setOfflineChecksum] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState('');
  const closeButton = useRef<HTMLButtonElement>(null);
  const openCycle = useRef(0);

  useEffect(() => { openCycle.current++; }, [isOpen]);
  const version = (isCustom ? customVersion.trim() : selectedVersion).replace(/^v/, '');
  const versionBusy = busyVersions.includes(version);
  const versionInstalled = availableVersions.some(item => item.version === version && item.is_installed);

  useEffect(() => {
    if (isOpen) {
      setSelectedVersion(current => preselectedVersion ||
        (availableVersions.some(v => v.version === current && !v.is_installed) ? current : availableVersions.find(v => !v.is_installed)?.version || ''));
      setSubmitError('');
    }
  }, [isOpen, preselectedVersion, availableVersions]);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (submitting || versionBusy || versionInstalled || jobsUnavailable) return;
    const cycle = openCycle.current;
    const ver = isCustom ? customVersion.trim() : selectedVersion;
    if (!ver) return;

    setSubmitting(true);
    setSubmitError('');
    closeButton.current?.focus();
    try {
      await onInstall({
        version: ver,
        mirror_url: mirrorUrl.trim() || undefined,
        offline_tarball_path: offlinePath.trim() || undefined,
        offline_checksum: offlineChecksum.trim() || undefined,
        enable_corepack: enableCorepack,
      });
      if (openCycle.current === cycle) onClose();
    } catch (error: unknown) {
      setSubmitError(error instanceof Error ? error.message : 'Gagal membuat tugas pemasangan Node.js.');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <ModalOverlay
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/50 backdrop-blur-sm animate-in fade-in duration-150"
      role="dialog"
      aria-modal="true"
      aria-labelledby="node-install-title"
      onDismiss={onClose}
    >
      <div className="w-full max-w-lg bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-lg)] shadow-2xl overflow-hidden flex flex-col max-h-[90vh] dialog-panel">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--line)] dialog-header">
          <div className="flex items-center gap-2.5">
            <div className="w-8 h-8 rounded-[var(--radius-md)] bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 flex items-center justify-center">
              <Download className="w-4 h-4" />
            </div>
            <div>
              <h2 id="node-install-title" className="text-sm font-semibold text-[var(--ink)]">
                Pasang Versi Node.js
              </h2>
              <p className="text-[11px] text-[var(--ink-muted)]">
                Pemasangan berjalan di latar belakang dan dapat dipantau di Tugas.
              </p>
            </div>
          </div>
          <button
            ref={closeButton}
            onClick={onClose}
            className="p-1.5 rounded-[var(--radius-md)] text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--line)] transition-colors"
            aria-label="Tutup"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Form Body */}
        <form onSubmit={handleSubmit} className="p-5 overflow-y-auto dialog-form">
          <div className="dialog-body space-y-4">
            {submitError && <p role="alert" className="text-xs text-[var(--danger)] whitespace-pre-wrap break-words">{submitError}</p>}
            {versionBusy && <p role="status" className="text-xs text-[var(--ink-muted)]">Versi ini sedang dipasang. Pantau progres pada halaman Tugas; modal boleh ditutup.</p>}
            {submitting && <p role="status" className="text-xs text-[var(--ink-muted)]">Membuat tugas pemasangan. Modal boleh ditutup; proses tetap dilanjutkan.</p>}
            {/* Version Selection */}
            <div className="space-y-2">
              <label className="block text-xs font-medium text-[var(--ink)]">Versi Node.js</label>

              {!isCustom ? (
                <div className="space-y-2">
                  <select
                    value={selectedVersion}
                    onChange={(e) => setSelectedVersion(e.target.value)}
                    disabled={submitting}
                    className="w-full px-3 py-2 text-xs rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] focus:outline-none focus:ring-1 focus:ring-[var(--primary)]"
                  >
                    {availableVersions.map((v) => {
                      const label = `v${v.version}${v.is_lts ? ` (LTS ${v.lts_codename || ''})` : ''}${v.is_installed ? ' — Sudah Terpasang' : ''}`;
                      return (
                        <option key={v.version} value={v.version} disabled={v.is_installed || busyVersions.includes(v.version)}>
                          {label}
                        </option>
                      );
                    })}
                  </select>

                  <button
                    type="button"
                    onClick={() => setIsCustom(true)}
                    className="text-[11px] text-[var(--primary)] hover:underline"
                  >
                    Gunakan nomor versi kustom lainnya
                  </button>
                </div>
              ) : (
                <div className="space-y-2">
                  <input
                    type="text"
                    placeholder="Contoh: 22.14.0 atau 20.18.3"
                    value={customVersion}
                    onChange={(e) => setCustomVersion(e.target.value)}
                    disabled={submitting}
                    className="w-full px-3 py-2 text-xs font-mono rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] focus:outline-none focus:ring-1 focus:ring-[var(--primary)]"
                    required
                  />
                  <button
                    type="button"
                    onClick={() => setIsCustom(false)}
                    className="text-[11px] text-[var(--primary)] hover:underline"
                  >
                    Pilih dari daftar rilis katalog
                  </button>
                </div>
              )}
            </div>

            {/* Corepack Option */}
            <div className="p-3 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--bg)]/40 space-y-1.5">
              <label className="flex items-start gap-2.5 cursor-pointer">
                <input
                  type="checkbox"
                  checked={enableCorepack}
                  onChange={(e) => setEnableCorepack(e.target.checked)}
                  disabled={submitting}
                  className="mt-0.5 rounded border-[var(--line)] text-[var(--primary)] focus:ring-[var(--primary)]"
                />
                <div className="text-xs">
                  <span className="font-medium text-[var(--ink)]">Aktifkan Corepack (pnpm & yarn shims)</span>
                  <p className="text-[11px] text-[var(--ink-muted)] mt-0.5 leading-relaxed">
                    Menyediakan binary wrapper otomatis untuk package manager modern pnpm dan yarn di dalam
                    direktori bin runtime.
                  </p>
                </div>
              </label>
            </div>

            {/* Advanced Mirror / Offline Options */}
            <div className="border border-[var(--line)] rounded-[var(--radius-md)] overflow-hidden">
              <button
                type="button"
                onClick={() => setShowAdvanced(!showAdvanced)}
                className="w-full px-3.5 py-2.5 text-xs font-medium text-[var(--ink)] bg-[var(--surface)] hover:bg-[var(--line)]/40 flex items-center justify-between transition-colors"
              >
                <span className="flex items-center gap-2">
                  <Settings2 className="w-3.5 h-3.5 text-[var(--ink-muted)]" />
                  Pengaturan Lanjutan & Mirror
                </span>
                <span className="text-[10px] text-[var(--ink-muted)]">{showAdvanced ? 'Tutup' : 'Buka'}</span>
              </button>

              {showAdvanced && (
                <div className="p-3.5 space-y-3 bg-[var(--bg)]/50 border-t border-[var(--line)] text-xs">
                  <div>
                    <label className="block text-[11px] font-medium text-[var(--ink)] mb-1">
                      URL Mirror Node.js
                    </label>
                    <input
                      type="url"
                      placeholder="https://nodejs.org/dist"
                      value={mirrorUrl}
                      onChange={(e) => setMirrorUrl(e.target.value)}
                      disabled={submitting}
                      className="w-full px-3 py-1.5 text-xs font-mono rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] focus:outline-none focus:ring-1 focus:ring-[var(--primary)]"
                    />
                    <p className="text-[10px] text-[var(--ink-muted)] mt-1">
                      Biarkan kosong untuk menggunakan mirror default resmi (nodejs.org).
                    </p>
                  </div>

                  <div>
                    <label className="block text-[11px] font-medium text-[var(--ink)] mb-1">
                      Path Arsip Tarball Offline (Opsional)
                    </label>
                    <input
                      type="text"
                      placeholder="/tmp/node-v22.14.0-linux-x64.tar.gz"
                      value={offlinePath}
                      onChange={(e) => setOfflinePath(e.target.value)}
                      disabled={submitting}
                      className="w-full px-3 py-1.5 text-xs font-mono rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] focus:outline-none focus:ring-1 focus:ring-[var(--primary)]"
                    />
                  </div>

                  <div>
                    <label className="block text-[11px] font-medium text-[var(--ink)] mb-1">
                      Checksum SHA256 Arsip Offline (Opsional)
                    </label>
                    <input
                      type="text"
                      placeholder="64 karakter hash hex SHA256"
                      value={offlineChecksum}
                      onChange={(e) => setOfflineChecksum(e.target.value)}
                      disabled={submitting}
                      className="w-full px-3 py-1.5 text-xs font-mono rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface)] text-[var(--ink)] focus:outline-none focus:ring-1 focus:ring-[var(--primary)]"
                    />
                  </div>
                </div>
              )}
            </div>

            {/* Security Note */}
            <div className="flex items-center gap-2 text-[11px] text-[var(--ink-muted)]">
              <ShieldCheck className="w-3.5 h-3.5 text-emerald-500 shrink-0" />
              <span>Integritas berkas divalidasi terhadap manifest SHASUMS256 resmi sebelum diekstrak.</span>
            </div>

            {/* Footer Actions */}
          </div>
          <div className="flex items-center justify-end gap-2.5 pt-3 border-t border-[var(--line)] dialog-footer">
            <button
              type="button"
              onClick={onClose}
              className="px-3.5 py-1.5 text-xs font-medium rounded-[var(--radius-md)] border border-[var(--line)] text-[var(--ink)] hover:bg-[var(--line)]/50 transition-colors disabled:opacity-50"
            >
              Tutup
            </button>
            <button
              type="submit"
              disabled={submitting || versionBusy || versionInstalled || jobsUnavailable || !version}
              className="inline-flex items-center gap-1.5 px-4 py-1.5 text-xs font-semibold rounded-[var(--radius-md)] bg-emerald-600 text-white hover:bg-emerald-700 transition-colors disabled:opacity-50 shadow-sm"
            >
              {submitting ? (
                <>
                  <Loader2 className="w-3.5 h-3.5 animate-spin" />
                  <span>Membuat tugas...</span>
                </>
              ) : (
                <>
                  <Check className="w-3.5 h-3.5" />
                  <span>Mulai Pemasangan</span>
                </>
              )}
            </button>
          </div>
        </form>
      </div>
    </ModalOverlay>
  );
};
