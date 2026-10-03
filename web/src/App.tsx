import React, { useEffect, useState } from 'react';
import { Shell } from './components/layout/Shell';
import { DashboardPage } from './pages/DashboardPage';
import { LoginPage } from './pages/LoginPage';
import { FirstRunWizardPage } from './pages/FirstRunWizardPage';
import { SecurityProfilePage } from './pages/SecurityProfilePage';
import { AuditLogPage } from './pages/AuditLogPage';
import { JobsPage } from './pages/JobsPage';
import { TenantsPage } from './pages/TenantsPage';
import { TenantDetailPage } from './pages/TenantDetailPage';
import { PlansPage } from './pages/PlansPage';
import { ToastProvider } from './components/ui/Toast';
import { ThemeMode } from './types/api';
import { AuthProvider, useAuth } from './context/AuthContext';
import './i18n';

const MainApp: React.FC = () => {
  const { isAuthenticated, isLoading } = useAuth();
  const [theme, setTheme] = useState<ThemeMode>(() => {
    return (localStorage.getItem('nusa_theme') as ThemeMode) || 'system';
  });
  const [activeNav, setActiveNav] = useState('dashboard');
  const [selectedTenantId, setSelectedTenantId] = useState<number | null>(null);
  const [isWizardOpen, setIsWizardOpen] = useState(false);

  useEffect(() => {
    localStorage.setItem('nusa_theme', theme);

    const root = document.documentElement;
    const isDark =
      theme === 'dark' ||
      (theme === 'system' && window.matchMedia('(prefers-color-scheme: dark)').matches);

    if (isDark) {
      root.classList.add('dark');
    } else {
      root.classList.remove('dark');
    }

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleSystemChange = (e: MediaQueryListEvent) => {
      if (theme === 'system') {
        if (e.matches) root.classList.add('dark');
        else root.classList.remove('dark');
      }
    };
    mediaQuery.addEventListener('change', handleSystemChange);
    return () => mediaQuery.removeEventListener('change', handleSystemChange);
  }, [theme]);

  // Loading skeleton while verifying session
  if (isLoading) {
    return (
      <div className="min-h-screen w-screen flex flex-col items-center justify-center p-4 bg-[var(--bg)] text-[var(--ink)]">
        <div className="w-10 h-10 rounded-[var(--radius-md)] bg-[var(--primary)] text-white font-bold flex items-center justify-center animate-pulse text-lg mb-4">
          N
        </div>
        <div className="text-xs text-[var(--ink-muted)]">
          Memeriksa sesi pengguna...
        </div>
      </div>
    );
  }

  // First-run wizard view
  if (isWizardOpen) {
    return (
      <FirstRunWizardPage
        onFinish={() => setIsWizardOpen(false)}
        onBackToLogin={() => setIsWizardOpen(false)}
      />
    );
  }

  // Not authenticated: render Login Page
  if (!isAuthenticated) {
    return <LoginPage onOpenWizard={() => setIsWizardOpen(true)} />;
  }

  // Authenticated: render Shell and active page
  return (
    <>
      <Shell
        activeNav={activeNav === 'tenant-detail' ? 'tenants' : activeNav}
        onNavigate={(nav) => {
          if (nav === 'tenants' || nav === 'users') {
            setSelectedTenantId(null);
          }
          setActiveNav(nav);
        }}
        theme={theme}
        onThemeChange={setTheme}
      >
        {activeNav === 'dashboard' && <DashboardPage />}
        {activeNav === 'audit' && <AuditLogPage />}
        {activeNav === 'jobs' && <JobsPage />}
        {activeNav === 'profile' && <SecurityProfilePage />}
        {(activeNav === 'tenants' || activeNav === 'users') && (
          <TenantsPage
            onSelectTenant={(id) => {
              setSelectedTenantId(id);
              setActiveNav('tenant-detail');
            }}
            onNavigateToPlans={() => setActiveNav('plans')}
          />
        )}
        {activeNav === 'tenant-detail' && selectedTenantId && (
          <TenantDetailPage
            tenantId={selectedTenantId}
            onBack={() => {
              setSelectedTenantId(null);
              setActiveNav('tenants');
            }}
          />
        )}
        {activeNav === 'plans' && <PlansPage />}
        {['sites', 'analytics', 'runtime', 'settings'].includes(activeNav) && (
          <div className="p-8 text-center text-xs text-[var(--ink-muted)] border border-[var(--line)] rounded-[var(--radius-md)] bg-[var(--surface)]">
            Halaman <span className="font-semibold text-[var(--ink)]">{activeNav}</span> akan diimplementasikan pada fase berikutnya sesuai roadmap PRD.
          </div>
        )}
      </Shell>
      <ToastProvider />
    </>
  );
};

export const App: React.FC = () => {
  return (
    <AuthProvider>
      <MainApp />
    </AuthProvider>
  );
};

export default App;
