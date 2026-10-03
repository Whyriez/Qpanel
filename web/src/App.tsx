import React, { useEffect, useState } from 'react';
import { Shell } from './components/layout/Shell';
import { DashboardPage } from './pages/DashboardPage';
import { ToastProvider } from './components/ui/Toast';
import { ThemeMode } from './types/api';
import './i18n';

export const App: React.FC = () => {
  const [theme, setTheme] = useState<ThemeMode>(() => {
    return (localStorage.getItem('nusa_theme') as ThemeMode) || 'system';
  });
  const [activeNav, setActiveNav] = useState('dashboard');

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

    // Listener for system theme change
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

  return (
    <>
      <Shell
        activeNav={activeNav}
        onNavigate={setActiveNav}
        theme={theme}
        onThemeChange={setTheme}
      >
        {activeNav === 'dashboard' && <DashboardPage />}
        {activeNav !== 'dashboard' && (
          <div className="p-8 text-center text-xs text-[var(--ink-muted)] border border-[var(--line)] rounded-[var(--radius-md)] bg-[var(--surface)]">
            Halaman <span className="font-semibold text-[var(--ink)]">{activeNav}</span> akan diimplementasikan pada fase berikutnya sesuai PRD.
          </div>
        )}
      </Shell>
      <ToastProvider />
    </>
  );
};

export default App;
