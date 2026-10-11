import React, { useEffect, useState, Suspense, lazy } from 'react';
import { Shell } from './components/layout/Shell';
import { DashboardPage } from './pages/DashboardPage';
import { LoginPage } from './pages/LoginPage';
import { ToastProvider } from './components/ui/Toast';
import { ThemeMode } from './types/api';
import { AuthProvider, useAuth } from './context/AuthContext';
import './i18n';

const FirstRunWizardPage = lazy(() =>
  import('./pages/FirstRunWizardPage').then((m) => ({ default: m.FirstRunWizardPage }))
);
const SecurityProfilePage = lazy(() =>
  import('./pages/SecurityProfilePage').then((m) => ({ default: m.SecurityProfilePage }))
);
const AuditLogPage = lazy(() =>
  import('./pages/AuditLogPage').then((m) => ({ default: m.AuditLogPage }))
);
const JobsPage = lazy(() =>
  import('./pages/JobsPage').then((m) => ({ default: m.JobsPage }))
);
const TenantsPage = lazy(() =>
  import('./pages/TenantsPage').then((m) => ({ default: m.TenantsPage }))
);
const TenantDetailPage = lazy(() =>
  import('./pages/TenantDetailPage').then((m) => ({ default: m.TenantDetailPage }))
);
const PlansPage = lazy(() =>
  import('./pages/PlansPage').then((m) => ({ default: m.PlansPage }))
);
const SitesPage = lazy(() =>
  import('./pages/SitesPage').then((m) => ({ default: m.SitesPage }))
);
const SiteDetailPage = lazy(() =>
  import('./pages/SiteDetailPage').then((m) => ({ default: m.SiteDetailPage }))
);
const RuntimePage = lazy(() =>
  import('./pages/RuntimePage').then((m) => ({ default: m.RuntimePage }))
);
const FileManagerPage = lazy(() =>
  import('./pages/FileManagerPage').then((m) => ({ default: m.FileManagerPage }))
);
const TerminalPage = lazy(() =>
  import('./pages/TerminalPage').then((m) => ({ default: m.TerminalPage }))
);
const AnalyticsPage = lazy(() =>
  import('./pages/AnalyticsPage').then((m) => ({ default: m.AnalyticsPage }))
);
const SettingsPage = lazy(() =>
  import('./pages/SettingsPage').then((m) => ({ default: m.SettingsPage }))
);

const PageFallback: React.FC = () => (
  <div className="p-8 flex flex-col items-center justify-center space-y-2 text-xs text-[var(--ink-muted)]">
    <div className="w-5 h-5 rounded-full border-2 border-[var(--primary)] border-t-transparent animate-spin" />
    <span>Memuat modul halaman...</span>
  </div>
);

const MainApp: React.FC = () => {
  const { isAuthenticated, isLoading } = useAuth();
  const [theme, setTheme] = useState<ThemeMode>(() => {
    return (localStorage.getItem('nusa_theme') as ThemeMode) || 'system';
  });
  const [activeNav, setActiveNav] = useState('dashboard');
  const [selectedTenantId, setSelectedTenantId] = useState<number | null>(null);
  const [selectedSiteId, setSelectedSiteId] = useState<number | null>(null);
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
      <Suspense fallback={<PageFallback />}>
        <FirstRunWizardPage
          onFinish={() => setIsWizardOpen(false)}
          onBackToLogin={() => setIsWizardOpen(false)}
        />
      </Suspense>
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
        activeNav={
          activeNav === 'tenant-detail'
            ? 'tenants'
            : activeNav === 'site-detail'
            ? 'sites'
            : activeNav
        }
        onNavigate={(nav) => {
          if (nav === 'tenants' || nav === 'users') {
            setSelectedTenantId(null);
          }
          if (nav === 'sites') {
            setSelectedSiteId(null);
          }
          setActiveNav(nav);
        }}
        theme={theme}
        onThemeChange={setTheme}
      >
        {activeNav === 'dashboard' && <DashboardPage />}
        <Suspense fallback={<PageFallback />}>
          {activeNav === 'audit' && <AuditLogPage />}
          {activeNav === 'jobs' && <JobsPage />}
          {activeNav === 'profile' && <SecurityProfilePage />}
          {activeNav === 'sites' && (
            <SitesPage
              onSelectSite={(id) => {
                setSelectedSiteId(id);
                setActiveNav('site-detail');
              }}
            />
          )}
          {activeNav === 'site-detail' && selectedSiteId && (
            <SiteDetailPage
              siteId={selectedSiteId}
              onBack={() => {
                setSelectedSiteId(null);
                setActiveNav('sites');
              }}
            />
          )}
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
          {activeNav === 'runtime' && <RuntimePage />}
          {activeNav === 'files' && <FileManagerPage />}
          {activeNav === 'terminal' && <TerminalPage />}
          {activeNav === 'analytics' && <AnalyticsPage />}
          {activeNav === 'settings' && <SettingsPage />}
        </Suspense>
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
