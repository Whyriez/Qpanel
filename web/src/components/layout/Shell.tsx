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
  UserCheck,
  Settings,
  ChevronDown,
  Search,
  Sun,
  Moon,
  Monitor,
  Menu,
  Server,
  LogOut,
  Layers,
  FolderTree,
  Terminal,
} from 'lucide-react';
import { ThemeMode } from '../../types/api';
import { useAuth } from '../../context/AuthContext';
import { JobTray } from './JobTray';
import { CommandPalette } from '../ui/CommandPalette';
import { ImpersonationBanner } from './ImpersonationBanner';
import { NotificationBell } from '../notifications/NotificationBell';

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
  const { user, logout } = useAuth();
  const [isSidebarCollapsed, setIsSidebarCollapsed] = useState(false);
  const [isCommandPaletteOpen, setIsCommandPaletteOpen] = useState(false);

  const navItems = [
    { id: 'dashboard', label: t('nav.dashboard'), icon: LayoutDashboard },
    { id: 'sites', label: t('nav.sites'), icon: Globe },
    { id: 'files', label: t('nav.files'), icon: FolderTree },
    { id: 'terminal', label: t('nav.terminal'), icon: Terminal },
    { id: 'tenants', label: t('nav.tenants'), icon: Users },
    { id: 'plans', label: t('nav.plans'), icon: Layers },
    { id: 'analytics', label: t('nav.analytics'), icon: BarChart3 },
    { id: 'runtime', label: t('nav.runtime'), icon: Cpu },
    { id: 'jobs', label: t('nav.jobs'), icon: ListTodo },
    { id: 'audit', label: t('nav.audit'), icon: ShieldCheck },
    { id: 'profile', label: t('nav.profile'), icon: UserCheck },
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

  const userInitial = user?.username ? user.username.slice(0, 2).toUpperCase() : 'NP';

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
            className="p-1 rounded-[var(--radius-sm)] text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer"
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
                className={`w-full flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs font-medium transition-colors cursor-pointer ${
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
              className="p-1.5 rounded-[var(--radius-sm)] hover:bg-[var(--raised)] hover:text-[var(--ink)] cursor-pointer"
              title={`Tema: ${theme}`}
            >
              {theme === 'light' && <Sun className="w-3.5 h-3.5" />}
              {theme === 'dark' && <Moon className="w-3.5 h-3.5" />}
              {theme === 'system' && <Monitor className="w-3.5 h-3.5" />}
            </button>
            <button
              onClick={toggleLanguage}
              className="px-1.5 py-0.5 rounded-[var(--radius-sm)] hover:bg-[var(--raised)] hover:text-[var(--ink)] font-mono text-[11px] cursor-pointer"
              title="Ganti Bahasa (ID / EN)"
            >
              {i18n.language.toUpperCase()}
            </button>
          </div>
        </div>
      </aside>

      {/* Main Container */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {/* Impersonation Banner */}
        {user?.impersonated_by && (
          <ImpersonationBanner
            impersonatedBy={user.impersonated_by}
            username={user.username}
          />
        )}

        {/* Topbar Header */}
        <header className="h-14 border-b border-[var(--line)] bg-[var(--surface)] px-6 flex items-center justify-between gap-4 z-10">
          {/* Server Selector Badge */}
          <div className="flex items-center gap-2">
            <button className="flex items-center gap-2 px-2.5 py-1 text-xs font-medium border border-[var(--line)] rounded-[var(--radius-sm)] bg-[var(--raised)] text-[var(--ink)] hover:border-[var(--primary)]/40 transition-colors cursor-pointer">
              <Server className="w-3.5 h-3.5 text-[var(--primary)]" />
              <span>vps-01</span>
              <ChevronDown className="w-3 h-3 text-[var(--ink-muted)]" />
            </button>
          </div>

          {/* Quick Search & Commands Trigger */}
          <div className="flex-1 max-w-md">
            <div
              className="relative cursor-pointer"
              onClick={() => setIsCommandPaletteOpen(true)}
            >
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

          {/* User & Actions */}
          <div className="flex items-center gap-3">
            <NotificationBell
              userRole={user?.role}
              onNavigateUrl={(url) => {
                const segment = url.replace(/^\//, '').split('/')[0];
                if (segment) onNavigate(segment);
              }}
            />

            {/* Profile trigger */}
            <div
              onClick={() => onNavigate('profile')}
              className="flex items-center gap-2 pl-2 border-l border-[var(--line)] cursor-pointer hover:opacity-80 transition-opacity"
              title="Profil & Keamanan"
            >
              <div className="w-7 h-7 rounded-[var(--radius-full)] bg-[var(--primary)] text-white font-semibold text-xs flex items-center justify-center">
                {userInitial}
              </div>
              <span className="text-xs font-medium hidden sm:inline text-[var(--ink)]">
                {user?.username || 'admin'}
              </span>
            </div>

            <button
              onClick={logout}
              className="p-1.5 rounded-[var(--radius-sm)] text-[var(--danger)] hover:bg-[var(--danger)]/10 cursor-pointer"
              title={t('auth.logout')}
            >
              <LogOut className="w-4 h-4" />
            </button>
          </div>
        </header>

        {/* Content Area */}
        <main className="flex-1 overflow-y-auto p-6">
          <div className="max-w-[1280px] w-full text-left mx-0">
            {children}
          </div>
        </main>

        {/* Floating Job Tray */}
        <JobTray onNavigateToJobs={() => onNavigate('jobs')} />

        {/* Command Palette Modal */}
        <CommandPalette
          isOpen={isCommandPaletteOpen}
          onClose={() => setIsCommandPaletteOpen(false)}
          onNavigate={onNavigate}
          theme={theme}
          onThemeChange={onThemeChange}
        />
      </div>
    </div>
  );
};
