import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  LayoutDashboard,
  Globe,
  Users,
  BarChart3,
  Cpu,
  ListTodo,
  ShieldCheck,
  Settings,
  ChevronDown,
  Search,
  Bell,
  Sun,
  Moon,
  Monitor,
  Menu,
  Server,
} from 'lucide-react';
import { ThemeMode } from '../../types/api';

export interface ShellProps {
  children: React.ReactNode;
  activeNav?: string;
  onNavigate?: (nav: string) => void;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
}

export const Shell: React.FC<ShellProps> = ({
  children,
  activeNav = 'dashboard',
  onNavigate = () => {},
  theme,
  onThemeChange,
}) => {
  const { t, i18n } = useTranslation();
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);

  const navItems = [
    { id: 'dashboard', label: t('nav.dashboard'), icon: LayoutDashboard },
    { id: 'sites', label: t('nav.sites'), icon: Globe },
    { id: 'users', label: t('nav.users'), icon: Users },
    { id: 'analytics', label: t('nav.analytics'), icon: BarChart3 },
    { id: 'runtime', label: t('nav.runtime'), icon: Cpu },
    { id: 'jobs', label: t('nav.jobs'), icon: ListTodo },
    { id: 'audit', label: t('nav.audit'), icon: ShieldCheck },
    { id: 'settings', label: t('nav.settings'), icon: Settings },
  ];

  const cycleTheme = () => {
    if (theme === 'light') onThemeChange('dark');
    else if (theme === 'dark') onThemeChange('system');
    else onThemeChange('light');
  };

  const toggleLanguage = () => {
    const nextLang = i18n.language === 'id' ? 'en' : 'id';
    i18n.changeLanguage(nextLang);
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[var(--bg)] text-[var(--ink)]">
      {/* Sidebar Navigation */}
      <aside
        className={`flex flex-col border-r border-[var(--line)] bg-[var(--surface)] transition-all duration-200 z-20 ${
          isSidebarCollapsed ? 'w-16' : 'w-56'
        }`}
      >
        {/* Brand / Logo */}
        <div className="flex items-center justify-between h-14 px-4 border-b border-[var(--line)]">
          {!isSidebarCollapsed && (
            <div className="flex items-center gap-2">
              <div className="w-6 h-6 rounded-[var(--radius-sm)] bg-[var(--primary)] flex items-center justify-center text-white font-bold text-xs tracking-wider">
                N
              </div>
              <span className="font-semibold text-sm tracking-tight text-[var(--ink)]">
                {t('common.appName')}
              </span>
            </div>
          )}
          {isSidebarCollapsed && (
            <div className="w-6 h-6 mx-auto rounded-[var(--radius-sm)] bg-[var(--primary)] flex items-center justify-center text-white font-bold text-xs">
              N
            </div>
          )}
          <button
            onClick={() => setIsSidebarCollapsed(!isSidebarCollapsed)}
            className="p-1 rounded-[var(--radius-sm)] text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--raised)]"
            title="Toggle Sidebar"
          >
            <Menu className="w-4 h-4" />
          </button>
        </div>

        {/* Navigation Items */}
        <nav className="flex-1 p-2 space-y-0.5 overflow-y-auto">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeNav === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onNavigate(item.id)}
                className={`w-full flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs font-medium transition-colors ${
                  isActive
                    ? 'bg-[var(--primary)]/10 text-[var(--primary)] font-semibold'
                    : 'text-[var(--ink-muted)] hover:bg-[var(--raised)] hover:text-[var(--ink)]'
                }`}
                title={item.label}
              >
                <Icon className={`w-4 h-4 shrink-0 ${isActive ? 'text-[var(--primary)]' : ''}`} />
                {!isSidebarCollapsed && <span>{item.label}</span>}
              </button>
            );
          })}
        </nav>

        {/* Sidebar Footer */}
        <div className="p-3 border-t border-[var(--line)] flex items-center justify-between text-xs text-[var(--ink-muted)]">
          {!isSidebarCollapsed && (
            <span className="text-[11px] font-mono opacity-70">v0.1.0-alpha</span>
          )}
          <div className="flex items-center gap-1 mx-auto">
            <button
              onClick={cycleTheme}
              className="p-1.5 rounded-[var(--radius-sm)] hover:bg-[var(--raised)] hover:text-[var(--ink)]"
              title={`Tema: ${theme}`}
            >
              {theme === 'light' && <Sun className="w-3.5 h-3.5" />}
              {theme === 'dark' && <Moon className="w-3.5 h-3.5" />}
              {theme === 'system' && <Monitor className="w-3.5 h-3.5" />}
            </button>
            <button
              onClick={toggleLanguage}
              className="px-1.5 py-0.5 rounded-[var(--radius-sm)] hover:bg-[var(--raised)] hover:text-[var(--ink)] font-mono text-[11px]"
              title="Ganti Bahasa (ID / EN)"
            >
              {i18n.language.toUpperCase()}
            </button>
          </div>
        </div>
      </aside>

      {/* Main Container */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {/* Topbar Header */}
        <header className="h-14 border-b border-[var(--line)] bg-[var(--surface)] px-6 flex items-center justify-between gap-4 z-10">
          {/* Server Selector Badge */}
          <div className="flex items-center gap-2">
            <button className="flex items-center gap-2 px-2.5 py-1 text-xs font-medium border border-[var(--line)] rounded-[var(--radius-sm)] bg-[var(--raised)] text-[var(--ink)] hover:border-[var(--primary)]/40 transition-colors">
              <Server className="w-3.5 h-3.5 text-[var(--primary)]" />
              <span>vps-01</span>
              <ChevronDown className="w-3 h-3 text-[var(--ink-muted)]" />
            </button>
          </div>

          {/* Quick Search & Commands Trigger */}
          <div className="flex-1 max-w-md">
            <div className="relative">
              <Search className="w-3.5 h-3.5 absolute left-3 top-1/2 -translate-y-1/2 text-[var(--ink-muted)]" />
              <input
                type="text"
                readOnly
                placeholder={t('common.search')}
                className="w-full bg-[var(--raised)] border border-[var(--line)] rounded-[var(--radius-sm)] pl-9 pr-14 py-1.5 text-xs text-[var(--ink)] placeholder-[var(--ink-muted)] focus:outline-none cursor-pointer"
              />
              <kbd className="absolute right-2.5 top-1/2 -translate-y-1/2 text-[10px] font-mono px-1.5 py-0.5 border border-[var(--line)] rounded bg-[var(--surface)] text-[var(--ink-muted)]">
                Ctrl+K
              </kbd>
            </div>
          </div>

          {/* User & Notifications */}
          <div className="flex items-center gap-3">
            <button
              className="p-1.5 rounded-[var(--radius-sm)] text-[var(--ink-muted)] hover:bg-[var(--raised)] hover:text-[var(--ink)] relative"
              title="Notifikasi"
            >
              <Bell className="w-4 h-4" />
              <span className="absolute top-1 right-1 w-1.5 h-1.5 bg-[var(--primary)] rounded-full" />
            </button>

            <div className="flex items-center gap-2 pl-2 border-l border-[var(--line)]">
              <div className="w-7 h-7 rounded-[var(--radius-full)] bg-[var(--primary)] text-[var(--primary-foreground)] font-semibold text-xs flex items-center justify-center">
                RA
              </div>
              <span className="text-xs font-medium hidden sm:inline text-[var(--ink)]">
                admin
              </span>
            </div>
          </div>
        </header>

        {/* Content Area */}
        <main className="flex-1 overflow-y-auto p-6">
          <div className="max-w-[1280px] w-full text-left mx-0">
            {children}
          </div>
        </main>

        {/* Job Tray Floating Dock (Section 8.3 & 8.5) */}
        <div className="fixed bottom-4 right-6 z-30">
          <div className="flex items-center gap-2 px-3 py-1.5 bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-full)] shadow-[var(--shadow-floating)] text-xs text-[var(--ink-muted)]">
            <span className="w-2 h-2 rounded-full bg-[var(--success)] animate-pulse" />
            <span className="font-medium text-[var(--ink)]">Tugas (0 aktif)</span>
          </div>
        </div>
      </div>
    </div>
  );
};
