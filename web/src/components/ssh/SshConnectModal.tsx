import React, { useState, useEffect, useMemo } from 'react';
import {
  Terminal,
  Key,
  Copy,
  Check,
  Shield,
  Folder,
  X,
  AlertTriangle,
  Lock,
  FileText,
  Radio,
  RefreshCw,
  Sliders,
} from 'lucide-react';
import { SshConnectionInfoResponse, SshConnectionInfoQuery } from '../../types/api';
import { fetchSshConnectionInfo } from '../../api/client';

export interface SshConnectModalProps {
  isOpen: boolean;
  onClose: () => void;
  path?: string;
  siteId?: number;
  siteDomain?: string;
  tenantUsername?: string;
  onOpenSshKeys?: () => void;
}

type TabType = 'ssh' | 'sftp' | 'config';

export const SshConnectModal: React.FC<SshConnectModalProps> = ({
  isOpen,
  onClose,
  path = '',
  siteId,
  siteDomain,
  tenantUsername,
  onOpenSshKeys,
}) => {
  const [activeTab, setActiveTab] = useState<TabType>('ssh');
  const [info, setInfo] = useState<SshConnectionInfoResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Custom connection settings overrides
  const [customHost, setCustomHost] = useState('');
  const [customPort, setCustomPort] = useState<number>(22);
  const [showAdvancedSettings, setShowAdvancedSettings] = useState(false);

  // Copied tracking state for buttons
  const [copiedKey, setCopiedKey] = useState<string | null>(null);

  const copyToClipboard = async (text: string, key: string) => {
    try {
      if (navigator?.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
      } else {
        const textarea = document.createElement('textarea');
        textarea.value = text;
        document.body.appendChild(textarea);
        textarea.select();
        document.execCommand('copy');
        document.body.removeChild(textarea);
      }
      setCopiedKey(key);
      setTimeout(() => {
        setCopiedKey((curr) => (curr === key ? null : curr));
      }, 2000);
    } catch {
      // Ignore copy error
    }
  };

  // Fetch SSH connection details from backend
  useEffect(() => {
    if (!isOpen) return;

    let isMounted = true;
    setIsLoading(true);
    setErrorMessage(null);

    const query: SshConnectionInfoQuery = {
      path: path || undefined,
      site_id: siteId,
      tenant: tenantUsername,
    };

    fetchSshConnectionInfo(query)
      .then((res) => {
        if (!isMounted) return;
        setInfo(res);
        setCustomHost(res.host);
        setCustomPort(res.port);
        // If policy is sftp, automatically switch default tab to sftp
        if (res.shell_policy === 'sftp') {
          setActiveTab('sftp');
        }
      })
      .catch((err: Error) => {
        if (!isMounted) return;
        setErrorMessage(err.message || 'Gagal memuat detail koneksi SSH');
        // Fallback local calculations
        const uname = tenantUsername || 'p_user';
        const defaultHost = siteDomain || (typeof window !== 'undefined' ? window.location.hostname : '127.0.0.1');
        const cleanPath = path ? (path.startsWith('/') ? `/home/${uname}${path}` : `/home/${uname}/${path}`) : `/home/${uname}`;
        setCustomHost(defaultHost);
        setCustomPort(22);
        setInfo({
          host: defaultHost,
          port: 22,
          username: uname,
          user_id: 0,
          home_dir: `/home/${uname}`,
          active_path: cleanPath,
          relative_path: path,
          shell_policy: 'full',
          can_shell: true,
          can_sftp: true,
          has_ssh_keys: false,
          ssh_keys_count: 0,
          ssh_command: `ssh -t ${uname}@${defaultHost} "cd '${cleanPath.replace(/'/g, "'\\''")}' && exec \\$SHELL -l"`,
          sftp_command: `sftp ${uname}@${defaultHost}:'${cleanPath.replace(/'/g, "'\\''")}'`,
          sftp_url: `sftp://${uname}@${defaultHost}${cleanPath}`,
          ssh_config_snippet: `Host ${siteDomain || `${uname}-vps`}\n    HostName ${defaultHost}\n    User ${uname}\n    IdentityFile ~/.ssh/id_ed25519\n`,
          site_domain: siteDomain || null,
        });
      })
      .finally(() => {
        if (isMounted) setIsLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, [isOpen, path, siteId, siteDomain, tenantUsername]);

  // Escape helper for client-side single quotes
  const escapePath = (p: string) => p.replace(/'/g, "'\\''");

  // Dynamically recomputed commands if host/port overridden
  const effectiveHost = customHost.trim() || info?.host || '127.0.0.1';
  const effectivePort = customPort || 22;
  const effectiveUser = info?.username || tenantUsername || 'p_user';
  const effectivePath = info?.active_path || `/home/${effectiveUser}`;

  const portFlagSsh = effectivePort === 22 ? '' : `-p ${effectivePort} `;
  const portFlagSftp = effectivePort === 22 ? '' : `-P ${effectivePort} `;

  const computedSshCommand = useMemo(() => {
    if (info?.shell_policy === 'sftp' || info?.shell_policy === 'none') return null;
    return `ssh ${portFlagSsh}-t ${effectiveUser}@${effectiveHost} "cd '${escapePath(effectivePath)}' && exec \\$SHELL -l"`;
  }, [info?.shell_policy, portFlagSsh, effectiveUser, effectiveHost, effectivePath]);

  const computedSshSimpleCommand = useMemo(() => {
    if (info?.shell_policy === 'sftp' || info?.shell_policy === 'none') return null;
    return `ssh ${portFlagSsh}${effectiveUser}@${effectiveHost}`;
  }, [info?.shell_policy, portFlagSsh, effectiveUser, effectiveHost]);

  const computedSftpCommand = useMemo(() => {
    if (info?.shell_policy === 'none') return null;
    return `sftp ${portFlagSftp}${effectiveUser}@${effectiveHost}:'${escapePath(effectivePath)}'`;
  }, [info?.shell_policy, portFlagSftp, effectiveUser, effectiveHost, effectivePath]);

  const computedSftpUrl = useMemo(() => {
    if (info?.shell_policy === 'none') return null;
    const portPart = effectivePort === 22 ? '' : `:${effectivePort}`;
    return `sftp://${effectiveUser}@${effectiveHost}${portPart}${effectivePath.startsWith('/') ? effectivePath : `/${effectivePath}`}`;
  }, [info?.shell_policy, effectivePort, effectiveUser, effectiveHost, effectivePath]);

  const computedConfigSnippet = useMemo(() => {
    const alias = siteDomain || `${effectiveUser}-server`;
    const portLine = effectivePort === 22 ? '' : `    Port ${effectivePort}\n`;
    return `Host ${alias}\n    HostName ${effectiveHost}\n    User ${effectiveUser}\n${portLine}    IdentityFile ~/.ssh/id_ed25519\n`;
  }, [siteDomain, effectiveUser, effectiveHost, effectivePort]);

  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-xs animate-in fade-in duration-150"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="ssh-connect-title"
    >
      <div
        className="bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-lg)] shadow-[var(--shadow-floating)] w-full max-w-2xl overflow-hidden flex flex-col max-h-[90vh]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--line)] bg-[var(--raised)]">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-[var(--radius-md)] bg-[var(--primary)]/10 text-[var(--primary)] flex items-center justify-center">
              <Terminal className="w-5 h-5" />
            </div>
            <div>
              <h2 id="ssh-connect-title" className="text-base font-bold text-[var(--ink)] flex items-center gap-2">
                Koneksi SSH & SFTP
              </h2>
              <p className="text-xs text-[var(--ink-muted)] flex items-center gap-1.5 mt-0.5">
                <Folder className="w-3.5 h-3.5" />
                <span className="font-mono text-[11px] truncate max-w-sm" title={effectivePath}>
                  {effectivePath}
                </span>
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {/* Shell Policy Badge */}
            {info && (
              <span
                className={`text-[11px] font-medium px-2.5 py-1 rounded-full border ${
                  info.shell_policy === 'full'
                    ? 'bg-[var(--success)]/10 border-[var(--success)]/20 text-[var(--success)]'
                    : info.shell_policy === 'sftp'
                    ? 'bg-[var(--accent)]/10 border-[var(--accent)]/20 text-[var(--accent)]'
                    : 'bg-[var(--danger)]/10 border-[var(--danger)]/20 text-[var(--danger)]'
                }`}
              >
                {info.shell_policy === 'full'
                  ? 'Shell Penuh'
                  : info.shell_policy === 'sftp'
                  ? 'Hanya SFTP'
                  : 'Shell Nonaktif'}
              </span>
            )}

            <button
              onClick={onClose}
              className="p-1.5 rounded-[var(--radius-md)] text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--surface)] transition-colors"
              title="Tutup dialog"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </div>

        {/* Modal Body */}
        <div className="p-6 overflow-y-auto space-y-5 text-xs text-[var(--ink)]">
          {isLoading && !info && (
            <div className="py-8 flex flex-col items-center justify-center gap-2 text-[var(--ink-muted)]">
              <RefreshCw className="w-5 h-5 animate-spin text-[var(--primary)]" />
              <span className="text-xs">Memuat informasi koneksi...</span>
            </div>
          )}

          {/* Policy Notification Banners */}
          {info?.shell_policy === 'sftp' && (
            <div className="p-3.5 rounded-[var(--radius-md)] bg-[var(--accent)]/10 border border-[var(--accent)]/30 text-[var(--ink)] flex items-start gap-3">
              <AlertTriangle className="w-4 h-4 text-[var(--accent)] shrink-0 mt-0.5" />
              <div>
                <p className="font-semibold text-[var(--accent)]">
                  Kebijakan Shell SFTP-Only Aktif
                </p>
                <p className="text-[11px] text-[var(--ink-muted)] mt-0.5">
                  Akun Anda dibatasi pada transfer berkas SFTP (chroot). Akses terminal interaktif dinonaktifkan oleh paket atau administrator. Gunakan perintah SFTP atau aplikasi seperti FileZilla di bawah.
                </p>
              </div>
            </div>
          )}

          {info?.shell_policy === 'none' && (
            <div className="p-3.5 rounded-[var(--radius-md)] bg-[var(--danger)]/10 border border-[var(--danger)]/30 text-[var(--ink)] flex items-start gap-3">
              <Lock className="w-4 h-4 text-[var(--danger)] shrink-0 mt-0.5" />
              <div>
                <p className="font-semibold text-[var(--danger)]">
                  Akses Shell & SFTP Dinonaktifkan
                </p>
                <p className="text-[11px] text-[var(--ink-muted)] mt-0.5">
                  Akun ini tidak memiliki izin untuk melakukan koneksi SSH maupun SFTP. Hubungi administrator bila memerlukan akses jarak jauh.
                </p>
              </div>
            </div>
          )}

          {errorMessage && (
            <div className="p-3 rounded-[var(--radius-md)] bg-[var(--danger)]/10 border border-[var(--danger)]/20 text-[var(--danger)]">
              {errorMessage}
            </div>
          )}

          {/* Quick Copy Primary Action */}
          {computedSshCommand && (
            <div className="p-4 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-[var(--ink)] flex items-center gap-1.5">
                  <Terminal className="w-3.5 h-3.5 text-[var(--primary)]" />
                  Salin Perintah SSH (Buka di Folder Aktif)
                </span>
                <span className="text-[11px] text-[var(--ink-muted)]">
                  Alokasi PTY (-t) &bull; Login Shell (-l)
                </span>
              </div>
              <div className="flex items-center gap-2 bg-[#09191F] text-[#E4EEF0] p-2.5 rounded-[var(--radius-sm)] border border-[#1F3A45]">
                <code className="font-mono text-[11px] flex-1 overflow-x-auto whitespace-nowrap scrollbar-none select-all text-emerald-400">
                  {computedSshCommand}
                </code>
                <button
                  type="button"
                  onClick={() => copyToClipboard(computedSshCommand, 'ssh_cwd')}
                  className="px-2.5 py-1 rounded-[var(--radius-sm)] bg-[var(--primary)] hover:bg-[var(--primary-hover)] text-white text-[11px] font-semibold flex items-center gap-1 shrink-0 transition-colors cursor-pointer"
                  title="Salin perintah SSH"
                >
                  {copiedKey === 'ssh_cwd' ? (
                    <>
                      <Check className="w-3.5 h-3.5" />
                      <span>Tersalin!</span>
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" />
                      <span>Salin</span>
                    </>
                  )}
                </button>
              </div>
            </div>
          )}

          {/* Quick Copy SFTP Action if SFTP enabled */}
          {computedSftpCommand && (
            <div className="p-4 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] space-y-2">
              <div className="flex items-center justify-between">
                <span className="font-semibold text-[var(--ink)] flex items-center gap-1.5">
                  <Folder className="w-3.5 h-3.5 text-blue-500" />
                  Salin Perintah SFTP CLI
                </span>
                <span className="text-[11px] text-[var(--ink-muted)]">
                  Transfer Berkas Aman
                </span>
              </div>
              <div className="flex items-center gap-2 bg-[#09191F] text-[#E4EEF0] p-2.5 rounded-[var(--radius-sm)] border border-[#1F3A45]">
                <code className="font-mono text-[11px] flex-1 overflow-x-auto whitespace-nowrap scrollbar-none select-all text-blue-400">
                  {computedSftpCommand}
                </code>
                <button
                  type="button"
                  onClick={() => copyToClipboard(computedSftpCommand, 'sftp_cmd')}
                  className="px-2.5 py-1 rounded-[var(--radius-sm)] bg-blue-600 hover:bg-blue-700 text-white text-[11px] font-semibold flex items-center gap-1 shrink-0 transition-colors cursor-pointer"
                  title="Salin perintah SFTP"
                >
                  {copiedKey === 'sftp_cmd' ? (
                    <>
                      <Check className="w-3.5 h-3.5" />
                      <span>Tersalin!</span>
                    </>
                  ) : (
                    <>
                      <Copy className="w-3.5 h-3.5" />
                      <span>Salin</span>
                    </>
                  )}
                </button>
              </div>
            </div>
          )}

          {/* Connection Details Parameter Grid */}
          <div className="border border-[var(--line)] rounded-[var(--radius-md)] p-4 bg-[var(--surface)] space-y-3">
            <div className="flex items-center justify-between">
              <span className="font-semibold text-xs text-[var(--ink)]">
                Parameter Koneksi Server
              </span>
              <button
                type="button"
                onClick={() => setShowAdvancedSettings((prev) => !prev)}
                className="text-[11px] text-[var(--primary)] hover:underline flex items-center gap-1"
              >
                <Sliders className="w-3 h-3" />
                <span>{showAdvancedSettings ? 'Tutup Penyesuaian' : 'Sesuaikan Host & Port'}</span>
              </button>
            </div>

            {/* Optional customization inputs */}
            {showAdvancedSettings && (
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-2 pb-1 border-b border-[var(--line)]">
                <div>
                  <label className="block text-[11px] font-medium text-[var(--ink-muted)] mb-1">
                    Host / Domain Server
                  </label>
                  <input
                    type="text"
                    value={customHost}
                    onChange={(e) => setCustomHost(e.target.value)}
                    placeholder="example.com atau 123.45.67.89"
                    className="w-full px-2.5 py-1.5 font-mono text-xs rounded-[var(--radius-sm)] border border-[var(--line)] bg-[var(--raised)] text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                  />
                </div>
                <div>
                  <label className="block text-[11px] font-medium text-[var(--ink-muted)] mb-1">
                    Port SSH (Standar: 22)
                  </label>
                  <input
                    type="number"
                    min={1}
                    max={65535}
                    value={customPort}
                    onChange={(e) => setCustomPort(Number(e.target.value))}
                    className="w-full px-2.5 py-1.5 font-mono text-xs rounded-[var(--radius-sm)] border border-[var(--line)] bg-[var(--raised)] text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                  />
                </div>
              </div>
            )}

            <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 pt-1">
              <div className="space-y-1">
                <span className="text-[11px] text-[var(--ink-muted)] block">Host / Alamat:</span>
                <div className="flex items-center gap-1">
                  <span className="font-mono text-xs font-semibold text-[var(--ink)] truncate" title={effectiveHost}>
                    {effectiveHost}
                  </span>
                  <button
                    onClick={() => copyToClipboard(effectiveHost, 'param_host')}
                    className="text-[var(--ink-muted)] hover:text-[var(--ink)] p-0.5"
                    title="Salin Host"
                  >
                    {copiedKey === 'param_host' ? <Check className="w-3 h-3 text-[var(--success)]" /> : <Copy className="w-3 h-3" />}
                  </button>
                </div>
              </div>

              <div className="space-y-1">
                <span className="text-[11px] text-[var(--ink-muted)] block">Port:</span>
                <span className="font-mono text-xs font-semibold text-[var(--ink)]">
                  {effectivePort}
                </span>
              </div>

              <div className="space-y-1">
                <span className="text-[11px] text-[var(--ink-muted)] block">Pengguna Linux:</span>
                <div className="flex items-center gap-1">
                  <span className="font-mono text-xs font-semibold text-[var(--ink)] text-emerald-600 dark:text-emerald-400">
                    {effectiveUser}
                  </span>
                  <button
                    onClick={() => copyToClipboard(effectiveUser, 'param_user')}
                    className="text-[var(--ink-muted)] hover:text-[var(--ink)] p-0.5"
                    title="Salin Username"
                  >
                    {copiedKey === 'param_user' ? <Check className="w-3 h-3 text-[var(--success)]" /> : <Copy className="w-3 h-3" />}
                  </button>
                </div>
              </div>

              <div className="space-y-1">
                <span className="text-[11px] text-[var(--ink-muted)] block">Autentikasi:</span>
                <div className="flex items-center gap-1 text-[var(--ink)]">
                  <Key className="w-3 h-3 text-amber-500" />
                  <span className="font-medium text-xs">
                    {info?.has_ssh_keys ? `${info.ssh_keys_count} Kunci Publik` : 'Kunci SSH Publik'}
                  </span>
                </div>
              </div>
            </div>
          </div>

          {/* Navigation Tabs */}
          <div className="border-b border-[var(--line)] flex items-center gap-2">
            <button
              onClick={() => setActiveTab('ssh')}
              disabled={info?.shell_policy === 'sftp' || info?.shell_policy === 'none'}
              className={`pb-2.5 text-xs font-semibold border-b-2 flex items-center gap-1.5 transition-colors cursor-pointer ${
                activeTab === 'ssh'
                  ? 'border-[var(--primary)] text-[var(--primary)]'
                  : 'border-transparent text-[var(--ink-muted)] hover:text-[var(--ink)]'
              } ${info?.shell_policy === 'sftp' || info?.shell_policy === 'none' ? 'opacity-40 cursor-not-allowed' : ''}`}
            >
              <Terminal className="w-3.5 h-3.5" />
              <span>SSH Terminal</span>
            </button>

            <button
              onClick={() => setActiveTab('sftp')}
              disabled={info?.shell_policy === 'none'}
              className={`pb-2.5 text-xs font-semibold border-b-2 flex items-center gap-1.5 transition-colors cursor-pointer ${
                activeTab === 'sftp'
                  ? 'border-[var(--primary)] text-[var(--primary)]'
                  : 'border-transparent text-[var(--ink-muted)] hover:text-[var(--ink)]'
              } ${info?.shell_policy === 'none' ? 'opacity-40 cursor-not-allowed' : ''}`}
            >
              <Folder className="w-3.5 h-3.5" />
              <span>SFTP & GUI Client</span>
            </button>

            <button
              onClick={() => setActiveTab('config')}
              className={`pb-2.5 text-xs font-semibold border-b-2 flex items-center gap-1.5 transition-colors cursor-pointer ${
                activeTab === 'config'
                  ? 'border-[var(--primary)] text-[var(--primary)]'
                  : 'border-transparent text-[var(--ink-muted)] hover:text-[var(--ink)]'
              }`}
            >
              <FileText className="w-3.5 h-3.5" />
              <span>~/.ssh/config & Key</span>
            </button>
          </div>

          {/* TAB 1: SSH Terminal Details */}
          {activeTab === 'ssh' && (
            <div className="space-y-4">
              <div className="space-y-1.5">
                <span className="text-[11px] font-semibold text-[var(--ink-muted)] block">
                  Perintah SSH Standar (Home Directory):
                </span>
                <div className="flex items-center gap-2 bg-[#09191F] text-[#E4EEF0] p-2.5 rounded-[var(--radius-sm)] border border-[#1F3A45]">
                  <code className="font-mono text-[11px] flex-1 overflow-x-auto whitespace-nowrap select-all text-gray-300">
                    {computedSshSimpleCommand || '-'}
                  </code>
                  <button
                    type="button"
                    onClick={() => computedSshSimpleCommand && copyToClipboard(computedSshSimpleCommand, 'ssh_simple')}
                    className="px-2 py-0.5 rounded border border-[#1F3A45] hover:bg-[#143039] text-[11px] text-gray-300 flex items-center gap-1 transition-colors"
                  >
                    {copiedKey === 'ssh_simple' ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
                    <span>{copiedKey === 'ssh_simple' ? 'Tersalin' : 'Salin'}</span>
                  </button>
                </div>
              </div>

              <div className="p-3.5 rounded-[var(--radius-md)] bg-[var(--surface)] border border-[var(--line)] space-y-2">
                <span className="font-semibold text-xs text-[var(--ink)] block">
                  Penjelasan Parameter Perintah:
                </span>
                <ul className="list-disc list-inside space-y-1 text-[11px] text-[var(--ink-muted)] leading-relaxed">
                  <li>
                    <code className="font-mono bg-[var(--raised)] px-1 py-0.5 rounded text-[var(--ink)]">-t</code>: Memaksa alokasi pseudo-terminal (PTY) agar shell interaktif berjalan normal saat menjalankan perintah otomatis.
                  </li>
                  <li>
                    <code className="font-mono bg-[var(--raised)] px-1 py-0.5 rounded text-[var(--ink)]">cd &apos;...&apos;</code>: Membuka terminal secara langsung di folder aktif saat ini tanpa perlu mengetik navigasi manual.
                  </li>
                  <li>
                    <code className="font-mono bg-[var(--raised)] px-1 py-0.5 rounded text-[var(--ink)]">exec \$SHELL -l</code>: Mengganti proses dengan login shell default tenant (bash) dengan environment lengkap.
                  </li>
                </ul>
              </div>
            </div>
          )}

          {/* TAB 2: SFTP & GUI Clients */}
          {activeTab === 'sftp' && (
            <div className="space-y-4">
              <div className="space-y-1.5">
                <span className="text-[11px] font-semibold text-[var(--ink-muted)] block">
                  SFTP Deep-Link URL (Cyberduck / FileZilla):
                </span>
                <div className="flex items-center gap-2 bg-[#09191F] text-[#E4EEF0] p-2.5 rounded-[var(--radius-sm)] border border-[#1F3A45]">
                  <code className="font-mono text-[11px] flex-1 overflow-x-auto whitespace-nowrap select-all text-blue-300">
                    {computedSftpUrl || '-'}
                  </code>
                  <button
                    type="button"
                    onClick={() => computedSftpUrl && copyToClipboard(computedSftpUrl, 'sftp_url')}
                    className="px-2 py-0.5 rounded border border-[#1F3A45] hover:bg-[#143039] text-[11px] text-gray-300 flex items-center gap-1 transition-colors"
                  >
                    {copiedKey === 'sftp_url' ? <Check className="w-3 h-3 text-emerald-400" /> : <Copy className="w-3 h-3" />}
                    <span>{copiedKey === 'sftp_url' ? 'Tersalin' : 'Salin URL'}</span>
                  </button>
                </div>
              </div>

              <div className="p-3.5 rounded-[var(--radius-md)] bg-[var(--surface)] border border-[var(--line)] space-y-2">
                <span className="font-semibold text-xs text-[var(--ink)] block">
                  Panduan Pengaturan FileZilla / WinSCP / Cyberduck:
                </span>
                <div className="grid grid-cols-2 gap-2 text-[11px] text-[var(--ink-muted)] pt-1">
                  <div>
                    <strong className="text-[var(--ink)]">Protocol:</strong> SFTP - SSH File Transfer Protocol
                  </div>
                  <div>
                    <strong className="text-[var(--ink)]">Port:</strong> {effectivePort}
                  </div>
                  <div>
                    <strong className="text-[var(--ink)]">Host:</strong> {effectiveHost}
                  </div>
                  <div>
                    <strong className="text-[var(--ink)]">Logon Type:</strong> Key file (Kunci privat Anda)
                  </div>
                </div>
                <p className="text-[11px] text-[var(--ink-muted)] pt-1">
                  Akses langsung folder: Masukkan <code className="font-mono bg-[var(--raised)] px-1 py-0.5 rounded text-[var(--ink)]">{effectivePath}</code> pada field Remote Site / Direktori Jauh.
                </p>
              </div>
            </div>
          )}

          {/* TAB 3: ~/.ssh/config & Key Management */}
          {activeTab === 'config' && (
            <div className="space-y-4">
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="text-[11px] font-semibold text-[var(--ink-muted)] block">
                    Konfigurasi OpenSSH Client (~/.ssh/config):
                  </span>
                  <button
                    type="button"
                    onClick={() => copyToClipboard(computedConfigSnippet, 'ssh_cfg')}
                    className="text-[11px] text-[var(--primary)] hover:underline flex items-center gap-1"
                  >
                    {copiedKey === 'ssh_cfg' ? <Check className="w-3 h-3 text-[var(--success)]" /> : <Copy className="w-3 h-3" />}
                    <span>{copiedKey === 'ssh_cfg' ? 'Tersalin!' : 'Salin Snippet'}</span>
                  </button>
                </div>
                <div className="bg-[#09191F] text-[#E4EEF0] p-3 rounded-[var(--radius-sm)] border border-[#1F3A45]">
                  <pre className="font-mono text-[11px] overflow-x-auto whitespace-pre leading-relaxed text-amber-300">
                    {computedConfigSnippet}
                  </pre>
                </div>
                <p className="text-[11px] text-[var(--ink-muted)]">
                  Setelah disimpan di <code className="font-mono bg-[var(--raised)] px-1 py-0.5 rounded text-[var(--ink)]">~/.ssh/config</code>, Anda cukup mengetik <code className="font-mono bg-[var(--raised)] px-1 py-0.5 rounded text-[var(--ink)]">ssh {siteDomain || `${effectiveUser}-server`}</code> dari terminal komputer lokal.
                </p>
              </div>

              {/* SSH Key Registration Banner */}
              <div className="p-3.5 rounded-[var(--radius-md)] bg-[var(--raised)] border border-[var(--line)] flex items-center justify-between gap-4">
                <div className="flex items-center gap-2.5">
                  <Shield className="w-4 h-4 text-emerald-500 shrink-0" />
                  <div>
                    <span className="font-semibold text-xs text-[var(--ink)] block">
                      Status Kunci Publik SSH
                    </span>
                    <span className="text-[11px] text-[var(--ink-muted)] block mt-0.5">
                      {info?.has_ssh_keys
                        ? `${info.ssh_keys_count} kunci terdaftar di server (root-managed authorized_keys)`
                        : 'Belum ada kunci terdaftar. Tambahkan public key Anda agar bisa login tanpa password.'}
                    </span>
                  </div>
                </div>

                {onOpenSshKeys ? (
                  <button
                    type="button"
                    onClick={() => {
                      onClose();
                      onOpenSshKeys();
                    }}
                    className="px-3 py-1.5 rounded-[var(--radius-sm)] bg-[var(--surface)] hover:bg-[var(--raised)] border border-[var(--line)] text-[var(--ink)] text-xs font-semibold shrink-0 transition-colors cursor-pointer"
                  >
                    Kelola Kunci SSH
                  </button>
                ) : (
                  <a
                    href="/security"
                    className="px-3 py-1.5 rounded-[var(--radius-sm)] bg-[var(--surface)] hover:bg-[var(--raised)] border border-[var(--line)] text-[var(--ink)] text-xs font-semibold shrink-0 transition-colors"
                  >
                    Profil Keamanan
                  </a>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Modal Footer */}
        <div className="flex items-center justify-between px-6 py-3.5 border-t border-[var(--line)] bg-[var(--raised)]">
          <div className="text-[11px] text-[var(--ink-muted)] flex items-center gap-1.5">
            <Radio className="w-3.5 h-3.5 text-emerald-500" />
            <span>Port 22 SSH Daemon aktif &bull; Drop-in /etc/ssh/sshd_config.d/50-panel.conf</span>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="px-4 py-1.5 rounded-[var(--radius-md)] bg-[var(--surface)] border border-[var(--line)] hover:bg-[var(--surface-hover)] text-xs font-semibold text-[var(--ink)] transition-colors cursor-pointer"
          >
            Selesai
          </button>
        </div>
      </div>
    </div>
  );
};
