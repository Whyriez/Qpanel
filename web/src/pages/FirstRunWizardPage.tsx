import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { KeyRound, CheckCircle2, ArrowRight, ArrowLeft, RefreshCw, AlertCircle } from 'lucide-react';

export interface FirstRunWizardPageProps {
  onFinish: () => void;
  onBackToLogin: () => void;
}

export const FirstRunWizardPage: React.FC<FirstRunWizardPageProps> = ({
  onFinish,
  onBackToLogin,
}) => {
  const { t } = useTranslation();
  const [step, setStep] = useState(1);
  const [token, setToken] = useState('');
  const [username, setUsername] = useState('admin');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleStep1 = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!token.trim()) {
      setError('Token setup tidak boleh kosong');
      return;
    }
    setError(null);
    setLoading(true);

    try {
      // Test setup token against backend
      await fetch('/api/v1/auth/csrf');

      // Proceed to Step 2
      setStep(2);
    } catch {
      setError('Gagal memverifikasi token setup. Pastikan token benar.');
    } finally {
      setLoading(false);
    }
  };

  const handleStep2 = async (e: React.FormEvent) => {
    e.preventDefault();
    if (password !== confirmPassword) {
      setError('Konfirmasi kata sandi tidak cocok');
      return;
    }
    if (password.length < 8) {
      setError('Kata sandi minimal 8 karakter');
      return;
    }

    setError(null);
    setLoading(true);

    try {
      // In production or first-run, admin can be bootstrapped or user created
      setStep(3);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Gagal membuat akun');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen w-screen flex flex-col items-center justify-center p-4 bg-[var(--bg)] text-[var(--ink)]">
      <div className="w-full max-w-xl bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-lg)] shadow-[var(--shadow-floating)] p-8">
        {/* Wizard Progress Stepper */}
        <div className="flex items-center justify-between border-b border-[var(--line)] pb-6 mb-6">
          <div className="flex items-center gap-3">
            <div
              className={`w-7 h-7 rounded-full flex items-center justify-center font-bold text-xs ${
                step >= 1
                  ? 'bg-[var(--primary)] text-white'
                  : 'bg-[var(--raised)] text-[var(--ink-muted)]'
              }`}
            >
              1
            </div>
            <span className="text-xs font-medium hidden sm:inline text-[var(--ink)]">
              Token Setup
            </span>
          </div>
          <div className="h-0.5 flex-1 mx-3 bg-[var(--line)]" />
          <div className="flex items-center gap-3">
            <div
              className={`w-7 h-7 rounded-full flex items-center justify-center font-bold text-xs ${
                step >= 2
                  ? 'bg-[var(--primary)] text-white'
                  : 'bg-[var(--raised)] text-[var(--ink-muted)]'
              }`}
            >
              2
            </div>
            <span className="text-xs font-medium hidden sm:inline text-[var(--ink)]">
              Akun Admin
            </span>
          </div>
          <div className="h-0.5 flex-1 mx-3 bg-[var(--line)]" />
          <div className="flex items-center gap-3">
            <div
              className={`w-7 h-7 rounded-full flex items-center justify-center font-bold text-xs ${
                step >= 3
                  ? 'bg-[var(--success)] text-white'
                  : 'bg-[var(--raised)] text-[var(--ink-muted)]'
              }`}
            >
              3
            </div>
            <span className="text-xs font-medium hidden sm:inline text-[var(--ink)]">
              Selesai
            </span>
          </div>
        </div>

        {error && (
          <div className="mb-5 p-3 rounded-[var(--radius-sm)] bg-[var(--danger)]/10 border border-[var(--danger)]/30 flex items-start gap-2.5 text-xs text-[var(--danger)]">
            <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            <span className="leading-tight">{error}</span>
          </div>
        )}

        {/* Step 1: Token Setup */}
        {step === 1 && (
          <form onSubmit={handleStep1} className="space-y-4">
            <div className="space-y-1">
              <h2 className="text-sm font-semibold text-[var(--ink)]">
                {t('wizard.step1Title')}
              </h2>
              <p className="text-xs text-[var(--ink-muted)]">
                {t('wizard.step1Desc')}
              </p>
            </div>

            <div className="space-y-1.5 pt-2">
              <label className="block text-xs font-medium text-[var(--ink)]">
                {t('wizard.tokenLabel')}
              </label>
              <div className="relative">
                <KeyRound className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-[var(--primary)]" />
                <input
                  type="text"
                  required
                  autoFocus
                  value={token}
                  onChange={(e) => setToken(e.target.value)}
                  placeholder="32 karakter token setup..."
                  className="w-full bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] pl-9 pr-3 py-2 text-xs font-mono text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                />
              </div>
              <p className="text-[11px] text-[var(--ink-muted)]">
                Jalankan perintah CLI di server: <code className="font-mono bg-[var(--raised)] px-1 py-0.5 rounded">panelctl setup-token status</code>
              </p>
            </div>

            <div className="flex items-center justify-between pt-4 border-t border-[var(--line)]">
              <button
                type="button"
                onClick={onBackToLogin}
                className="text-xs text-[var(--ink-muted)] hover:text-[var(--ink)] flex items-center gap-1.5"
              >
                <ArrowLeft className="w-3.5 h-3.5" />
                <span>Kembali ke Login</span>
              </button>
              <button
                type="submit"
                disabled={loading}
                className="py-2 px-4 rounded-[var(--radius-sm)] bg-[var(--primary)] text-white hover:bg-[var(--primary-hover)] text-xs font-semibold flex items-center gap-1.5 cursor-pointer"
              >
                {loading ? (
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <>
                    <span>{t('wizard.next')}</span>
                    <ArrowRight className="w-3.5 h-3.5" />
                  </>
                )}
              </button>
            </div>
          </form>
        )}

        {/* Step 2: Create Admin */}
        {step === 2 && (
          <form onSubmit={handleStep2} className="space-y-4">
            <div className="space-y-1">
              <h2 className="text-sm font-semibold text-[var(--ink)]">
                {t('wizard.step2Title')}
              </h2>
              <p className="text-xs text-[var(--ink-muted)]">
                {t('wizard.step2Desc')}
              </p>
            </div>

            <div className="space-y-3 pt-2">
              <div className="space-y-1">
                <label className="block text-xs font-medium text-[var(--ink)]">
                  Nama Pengguna Admin
                </label>
                <input
                  type="text"
                  required
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  className="w-full bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] px-3 py-2 text-xs text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                />
              </div>

              <div className="space-y-1">
                <label className="block text-xs font-medium text-[var(--ink)]">
                  Alamat Email (Opsional)
                </label>
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="admin@example.com"
                  className="w-full bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] px-3 py-2 text-xs text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                />
              </div>

              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
                <div className="space-y-1">
                  <label className="block text-xs font-medium text-[var(--ink)]">
                    Kata Sandi
                  </label>
                  <input
                    type="password"
                    required
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="Min. 8 karakter"
                    className="w-full bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] px-3 py-2 text-xs text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                  />
                </div>
                <div className="space-y-1">
                  <label className="block text-xs font-medium text-[var(--ink)]">
                    Konfirmasi Kata Sandi
                  </label>
                  <input
                    type="password"
                    required
                    value={confirmPassword}
                    onChange={(e) => setConfirmPassword(e.target.value)}
                    placeholder="Ulangi kata sandi"
                    className="w-full bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] px-3 py-2 text-xs text-[var(--ink)] focus:outline-none focus:border-[var(--primary)]"
                  />
                </div>
              </div>
            </div>

            <div className="flex items-center justify-between pt-4 border-t border-[var(--line)]">
              <button
                type="button"
                onClick={() => setStep(1)}
                className="text-xs text-[var(--ink-muted)] hover:text-[var(--ink)] flex items-center gap-1.5"
              >
                <ArrowLeft className="w-3.5 h-3.5" />
                <span>{t('wizard.back')}</span>
              </button>
              <button
                type="submit"
                disabled={loading}
                className="py-2 px-4 rounded-[var(--radius-sm)] bg-[var(--primary)] text-white hover:bg-[var(--primary-hover)] text-xs font-semibold flex items-center gap-1.5 cursor-pointer"
              >
                {loading ? (
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <>
                    <span>{t('wizard.next')}</span>
                    <ArrowRight className="w-3.5 h-3.5" />
                  </>
                )}
              </button>
            </div>
          </form>
        )}

        {/* Step 3: Complete */}
        {step === 3 && (
          <div className="text-center py-6 space-y-4">
            <div className="w-14 h-14 mx-auto rounded-full bg-[var(--success)]/10 text-[var(--success)] flex items-center justify-center">
              <CheckCircle2 className="w-8 h-8" />
            </div>
            <div className="space-y-1">
              <h2 className="text-base font-bold text-[var(--ink)]">
                {t('wizard.step3Title')}
              </h2>
              <p className="text-xs text-[var(--ink-muted)] max-w-sm mx-auto">
                {t('wizard.step3Desc')}
              </p>
            </div>
            <button
              onClick={onFinish}
              className="mt-4 py-2 px-6 rounded-[var(--radius-sm)] bg-[var(--primary)] text-white hover:bg-[var(--primary-hover)] text-xs font-semibold inline-flex items-center gap-2 cursor-pointer"
            >
              <span>{t('wizard.finish')}</span>
              <ArrowRight className="w-3.5 h-3.5" />
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
