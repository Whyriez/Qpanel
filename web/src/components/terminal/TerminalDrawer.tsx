import React, { useState, useEffect, useCallback } from 'react';
import { TerminalTabItem, TerminalSession, TerminalConnectionStatus } from '../../types/terminal';
import { createTerminalSession, closeTerminalSession } from '../../api/client';
import { TerminalTabs } from './TerminalTabs';
import { TerminalInstance } from './TerminalInstance';
import { toast } from 'sonner';

interface TerminalDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  initialCwd?: string;
  title?: string;
}

export const TerminalDrawer: React.FC<TerminalDrawerProps> = ({
  isOpen,
  onClose,
  initialCwd,
}) => {
  const [tabs, setTabs] = useState<TerminalTabItem[]>([]);
  const [activeTabId, setActiveTabId] = useState<string>('');
  const [isMaximized, setIsMaximized] = useState(false);
  const [isSpawning, setIsSpawning] = useState(false);

  // Helper to spawn a new session tab
  const spawnTab = useCallback(
    async (cwd?: string) => {
      if (isSpawning) return;
      setIsSpawning(true);

      const targetCwd = cwd || initialCwd || '';
      const fallbackTitle = targetCwd
        ? targetCwd.split('/').filter(Boolean).pop() || 'Terminal'
        : `Terminal ${tabs.length + 1}`;

      const tempId = `tab-${Date.now()}`;
      const newTab: TerminalTabItem = {
        id: tempId,
        title: fallbackTitle,
        cwd: targetCwd,
        status: 'connecting',
      };

      setTabs((prev) => [...prev, newTab]);
      setActiveTabId(tempId);

      try {
        const session: TerminalSession = await createTerminalSession({
          cwd: targetCwd || undefined,
          cols: 80,
          rows: 24,
        });

        setTabs((prev) =>
          prev.map((t) =>
            t.id === tempId
              ? {
                  ...t,
                  session,
                  title: session.cwd.split('/').filter(Boolean).pop() || fallbackTitle,
                  status: 'connecting',
                }
              : t
          )
        );
      } catch (err: unknown) {
        const errorMsg = err instanceof Error ? err.message : 'Gagal membuka sesi terminal';
        toast.error(errorMsg);
        // Remove temporary failed tab
        setTabs((prev) => prev.filter((t) => t.id !== tempId));
      } finally {
        setIsSpawning(false);
      }
    },
    [initialCwd, isSpawning, tabs.length]
  );

  // Spawn initial tab when drawer opens with no tabs
  useEffect(() => {
    if (isOpen && tabs.length === 0 && !isSpawning) {
      spawnTab(initialCwd);
    }
  }, [isOpen, tabs.length, initialCwd, spawnTab, isSpawning]);

  const handleCloseTab = async (tabId: string) => {
    const tabToClose = tabs.find((t) => t.id === tabId);
    if (tabToClose?.session) {
      try {
        await closeTerminalSession(tabToClose.session.session_id);
      } catch {
        // ignore close error
      }
    }

    const nextTabs = tabs.filter((t) => t.id !== tabId);
    setTabs(nextTabs);

    if (activeTabId === tabId) {
      if (nextTabs.length > 0) {
        setActiveTabId(nextTabs[nextTabs.length - 1].id);
      } else {
        onClose();
      }
    }
  };

  const handleStatusChange = (tabId: string, status: TerminalConnectionStatus) => {
    setTabs((prev) =>
      prev.map((t) => (t.id === tabId ? { ...t, status } : t))
    );
  };

  if (!isOpen) return null;

  return (
    <div
      className={`fixed z-50 transition-all duration-200 flex flex-col bg-[var(--surface)] border border-[var(--line)] shadow-2xl overflow-hidden ${
        isMaximized
          ? 'inset-2 rounded-[var(--radius-lg)]'
          : 'bottom-0 left-0 right-0 md:left-64 h-[420px] rounded-t-[var(--radius-lg)] border-b-0'
      }`}
    >
      {/* Tab bar header */}
      <TerminalTabs
        tabs={tabs}
        activeTabId={activeTabId}
        onSelectTab={setActiveTabId}
        onNewTab={() => spawnTab()}
        onCloseTab={handleCloseTab}
        isMaximized={isMaximized}
        onToggleMaximize={() => setIsMaximized(!isMaximized)}
        onCloseAll={onClose}
      />

      {/* Terminal Viewports Container */}
      <div className="flex-1 w-full h-full relative overflow-hidden bg-[var(--surface)]">
        {tabs.map((tab) => {
          if (!tab.session) {
            return (
              <div
                key={tab.id}
                className={`h-full w-full flex flex-col items-center justify-center text-xs text-[var(--ink-muted)] ${
                  tab.id === activeTabId ? 'flex' : 'hidden'
                }`}
              >
                <div className="animate-spin h-5 w-5 border-2 border-[var(--primary)] border-t-transparent rounded-full mb-2" />
                <span>Memulai sesi shell tenant...</span>
              </div>
            );
          }

          return (
            <TerminalInstance
              key={tab.session.session_id}
              session={tab.session}
              isActive={tab.id === activeTabId}
              onStatusChange={(st) => handleStatusChange(tab.id, st)}
            />
          );
        })}
      </div>
    </div>
  );
};
