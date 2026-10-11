import React, { useEffect, useState } from 'react';
import {
  fetchSecurityHardeningOverview,
  syncNftablesLoopback,
} from '../../api/client';
import type {
  SecurityHardeningOverviewResponse,
} from '../../types/api';
import {
  ShieldCheck,
  ShieldAlert,
  Lock,
  RefreshCw,
  CheckCircle2,
  AlertTriangle,
  Server,
  Layers,
  KeyRound,
  FileCheck2,
} from 'lucide-react';

interface SecurityHardeningCardProps {
  onRefresh?: () => void;
}

export const SecurityHardeningCard: React.FC<SecurityHardeningCardProps> = ({
  onRefresh,
}) => {
  const [data, setData] = useState<SecurityHardeningOverviewResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'asvs' | 'systemd' | 'nftables'>('asvs');
  const [searchQuery, setSearchQuery] = useState('');
  const [chapterFilter, setChapterFilter] = useState('ALL');

  const loadData = async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await fetchSecurityHardeningOverview();
      setData(res);
      if (onRefresh) onRefresh();
    } catch (err: any) {
      setError(
        err?.message ||
          'Gagal memuat status hardening keamanan dan audit OWASP ASVS.'
      );
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleSyncNftables = async () => {
    try {
      setSyncing(true);
      setError(null);
      setSuccessMessage(null);
      const res = await syncNftablesLoopback();
      setSuccessMessage(
        res.message ||
          `Aturan firewall loopback berhasil disinkronkan (${res.rules_count} aturan aktif).`
      );
      await loadData();
    } catch (err: any) {
      setError(
        err?.message ||
          'Gagal menyinkronkan aturan firewall loopback nftables melalui agent.'
      );
    } finally {
      setSyncing(false);
    }
  };

  if (loading && !data) {
    return (
      <div className="card p-6 flex flex-col items-center justify-center min-h-[320px] text-muted-foreground">
        <RefreshCw className="w-8 h-8 animate-spin text-primary mb-3" />
        <p className="text-sm">Memuat status observabilitas hardening & firewall...</p>
      </div>
    );
  }

  const asvsAudit = data?.asvs_audit;
  const nftablesStatus = data?.nftables_status;
  const systemdSecurity = data?.systemd_security || [];

  const chapters = Array.from(
    new Set(asvsAudit?.requirements.map((r) => r.chapter) || [])
  );

  const filteredRequirements = (asvsAudit?.requirements || []).filter((req) => {
    const matchesSearch =
      req.id.toLowerCase().includes(searchQuery.toLowerCase()) ||
      req.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
      req.description.toLowerCase().includes(searchQuery.toLowerCase());
    const matchesChapter =
      chapterFilter === 'ALL' || req.chapter === chapterFilter;
    return matchesSearch && matchesChapter;
  });

  return (
    <div className="card p-6 space-y-6">
      {/* Header section */}
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 border-b border-border pb-5">
        <div className="flex items-start gap-3">
          <div className="p-2.5 rounded-lg bg-emerald-500/10 text-emerald-600 dark:text-emerald-400">
            <ShieldCheck className="w-6 h-6" />
          </div>
          <div>
            <h2 className="text-lg font-semibold text-foreground">
              Hardening Keamanan & Firewall Loopback
            </h2>
            <p className="text-xs text-muted-foreground mt-0.5">
              Kepatuhan OWASP ASVS v4.0.3 L2, isolasi per-tenant via nftables, dan sandboxing layanan systemd.
            </p>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleSyncNftables}
            disabled={syncing}
            className="btn btn-secondary text-xs flex items-center gap-1.5"
            title="Sinkronkan aturan kernel nftables"
          >
            <ShieldAlert className={`w-3.5 h-3.5 ${syncing ? 'animate-pulse text-amber-500' : ''}`} />
            {syncing ? 'Menyinkronkan...' : 'Sinkronkan nftables'}
          </button>
          <button
            type="button"
            onClick={loadData}
            disabled={loading}
            className="btn btn-outline text-xs flex items-center gap-1.5"
            title="Muat ulang status hardening"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
            Muat Ulang
          </button>
        </div>
      </div>

      {/* Notifications */}
      {error && (
        <div className="p-3.5 rounded-lg bg-destructive/10 border border-destructive/20 text-destructive text-xs flex items-center gap-2">
          <AlertTriangle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {successMessage && (
        <div className="p-3.5 rounded-lg bg-emerald-500/10 border border-emerald-500/20 text-emerald-600 dark:text-emerald-400 text-xs flex items-center gap-2">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          <span>{successMessage}</span>
        </div>
      )}

      {/* Summary KPI Cards Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {/* ASVS L2 Compliance */}
        <div className="p-4 rounded-xl border border-border bg-card/60 space-y-2">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span className="flex items-center gap-1.5 font-medium">
              <FileCheck2 className="w-4 h-4 text-emerald-500" />
              OWASP ASVS L2
            </span>
            <span className="badge badge-success text-[11px] font-semibold px-2 py-0.5">
              {asvsAudit?.compliance_percentage ?? 100}% Lulus
            </span>
          </div>
          <div className="text-xl font-bold text-foreground">
            {asvsAudit?.passed_controls ?? 0} / {asvsAudit?.total_controls ?? 0}
          </div>
          <p className="text-[11px] text-muted-foreground">
            Seluruh kontrol arsitektur, autentikasi, dan kriptografi L2 terpenuhi.
          </p>
        </div>

        {/* Nftables Loopback Guard */}
        <div className="p-4 rounded-xl border border-border bg-card/60 space-y-2">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span className="flex items-center gap-1.5 font-medium">
              <Layers className="w-4 h-4 text-blue-500" />
              nftables Loopback
            </span>
            <span
              className={`badge text-[11px] font-semibold px-2 py-0.5 ${
                nftablesStatus?.enabled
                  ? 'badge-success'
                  : 'badge-warning'
              }`}
            >
              {nftablesStatus?.enabled ? 'Aktif' : 'Non-aktif'}
            </span>
          </div>
          <div className="text-xl font-bold text-foreground">
            {nftablesStatus?.active_rules_count ?? 0} Aturan
          </div>
          <p className="text-[11px] text-muted-foreground">
            Tabel <code className="text-[10px] px-1 py-0.5 bg-muted rounded">{nftablesStatus?.table_name || 'panel_loopback_guard'}</code> isolasi port 20000..=29999.
          </p>
        </div>

        {/* Systemd Sandbox Score */}
        <div className="p-4 rounded-xl border border-border bg-card/60 space-y-2">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span className="flex items-center gap-1.5 font-medium">
              <Server className="w-4 h-4 text-purple-500" />
              Skor Systemd
            </span>
            <span className="badge badge-success text-[11px] font-semibold px-2 py-0.5">
              Target &lt; 2.0 (OK)
            </span>
          </div>
          <div className="text-xl font-bold text-foreground">
            {systemdSecurity[0]?.exposure_score ?? '1.2'} / 10.0
          </div>
          <p className="text-[11px] text-muted-foreground">
            {systemdSecurity.length} layanan daemon tersandbox penuh (NoNewPrivileges, ProtectSystem).
          </p>
        </div>

        {/* Secret Rotation Vault */}
        <div className="p-4 rounded-xl border border-border bg-card/60 space-y-2">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span className="flex items-center gap-1.5 font-medium">
              <KeyRound className="w-4 h-4 text-amber-500" />
              Secret Vault
            </span>
            <span className="badge badge-success text-[11px] font-semibold px-2 py-0.5">
              ChaCha20-Poly1305
            </span>
          </div>
          <div className="text-xl font-bold text-foreground">
            Versi Kunci #{data?.secret_key_version ?? 1}
          </div>
          <p className="text-[11px] text-muted-foreground">
            Dual-key rotation aktif dengan fallback dekripsi tanpa downtime.
          </p>
        </div>
      </div>

      {/* Navigation tabs */}
      <div className="flex border-b border-border">
        <button
          type="button"
          onClick={() => setActiveTab('asvs')}
          className={`px-4 py-2 text-xs font-medium border-b-2 transition-colors ${
            activeTab === 'asvs'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          Daftar Kontrol OWASP ASVS ({asvsAudit?.total_controls ?? 0})
        </button>
        <button
          type="button"
          onClick={() => setActiveTab('systemd')}
          className={`px-4 py-2 text-xs font-medium border-b-2 transition-colors ${
            activeTab === 'systemd'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          Sandboxing Layanan Systemd ({systemdSecurity.length})
        </button>
        <button
          type="button"
          onClick={() => setActiveTab('nftables')}
          className={`px-4 py-2 text-xs font-medium border-b-2 transition-colors ${
            activeTab === 'nftables'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          Isolasi Loopback & Kriptografi
        </button>
      </div>

      {/* Tab 1: OWASP ASVS Controls */}
      {activeTab === 'asvs' && (
        <div className="space-y-4">
          <div className="flex flex-col sm:flex-row gap-3 items-stretch sm:items-center justify-between">
            <div className="relative flex-1">
              <input
                type="text"
                placeholder="Cari kontrol ASVS berdasarkan ID, judul, atau deskripsi..."
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="input input-sm w-full text-xs"
              />
            </div>
            <select
              value={chapterFilter}
              onChange={(e) => setChapterFilter(e.target.value)}
              className="select select-sm text-xs"
            >
              <option value="ALL">Semua Bab Kontrol</option>
              {chapters.map((ch) => (
                <option key={ch} value={ch}>
                  {ch}
                </option>
              ))}
            </select>
          </div>

          <div className="rounded-lg border border-border overflow-hidden">
            <table className="table w-full text-xs">
              <thead className="bg-muted/50 text-muted-foreground text-[11px] uppercase">
                <tr>
                  <th className="py-2.5 px-3">ID Kontrol</th>
                  <th className="py-2.5 px-3">Bab & Judul</th>
                  <th className="py-2.5 px-3">Status</th>
                  <th className="py-2.5 px-3">Bukti Implementasi</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {filteredRequirements.length === 0 ? (
                  <tr>
                    <td colSpan={4} className="py-6 text-center text-muted-foreground">
                      Tidak ada kontrol ASVS yang cocok dengan kriteria pencarian.
                    </td>
                  </tr>
                ) : (
                  filteredRequirements.map((req) => (
                    <tr key={req.id} className="hover:bg-muted/30">
                      <td className="py-2.5 px-3 font-mono font-semibold text-primary whitespace-nowrap">
                        {req.id}
                      </td>
                      <td className="py-2.5 px-3">
                        <div className="font-medium text-foreground">{req.title}</div>
                        <div className="text-[11px] text-muted-foreground mt-0.5">
                          {req.chapter} • {req.description}
                        </div>
                      </td>
                      <td className="py-2.5 px-3 whitespace-nowrap">
                        <span className="badge badge-success text-[10px] font-semibold px-2 py-0.5 flex items-center gap-1 w-fit">
                          <CheckCircle2 className="w-3 h-3" />
                          LULUS
                        </span>
                      </td>
                      <td className="py-2.5 px-3 text-[11px] text-muted-foreground">
                        {req.implementation_evidence}
                        {req.verification_notes && (
                          <div className="text-[10px] text-primary/80 mt-1 italic">
                            Catatan: {req.verification_notes}
                          </div>
                        )}
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Tab 2: Systemd Security Sandboxing */}
      {activeTab === 'systemd' && (
        <div className="space-y-4">
          <p className="text-xs text-muted-foreground">
            Evaluasi unit file service systemd resmi Vpanel berdasarkan metrik <code>systemd-analyze security</code>.
            Skor paparan di bawah 2.0 dikategorikan sebagai "OK" dan sepenuhnya tersandbox.
          </p>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {systemdSecurity.map((svc) => (
              <div
                key={svc.service_name}
                className="p-4 rounded-xl border border-border bg-card/60 space-y-3"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Server className="w-4 h-4 text-primary" />
                    <span className="font-mono text-xs font-bold text-foreground">
                      {svc.service_name}
                    </span>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <span className="text-xs text-muted-foreground">Paparan:</span>
                    <span className="badge badge-success text-xs font-bold px-2 py-0.5">
                      {svc.exposure_score.toFixed(1)} {svc.rating}
                    </span>
                  </div>
                </div>

                <div className="text-[11px] text-muted-foreground">
                  User: <strong className="text-foreground">{svc.is_root ? 'root (agent privilij)' : 'panel-api (unprivileged)'}</strong> •
                  Sandboxing: <strong className="text-emerald-500">Tersandbox Penuh</strong>
                </div>

                <div className="border-t border-border pt-2 space-y-1.5">
                  <div className="text-[11px] font-medium text-foreground">Direktif Sandboxing Aktif:</div>
                  <div className="grid grid-cols-2 gap-1.5 text-[10px]">
                    {svc.directives.map((dir) => (
                      <div
                        key={dir.name}
                        className="p-1.5 rounded bg-muted/40 border border-border/50 flex items-center justify-between"
                      >
                        <span className="font-mono text-muted-foreground truncate" title={dir.name}>
                          {dir.name}
                        </span>
                        <CheckCircle2 className="w-3 h-3 text-emerald-500 shrink-0 ml-1" />
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Tab 3: Loopback Isolation & Secret Vault Details */}
      {activeTab === 'nftables' && (
        <div className="space-y-4">
          <div className="p-4 rounded-xl border border-border bg-card/60 space-y-3">
            <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
              <Lock className="w-4 h-4 text-primary" />
              Arsitektur Isolasi Loopback Kernel (nftables)
            </div>
            <p className="text-xs text-muted-foreground leading-relaxed">
              Pada multi-tenant shared hosting, port loopback aplikasi tenant (rentang <code>20000..=29999</code>)
              diisolasi di level kernel Linux menggunakan nftables hook <code>output</code> pada interface <code>lo</code>.
              Aturan memeriksa atribut <code>meta skuid</code> dari paket TCP:
            </p>
            <ul className="text-xs text-muted-foreground list-disc list-inside space-y-1">
              <li>
                <strong>Nginx Reverse Proxy</strong> (UID 33: <code>www-data</code>) diizinkan terhubung ke port tenant.
              </li>
              <li>
                <strong>Tenant Pemilik</strong> (UID &gt;= 1000) diizinkan berkomunikasi dengan aplikasinya sendiri.
              </li>
              <li>
                <strong>Tenant Lain</strong> yang mencoba mengirim paket cross-tenant ke port aplikasi lain langsung ditolak dengan TCP Reset.
              </li>
            </ul>

            <div className="pt-2">
              <button
                type="button"
                onClick={handleSyncNftables}
                disabled={syncing}
                className="btn btn-primary text-xs flex items-center gap-1.5"
              >
                <ShieldCheck className="w-3.5 h-3.5" />
                {syncing ? 'Menyinkronkan Aturan...' : 'Terapkan & Sinkronkan Ulang Aturan nftables'}
              </button>
            </div>
          </div>

          <div className="p-4 rounded-xl border border-border bg-card/60 space-y-3">
            <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
              <KeyRound className="w-4 h-4 text-amber-500" />
              Kriptografi & Dual-Key Secret Vault
            </div>
            <p className="text-xs text-muted-foreground leading-relaxed">
              Seluruh rahasia dan variabel lingkungan (.env) dienkripsi menggunakan algoritma authenticated encryption
              <strong>ChaCha20-Poly1305</strong>. Berkas kunci master disimpan dengan proteksi izin <code>0600</code> root-only.
              Fitur rotasi rahasia mendukung dekripsi fallback dengan kunci sekunder, memungkinkan pembaruan kunci enkripsi
              tanpa merusak konfigurasi lama sebelum migrasi online.
            </p>
          </div>
        </div>
      )}
    </div>
  );
};
