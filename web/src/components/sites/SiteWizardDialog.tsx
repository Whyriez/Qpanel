import React, { useState, useEffect } from 'react';
import {
  X,
  Globe,
  ArrowRight,
  ArrowLeft,
  Loader2,
  CheckCircle2,
  AlertCircle,
  FileCode2,
  Server,
  Terminal,
  ShieldCheck,
} from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { TenantListItem, Site, CreateSiteRequest } from '../../types/api';
import { fetchTenants, createSite } from '../../api/client';
import { useAuth } from '../../context/AuthContext';

export interface SiteWizardDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: (site: Site) => void;
}

export const SiteWizardDialog: React.FC<SiteWizardDialogProps> = ({
  isOpen,
  onClose,
  onSuccess,
}) => {
  const { t } = useTranslation();
  const { user } = useAuth();

  const [step, setStep] = useState<1 | 2 | 3 | 4>(1);
  const [siteType, setSiteType] = useState<'static' | 'php' | 'node'>('static');
  const [domain, setDomain] = useState('');
  const [spaFallback, setSpaFallback] = useState(false);
  const [forceHttps, setForceHttps] = useState(true);
  const [selectedTenantId, setSelectedTenantId] = useState<number | null>(null);

  // Node.js specific fields (Phase 7)
  const [nodeVersion, setNodeVersion] = useState<string>('22.14.0');
  const [nodeEntrypoint, setNodeEntrypoint] = useState<string>('index.js');
  const [nodeEntrypointType, setNodeEntrypointType] = useState<string>('script');

  const [tenants, setTenants] = useState<TenantListItem[]>([]);
  const [isLoadingTenants, setIsLoadingTenants] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  useEffect(() => {
    if (isOpen) {
      setStep(1);
      setSiteType('static');
      setDomain('');
      setSpaFallback(false);
      setForceHttps(true);
      setErrorMessage(null);

      // Load tenants for tenant selection
      setIsLoadingTenants(true);
      fetchTenants()
        .then((data) => {
          setTenants(data);
          // If current logged-in user is a regular user or matches tenant, auto-select
          if (user?.role !== 'admin' && data.length > 0) {
            setSelectedTenantId(data[0].id);
          } else if (data.length > 0) {
            setSelectedTenantId(data[0].id);
          }
        })
        .catch((err) => {
          setErrorMessage('Gagal memuat daftar tenant: ' + (err.message || 'Error'));
        })
        .finally(() => {
          setIsLoadingTenants(false);
        });
    }
  }, [isOpen, user]);

  if (!isOpen) return null;

  const selectedTenant = tenants.find((t) => t.id === selectedTenantId);
  const tenantSlug = selectedTenant?.linux_username || (selectedTenant ? `p_${selectedTenant.username}` : 'p_<slug>');
  const docrootPreview = `/home/${tenantSlug}/sites/${domain.trim() || '<domain>'}/public`;

  const handleNextStep1 = () => {
    setErrorMessage(null);
    setStep(2);
  };

  const handleNextStep2 = (e: React.FormEvent) => {
    e.preventDefault();
    const cleanDomain = domain.trim().toLowerCase();
    if (!cleanDomain) {
      setErrorMessage('Nama domain wajib diisi.');
      return;
    }
    // Simple frontend sanity check for domain name format
    if (!cleanDomain.includes('.') || cleanDomain.startsWith('.') || cleanDomain.endsWith('.')) {
      setErrorMessage('Format domain tidak valid (contoh: example.com atau sub.domain.id).');
      return;
    }
    setErrorMessage(null);
    setStep(3);
  };

  const handleNextStep3 = () => {
    if (!selectedTenantId) {
      setErrorMessage('Pilih tenant pemilik situs untuk melanjutkan.');
      return;
    }
    setErrorMessage(null);
    setStep(4);
  };

  const handleFinalSubmit = async () => {
    setIsSubmitting(true);
    setErrorMessage(null);

    const payload: CreateSiteRequest = {
      domain: domain.trim().toLowerCase(),
      user_id: selectedTenantId || undefined,
      spa_fallback: siteType === 'static' ? spaFallback : false,
      force_https: forceHttps,
      ...(siteType === 'node'
        ? {
            runtime: 'nodejs',
            version: nodeVersion,
            entrypoint: nodeEntrypoint.trim() || undefined,
            entrypoint_type: nodeEntrypointType,
          }
        : {}),
    };

    try {
      const created = await createSite(payload);
      onSuccess(created);
      onClose();
    } catch (err: unknown) {
      const e = err as Error;
      setErrorMessage(e.message || 'Gagal membuat situs. Silakan periksa konfigurasi domain.');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-xs animate-in fade-in duration-150">
      <div className="bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-lg)] shadow-[var(--shadow-floating)] w-full max-w-2xl overflow-hidden flex flex-col max-h-[90vh]">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-[var(--line)] bg-[var(--raised)]">
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-[var(--radius-md)] bg-[var(--primary)]/10 text-[var(--primary)] flex items-center justify-center">
              <Globe className="w-4 h-4" />
            </div>
            <div>
              <h2 className="text-base font-semibold text-[var(--ink)]">
                {t('sites.wizard.title')}
              </h2>
              <p className="text-xs text-[var(--ink-muted)]">
                Langkah {step} dari 4 &bull;{' '}
                {step === 1 && t('sites.wizard.step1Title')}
                {step === 2 && t('sites.wizard.step2Title')}
                {step === 3 && t('sites.wizard.step3Title')}
                {step === 4 && t('sites.wizard.step4Title')}
              </p>
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded-[var(--radius-sm)] text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--surface)] transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Step Progress Indicators */}
        <div className="grid grid-cols-4 border-b border-[var(--line)] bg-[var(--bg)] text-xs font-medium">
          <div
            className={`py-2 px-3 text-center border-b-2 transition-colors ${
              step >= 1
                ? 'border-[var(--primary)] text-[var(--primary)] font-semibold'
                : 'border-transparent text-[var(--ink-muted)]'
            }`}
          >
            1. Tipe
          </div>
          <div
            className={`py-2 px-3 text-center border-b-2 transition-colors ${
              step >= 2
                ? 'border-[var(--primary)] text-[var(--primary)] font-semibold'
                : 'border-transparent text-[var(--ink-muted)]'
            }`}
          >
            2. Domain
          </div>
          <div
            className={`py-2 px-3 text-center border-b-2 transition-colors ${
              step >= 3
                ? 'border-[var(--primary)] text-[var(--primary)] font-semibold'
                : 'border-transparent text-[var(--ink-muted)]'
            }`}
          >
            3. Pemilik
          </div>
          <div
            className={`py-2 px-3 text-center border-b-2 transition-colors ${
              step >= 4
                ? 'border-[var(--primary)] text-[var(--primary)] font-semibold'
                : 'border-transparent text-[var(--ink-muted)]'
            }`}
          >
            4. Tinjau
          </div>
        </div>

        {/* Content Body */}
        <div className="p-6 overflow-y-auto flex-1">
          {errorMessage && (
            <div className="mb-5 p-3 rounded-[var(--radius-md)] bg-[var(--danger)]/10 border border-[var(--danger)]/20 text-[var(--danger)] text-xs flex items-start gap-2.5">
              <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
              <span>{errorMessage}</span>
            </div>
          )}

          {/* STEP 1: Pilih Tipe Situs */}
          {step === 1 && (
            <div className="space-y-4">
              <div className="text-xs text-[var(--ink-muted)] mb-3">
                {t('sites.wizard.step1Desc')}
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                {/* Static Site (Active) */}
                <div
                  onClick={() => setSiteType('static')}
                  className={`cursor-pointer p-4 rounded-[var(--radius-md)] border transition-all ${
                    siteType === 'static'
                      ? 'border-[var(--primary)] bg-[var(--primary)]/5 shadow-xs'
                      : 'border-[var(--line)] bg-[var(--raised)] hover:border-[var(--ink-muted)]'
                  }`}
                >
                  <div className="flex items-center justify-between mb-2">
                    <FileCode2 className="w-5 h-5 text-[var(--primary)]" />
                    <span className="text-[10px] px-2 py-0.5 rounded-full bg-[var(--success)]/10 text-[var(--success)] font-semibold uppercase">
                      Tersedia
                    </span>
                  </div>
                  <h3 className="text-sm font-semibold text-[var(--ink)] mb-1">
                    {t('sites.wizard.staticTitle')}
                  </h3>
                  <p className="text-xs text-[var(--ink-muted)]">
                    {t('sites.wizard.staticDesc')}
                  </p>
                </div>

                {/* PHP (Phase 4 disabled) */}
                <div className="p-4 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] opacity-60 cursor-not-allowed">
                  <div className="flex items-center justify-between mb-2">
                    <Server className="w-5 h-5 text-[var(--ink-muted)]" />
                    <span className="text-[10px] px-2 py-0.5 rounded-full bg-[var(--line)] text-[var(--ink-muted)] font-semibold uppercase">
                      Fase 4
                    </span>
                  </div>
                  <h3 className="text-sm font-semibold text-[var(--ink)] mb-1">
                    {t('sites.wizard.phpTitle')}
                  </h3>
                  <p className="text-xs text-[var(--ink-muted)]">
                    {t('sites.wizard.phpDesc')}
                  </p>
                </div>

                {/* Node.js Application (Available in Phase 7) */}
                <div
                  onClick={() => setSiteType('node')}
                  className={`cursor-pointer p-4 rounded-[var(--radius-md)] border transition-all ${
                    siteType === 'node'
                      ? 'border-[var(--primary)] bg-[var(--primary)]/5 shadow-xs'
                      : 'border-[var(--line)] bg-[var(--raised)] hover:border-[var(--ink-muted)]'
                  }`}
                >
                  <div className="flex items-center justify-between mb-2">
                    <Terminal className="w-5 h-5 text-[var(--primary)]" />
                    <span className="text-[10px] px-2 py-0.5 rounded-full bg-[var(--success)]/10 text-[var(--success)] font-semibold uppercase">
                      Tersedia
                    </span>
                  </div>
                  <h3 className="text-sm font-semibold text-[var(--ink)] mb-1">
                    {t('sites.wizard.nodeTitle')}
                  </h3>
                  <p className="text-xs text-[var(--ink-muted)]">
                    {t('sites.wizard.nodeDesc')}
                  </p>
                </div>
              </div>
            </div>
          )}

          {/* STEP 2: Domain & Routing */}
          {step === 2 && (
            <form id="step2-form" onSubmit={handleNextStep2} className="space-y-4">
              <div>
                <label className="block text-xs font-semibold text-[var(--ink)] mb-1.5">
                  {t('sites.wizard.domainLabel')}{' '}
                  <span className="text-[var(--danger)]">*</span>
                </label>
                <div className="relative">
                  <Globe className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ink-muted)]" />
                  <input
                    type="text"
                    required
                    placeholder="situsku.com atau demo.perusahaan.id"
                    value={domain}
                    onChange={(e) => setDomain(e.target.value.toLowerCase())}
                    className="w-full pl-9 pr-3 py-2 text-xs font-mono rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] text-[var(--ink)] focus:outline-none focus:border-[var(--primary)] focus:ring-1 focus:ring-[var(--primary)]"
                    autoFocus
                  />
                </div>
                <p className="text-[11px] text-[var(--ink-muted)] mt-1.5">
                  {t('sites.wizard.domainHelp')}
                </p>
              </div>

              {/* Node.js Specific Options */}
              {siteType === 'node' && (
                <div className="pt-2 space-y-3 border-t border-[var(--line)]">
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                    <div>
                      <label className="block text-xs font-semibold text-[var(--ink)] mb-1">
                        Versi Node.js
                      </label>
                      <select
                        value={nodeVersion}
                        onChange={(e) => setNodeVersion(e.target.value)}
                        className="w-full text-xs px-3 py-2 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                      >
                        <option value="22.14.0">Node.js v22.14.0 (LTS)</option>
                        <option value="20.18.3">Node.js v20.18.3 (LTS)</option>
                        <option value="18.20.7">Node.js v18.20.7 (LTS)</option>
                      </select>
                    </div>

                    <div>
                      <label className="block text-xs font-semibold text-[var(--ink)] mb-1">
                        Tipe Entrypoint
                      </label>
                      <select
                        value={nodeEntrypointType}
                        onChange={(e) => setNodeEntrypointType(e.target.value)}
                        className="w-full text-xs px-3 py-2 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                      >
                        <option value="script">File Script (index.js)</option>
                        <option value="npm_start">npm start</option>
                        <option value="npm_custom">npm run &lt;custom&gt;</option>
                      </select>
                    </div>
                  </div>

                  {nodeEntrypointType !== 'npm_start' && (
                    <div>
                      <label className="block text-xs font-semibold text-[var(--ink)] mb-1">
                        {nodeEntrypointType === 'script' ? 'Nama File Script' : 'Nama Script npm'}
                      </label>
                      <input
                        type="text"
                        value={nodeEntrypoint}
                        onChange={(e) => setNodeEntrypoint(e.target.value)}
                        placeholder={nodeEntrypointType === 'script' ? 'index.js atau app.js' : 'start:prod'}
                        className="w-full text-xs px-3 py-2 font-mono rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                      />
                    </div>
                  )}

                  <div className="p-2.5 rounded-[var(--radius-sm)] bg-[var(--info)]/10 text-[var(--info)] text-[11px]">
                    Port lokal akan dialokasikan otomatis secara unik dalam rentang 20000 - 29999 dan diproxy oleh Nginx.
                  </div>
                </div>
              )}

              <div className="pt-2 space-y-3 border-t border-[var(--line)]">
                {siteType === 'static' && (
                  <label className="flex items-start gap-3 p-3 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] cursor-pointer hover:bg-[var(--surface)] transition-colors">
                    <input
                      type="checkbox"
                      checked={spaFallback}
                      onChange={(e) => setSpaFallback(e.target.checked)}
                      className="mt-0.5 rounded-[var(--radius-sm)] border-[var(--line)] text-[var(--primary)] focus:ring-[var(--primary)]"
                    />
                    <div>
                      <span className="text-xs font-semibold text-[var(--ink)] block">
                        {t('sites.wizard.spaFallbackLabel')}
                      </span>
                      <span className="text-[11px] text-[var(--ink-muted)] block mt-0.5">
                        {t('sites.wizard.spaFallbackHelp')}
                      </span>
                    </div>
                  </label>
                )}

                <label className="flex items-start gap-3 p-3 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] cursor-pointer hover:bg-[var(--surface)] transition-colors">
                  <input
                    type="checkbox"
                    checked={forceHttps}
                    onChange={(e) => setForceHttps(e.target.checked)}
                    className="mt-0.5 rounded-[var(--radius-sm)] border-[var(--line)] text-[var(--primary)] focus:ring-[var(--primary)]"
                  />
                  <div>
                    <span className="text-xs font-semibold text-[var(--ink)] block">
                      {t('sites.wizard.forceHttpsLabel')}
                    </span>
                    <span className="text-[11px] text-[var(--ink-muted)] block mt-0.5">
                      {t('sites.wizard.forceHttpsHelp')}
                    </span>
                  </div>
                </label>
              </div>
            </form>
          )}

          {/* STEP 3: Pemilik Tenant */}
          {step === 3 && (
            <div className="space-y-4">
              <div className="text-xs text-[var(--ink-muted)] mb-2">
                {t('sites.wizard.step3Desc')}
              </div>

              {isLoadingTenants ? (
                <div className="p-8 text-center text-xs text-[var(--ink-muted)] flex items-center justify-center gap-2">
                  <Loader2 className="w-4 h-4 animate-spin text-[var(--primary)]" />
                  Memuat daftar tenant...
                </div>
              ) : tenants.length === 0 ? (
                <div className="p-4 rounded-[var(--radius-md)] bg-[var(--danger)]/10 border border-[var(--danger)]/20 text-[var(--danger)] text-xs">
                  Belum ada tenant aktif. Anda harus membuat minimal 1 tenant terlebih dahulu di halaman Tenant.
                </div>
              ) : (
                <div className="space-y-2 max-h-60 overflow-y-auto pr-1">
                  {tenants.map((tItem) => {
                    const isSelected = tItem.id === selectedTenantId;
                    return (
                      <div
                        key={tItem.id}
                        onClick={() => setSelectedTenantId(tItem.id)}
                        className={`cursor-pointer p-3 rounded-[var(--radius-md)] border transition-all flex items-center justify-between ${
                          isSelected
                            ? 'border-[var(--primary)] bg-[var(--primary)]/5 ring-1 ring-[var(--primary)]'
                            : 'border-[var(--line)] bg-[var(--raised)] hover:bg-[var(--surface)]'
                        }`}
                      >
                        <div className="flex items-center gap-3">
                          <div
                            className={`w-7 h-7 rounded-full flex items-center justify-center text-xs font-bold ${
                              isSelected
                                ? 'bg-[var(--primary)] text-white'
                                : 'bg-[var(--line)] text-[var(--ink-muted)]'
                            }`}
                          >
                            {tItem.username.slice(0, 2).toUpperCase()}
                          </div>
                          <div>
                            <div className="text-xs font-semibold text-[var(--ink)]">
                              {tItem.username}
                            </div>
                            <div className="text-[11px] font-mono text-[var(--ink-muted)]">
                              uid: {tItem.linux_uid || '-'} &bull; linux: {tItem.linux_username || `p_${tItem.username}`}
                            </div>
                          </div>
                        </div>

                        {isSelected && (
                          <CheckCircle2 className="w-4 h-4 text-[var(--primary)] shrink-0" />
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}

          {/* STEP 4: Tinjau & Konfirmasi */}
          {step === 4 && (
            <div className="space-y-4">
              <div className="text-xs text-[var(--ink-muted)] mb-3">
                {t('sites.wizard.step4Desc')}
              </div>

              <div className="rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--raised)] divide-y divide-[var(--line)] text-xs">
                <div className="flex items-center justify-between p-3">
                  <span className="text-[var(--ink-muted)]">{t('sites.wizard.reviewDomain')}</span>
                  <span className="font-mono font-semibold text-[var(--ink)]">{domain}</span>
                </div>
                <div className="flex items-center justify-between p-3">
                  <span className="text-[var(--ink-muted)]">{t('sites.wizard.reviewType')}</span>
                  <span className="font-semibold text-[var(--primary)]">
                    {siteType === 'node'
                      ? `Node.js Application (v${nodeVersion})`
                      : siteType === 'php'
                      ? 'PHP (Multi-Versi)'
                      : 'Situs Statis (HTML/CSS/JS)'}
                  </span>
                </div>
                {siteType === 'node' && (
                  <div className="flex items-center justify-between p-3">
                    <span className="text-[var(--ink-muted)]">Entrypoint:</span>
                    <span className="font-mono font-semibold text-[var(--ink)]">
                      {nodeEntrypointType === 'npm_start'
                        ? 'npm start'
                        : `${nodeEntrypointType}: ${nodeEntrypoint}`}
                    </span>
                  </div>
                )}
                <div className="flex items-center justify-between p-3">
                  <span className="text-[var(--ink-muted)]">{t('sites.wizard.reviewTenant')}</span>
                  <span className="font-semibold text-[var(--ink)]">
                    {selectedTenant?.username} ({tenantSlug})
                  </span>
                </div>
                <div className="p-3">
                  <span className="text-[var(--ink-muted)] block mb-1">
                    {t('sites.wizard.reviewDocroot')}
                  </span>
                  <span className="font-mono text-[11px] bg-[var(--bg)] px-2 py-1 rounded block text-[var(--ink)] overflow-x-auto">
                    {docrootPreview}
                  </span>
                </div>
                <div className="flex items-center justify-between p-3">
                  <span className="text-[var(--ink-muted)]">Opsi Routing:</span>
                  <span className="text-[11px] text-[var(--ink)]">
                    {spaFallback ? 'SPA Fallback aktif' : 'SPA standar'} &bull;{' '}
                    {forceHttps ? 'Paksa HTTPS aktif' : 'HTTP/HTTPS fleksibel'}
                  </span>
                </div>
              </div>

              <div className="p-3 rounded-[var(--radius-md)] bg-[var(--info)]/10 border border-[var(--info)]/20 text-xs text-[var(--info)] flex items-start gap-2">
                <ShieldCheck className="w-4 h-4 shrink-0 mt-0.5" />
                <span>
                  Situs akan dibuat dengan vhost Nginx terisolasi, sertifikat self-signed otomatis sebagai fallback, dan izin POSIX ACL www-data yang aman.
                </span>
              </div>
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-[var(--line)] bg-[var(--raised)]">
          {step > 1 ? (
            <button
              type="button"
              onClick={() => {
                setErrorMessage(null);
                setStep((prev) => (prev - 1) as 1 | 2 | 3 | 4);
              }}
              className="px-3.5 py-1.5 text-xs font-semibold rounded-[var(--radius-md)] border border-[var(--line)] text-[var(--ink)] hover:bg-[var(--surface)] transition-colors flex items-center gap-1.5"
            >
              <ArrowLeft className="w-3.5 h-3.5" />
              Kembali
            </button>
          ) : (
            <button
              type="button"
              onClick={onClose}
              className="px-3.5 py-1.5 text-xs font-semibold rounded-[var(--radius-md)] border border-[var(--line)] text-[var(--ink)] hover:bg-[var(--surface)] transition-colors"
            >
              Batal
            </button>
          )}

          {step === 1 && (
            <button
              type="button"
              onClick={handleNextStep1}
              className="px-4 py-1.5 text-xs font-semibold rounded-[var(--radius-md)] bg-[var(--primary)] text-[var(--primary-foreground)] hover:bg-[var(--primary-hover)] transition-colors flex items-center gap-1.5"
            >
              Lanjutkan
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
          )}

          {step === 2 && (
            <button
              type="submit"
              form="step2-form"
              className="px-4 py-1.5 text-xs font-semibold rounded-[var(--radius-md)] bg-[var(--primary)] text-[var(--primary-foreground)] hover:bg-[var(--primary-hover)] transition-colors flex items-center gap-1.5"
            >
              Lanjutkan
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
          )}

          {step === 3 && (
            <button
              type="button"
              disabled={isLoadingTenants || tenants.length === 0}
              onClick={handleNextStep3}
              className="px-4 py-1.5 text-xs font-semibold rounded-[var(--radius-md)] bg-[var(--primary)] text-[var(--primary-foreground)] hover:bg-[var(--primary-hover)] disabled:opacity-50 transition-colors flex items-center gap-1.5"
            >
              Lanjutkan
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
          )}

          {step === 4 && (
            <button
              type="button"
              disabled={isSubmitting}
              onClick={handleFinalSubmit}
              className="px-4 py-1.5 text-xs font-semibold rounded-[var(--radius-md)] bg-[var(--primary)] text-[var(--primary-foreground)] hover:bg-[var(--primary-hover)] disabled:opacity-50 transition-colors flex items-center gap-1.5"
            >
              {isSubmitting && <Loader2 className="w-3.5 h-3.5 animate-spin" />}
              {isSubmitting ? 'Menerapkan ke Nginx...' : t('sites.wizard.submitButton')}
            </button>
          )}
        </div>
      </div>
    </div>
  );
};
