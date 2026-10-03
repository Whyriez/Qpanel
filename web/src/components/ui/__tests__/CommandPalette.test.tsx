import { describe, it, expect, vi } from 'vitest';
import { renderToString } from 'react-dom/server';
import { CommandPalette } from '../CommandPalette';
import { AuthProvider } from '../../../context/AuthContext';
import '../../../i18n';

describe('CommandPalette Component', () => {
  it('renders command palette items when open', () => {
    const onNavigate = vi.fn();
    const onThemeChange = vi.fn();

    const html = renderToString(
      <AuthProvider>
        <CommandPalette
          isOpen={true}
          onNavigate={onNavigate}
          theme="light"
          onThemeChange={onThemeChange}
        />
      </AuthProvider>
    );

    expect(html).toContain('Command Palette');
    expect(html).toContain('Navigasi');
    expect(html).toContain('Pengaturan Cepat');
    expect(html).toContain('Tema: Terang');
    expect(html).toContain('Tema: Gelap');
  });

  it('renders nothing when closed', () => {
    const onNavigate = vi.fn();
    const onThemeChange = vi.fn();

    const html = renderToString(
      <AuthProvider>
        <CommandPalette
          isOpen={false}
          onNavigate={onNavigate}
          theme="light"
          onThemeChange={onThemeChange}
        />
      </AuthProvider>
    );

    expect(html).toBe('');
  });
});
