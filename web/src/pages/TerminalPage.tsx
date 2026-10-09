import React, { useState, useEffect, useCallback } from 'react';
import { TerminalTabItem, TerminalSession, TerminalConnectionStatus } from '../types/terminal';
import { createTerminalSession, closeTerminalSession } from '../api/client';
import { TerminalTabs } from '../components/terminal/TerminalTabs';
import { TerminalInstance } from '../components/terminal/TerminalInstance';
import { toast } from 'sonner';

export const TerminalPage: React.FC = () => {
  const [tabs, setTabs] = useState<TerminalTabItem[]>([]);
  const [activeTabId, setActiveTabId] = useState<string>('');
  const [isSpawning, setIsSpawning] = useState(false);

  const spawnTab = useCallback(
    async (cwd?: string) => {
      if (isSpawning) return;
      setIsSpawning(true);

      const targetCwd = cwd || '';
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
        setTabs((prev) => prev.filter((t) => t.id !== tempId));
      } finally {
        setIsSpawning(false);
      }
    },
    [isSpawning, tabs.length]
  );

  // Spawn initial tab on mount if empty
  useEffect(() => {
    if (tabs.length === 0 && !isSpawning) {
      spawnTab();
    }
  }, [tabs.length, isSpawning, spawnTab]);

  const handleCloseTab = async (tabId: string) => {
    const tabToClose = tabs.find((t) => t.id === tabId);
    if (tabToClose?.session) {
      try {
        await closeTerminalSession(tabToClose.session.session_id);
      } catch {
        // ignore
      }
    }

    const nextTabs = tabs.filter((t) => t.id !== tabId);
    setTabs(nextTabs);

    if (activeTabId === tabId && nextTabs.length > 0) {
      setActiveTabId(nextTabs[nextTabs.length - 1].id);
    }
  };

  const handleStatusChange = (tabId: string, status: TerminalConnectionStatus) => {
    setTabs((prev) =>
      prev.map((t) => (t.id === tabId ? { ...t, status } : t))
    );
  };

  return (
    <div className="h-full w-full flex flex-col rounded-[var(--radius-lg)] border border-[var(--line)] bg-[var(--surface)] overflow-hidden shadow-sm">
      {/* Header Tabs */}
      <TerminalTabs
        tabs={tabs}
        activeTabId={activeTabId}
        onSelectTab={setActiveTabId}
        onNewTab={() => spawnTab()}
        onCloseTab={handleCloseTab}
      />

      {/* Viewport */}
      <div className="flex-1 w-full h-full relative overflow-hidden bg-[var(--surface)]">
        {tabs.length === 0 ? (
          <div className="h-full w-full flex flex-col items-center justify-center p-8 text-center text-xs text-[var(--ink-muted)]">
            <p className="mb-3">Tidak ada sesi terminal aktif saat ini.</p>
            <button
              type="button"
              onClick={() => spawnTab()}
              className="px-3 py-1.5 rounded-[var(--radius-md)] bg-[var(--primary)] text-white text-xs font-medium hover:opacity-90"
            >
              Buka Sesi Terminal
            </button>
          </div>
        ) : (
          tabs.map((tab) => {
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
          })
        )}
      </div>
    </div>
  );
};
