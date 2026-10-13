import React, { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useTranslation } from "react-i18next";
import {
  LayoutDashboard,
  Globe,
  ListTodo,
  ShieldCheck,
  UserCheck,
  Settings,
  Sun,
  Moon,
  Monitor,
  Languages,
  LogOut,
  FolderTree,
  Terminal,
} from "lucide-react";
import { ThemeMode } from "../../types/api";
import { useAuth } from "../../context/AuthContext";

export interface CommandPaletteProps {
  isOpen?: boolean;
  onClose?: () => void;
  onOpen?: () => void;
  onNavigate: (nav: string) => void;
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
}

export const CommandPalette: React.FC<CommandPaletteProps> = ({
  isOpen: controlledOpen,
  onClose,
  onOpen,
  onNavigate,
  theme,
  onThemeChange,
}) => {
  const { t, i18n } = useTranslation();
  const { logout, user } = useAuth();
  const [open, setOpen] = useState(false);

  const isControlled = controlledOpen !== undefined;
  const isVisible = isControlled ? controlledOpen : open;

  const handleClose = () => {
    if (onClose) onClose();
    if (!isControlled) setOpen(false);
  };

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (isControlled) {
          if (isVisible && onClose) onClose();
          else if (!isVisible && onOpen) onOpen();
        } else {
          setOpen((prev) => !prev);
        }
      }
      if (e.key === "Escape" && isVisible) {
        handleClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isVisible, isControlled, onClose, onOpen]);

  if (!isVisible) return null;

  const handleSelectNav = (page: string) => {
    onNavigate(page);
    handleClose();
  };

  const handleToggleLang = () => {
    const nextLang = i18n.language === "id" ? "en" : "id";
    i18n.changeLanguage(nextLang);
    handleClose();
  };

  const handleLogout = async () => {
    handleClose();
    await logout();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center pt-24 px-4 bg-black/50 backdrop-blur-sm transition-opacity"
      onClick={handleClose}
    >
      <div
        className="w-full max-w-xl bg-[var(--surface)] border border-[var(--line)] rounded-[var(--radius-lg)] shadow-[var(--shadow-floating)] overflow-hidden"
        onClick={(e) => e.stopPropagation()}
      >
        <Command label="Command Palette" className="flex flex-col">
          <div className="flex items-center px-4 border-b border-[var(--line)]">
            <Command.Input
              autoFocus
              placeholder={t("common.search")}
              className="w-full h-12 bg-transparent text-sm text-[var(--ink)] placeholder-[var(--ink-muted)] outline-none"
            />
            <kbd className="text-[10px] font-mono px-1.5 py-0.5 border border-[var(--line)] rounded bg-[var(--raised)] text-[var(--ink-muted)]">
              ESC
            </kbd>
          </div>

          <Command.List className="max-h-80 overflow-y-auto p-2 space-y-1">
            <Command.Empty className="py-6 text-center text-xs text-[var(--ink-muted)]">
              Tidak ada hasil yang cocok.
            </Command.Empty>

            <Command.Group
              heading="Navigasi"
              className="text-[11px] font-semibold text-[var(--ink-muted)] uppercase tracking-wider px-2 py-1"
            >
              <Command.Item
                onSelect={() => handleSelectNav("dashboard")}
                className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer aria-selected:bg-[var(--primary)]/10 aria-selected:text-[var(--primary)]"
              >
                <LayoutDashboard className="w-4 h-4 text-[var(--ink-muted)]" />
                <span>{t("nav.dashboard")}</span>
              </Command.Item>
              <Command.Item
                onSelect={() => handleSelectNav("sites")}
                className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer aria-selected:bg-[var(--primary)]/10 aria-selected:text-[var(--primary)]"
              >
                <Globe className="w-4 h-4 text-[var(--ink-muted)]" />
                <span>{t("nav.sites")}</span>
              </Command.Item>
              <Command.Item
                onSelect={() => handleSelectNav("files")}
                className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer aria-selected:bg-[var(--primary)]/10 aria-selected:text-[var(--primary)]"
              >
                <FolderTree className="w-4 h-4 text-[var(--ink-muted)]" />
                <span>{t("nav.files")}</span>
              </Command.Item>
              <Command.Item
                onSelect={() => handleSelectNav("terminal")}
                className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer aria-selected:bg-[var(--primary)]/10 aria-selected:text-[var(--primary)]"
              >
                <Terminal className="w-4 h-4 text-[var(--ink-muted)]" />
                <span>{t("nav.terminal")}</span>
              </Command.Item>
              <Command.Item
                onSelect={() => handleSelectNav("jobs")}
                className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer aria-selected:bg-[var(--primary)]/10 aria-selected:text-[var(--primary)]"
              >
                <ListTodo className="w-4 h-4 text-[var(--ink-muted)]" />
                <span>{t("nav.jobs")}</span>
              </Command.Item>
              {user?.role === "admin" && (
                <Command.Item
                  onSelect={() => handleSelectNav("audit")}
                  className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer aria-selected:bg-[var(--primary)]/10 aria-selected:text-[var(--primary)]"
                >
                  <ShieldCheck className="w-4 h-4 text-[var(--ink-muted)]" />
                  <span>{t("nav.audit")}</span>
                </Command.Item>
              )}
              <Command.Item
                onSelect={() => handleSelectNav("profile")}
                className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer aria-selected:bg-[var(--primary)]/10 aria-selected:text-[var(--primary)]"
              >
                <UserCheck className="w-4 h-4 text-[var(--ink-muted)]" />
                <span>{t("nav.profile")}</span>
              </Command.Item>
              {user?.role === "admin" && (
                <Command.Item
                  onSelect={() => handleSelectNav("settings")}
                  className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer aria-selected:bg-[var(--primary)]/10 aria-selected:text-[var(--primary)]"
                >
                  <Settings className="w-4 h-4 text-[var(--ink-muted)]" />
                  <span>{t("nav.settings")}</span>
                </Command.Item>
              )}
            </Command.Group>

            <Command.Group
              heading="Pengaturan Cepat"
              className="text-[11px] font-semibold text-[var(--ink-muted)] uppercase tracking-wider px-2 py-1 pt-2 border-t border-[var(--line)]"
            >
              <Command.Item
                onSelect={() => {
                  onThemeChange("light");
                  handleClose();
                }}
                className="flex items-center justify-between px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer"
              >
                <div className="flex items-center gap-3">
                  <Sun className="w-4 h-4 text-[var(--accent)]" />
                  <span>Tema: Terang</span>
                </div>
                {theme === "light" && (
                  <span className="text-[10px] text-[var(--primary)] font-bold">
                    Aktif
                  </span>
                )}
              </Command.Item>
              <Command.Item
                onSelect={() => {
                  onThemeChange("dark");
                  handleClose();
                }}
                className="flex items-center justify-between px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer"
              >
                <div className="flex items-center gap-3">
                  <Moon className="w-4 h-4 text-[var(--primary)]" />
                  <span>Tema: Gelap</span>
                </div>
                {theme === "dark" && (
                  <span className="text-[10px] text-[var(--primary)] font-bold">
                    Aktif
                  </span>
                )}
              </Command.Item>
              <Command.Item
                onSelect={() => {
                  onThemeChange("system");
                  handleClose();
                }}
                className="flex items-center justify-between px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer"
              >
                <div className="flex items-center gap-3">
                  <Monitor className="w-4 h-4 text-[var(--ink-muted)]" />
                  <span>Tema: Sistem</span>
                </div>
                {theme === "system" && (
                  <span className="text-[10px] text-[var(--primary)] font-bold">
                    Aktif
                  </span>
                )}
              </Command.Item>
              <Command.Item
                onSelect={handleToggleLang}
                className="flex items-center justify-between px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--ink)] hover:bg-[var(--raised)] cursor-pointer"
              >
                <div className="flex items-center gap-3">
                  <Languages className="w-4 h-4 text-[var(--ink-muted)]" />
                  <span>
                    Ganti Bahasa:{" "}
                    {i18n.language === "id" ? "English" : "Indonesia"}
                  </span>
                </div>
                <span className="font-mono text-[10px] uppercase text-[var(--ink-muted)]">
                  {i18n.language}
                </span>
              </Command.Item>
              <Command.Item
                onSelect={handleLogout}
                className="flex items-center gap-3 px-3 py-2 rounded-[var(--radius-sm)] text-xs text-[var(--danger)] hover:bg-[var(--raised)] cursor-pointer"
              >
                <LogOut className="w-4 h-4 text-[var(--danger)]" />
                <span>{t("auth.logout")}</span>
              </Command.Item>
            </Command.Group>
          </Command.List>
        </Command>
      </div>
    </div>
  );
};
