import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { PythonRuntimesTab } from '../PythonRuntimesTab';
import { CatalogScheduleCard } from '../CatalogScheduleCard';
import { NodeInstallModal } from '../NodeInstallModal';
import { isFullPythonVersion, PythonInstallModal } from '../PythonInstallModal';

describe('Runtime catalog controls', () => {
  it('shows Python installation surface and loading status', () => {
    const html = renderToString(<PythonRuntimesTab />);
    expect(html).toContain('Runtime Python');
    expect(html).toContain('Memuat runtime Python');
    expect(html).toContain('Muat ulang Python');
    expect(html).toContain('>Pasang Python</button>');
  });
  it('offers installation and manual version entry even before the catalog is populated', () => {
    const html = renderToString(<PythonInstallModal isOpen versions={[]} installedVersions={[]} busyVersions={[]} unavailableReason="" refreshing={false} onClose={() => {}} onInstall={async () => {}} onRefresh={async () => {}} />);
    expect(html).toContain('Pasang Python');
    expect(html).toContain('Muat daftar versi Python');
    expect(html).toContain('Masukkan versi lengkap');
    expect(html).toMatch(/type="submit"[^>]*disabled=""/);
  });
  it('accepts exact CPython versions and rejects shorthand, commands, paths and overflow', () => {
    for (const version of ['3.13.9', '3.15.0']) expect(isFullPythonVersion(version)).toBe(true);
    for (const version of ['', '3.13', '3.15.0rc1', '../3.13.9', '3.13.9;id', '3.65536.0', '3.13.09', '4.0.0']) expect(isFullPythonVersion(version), version).toBe(false);
  });
  it('explains catalog-only scheduling and provides explicit refresh', () => {
    const html = renderToString(<CatalogScheduleCard onRefresh={() => {}} />);
    expect(html).toContain('Perbarui katalog sekarang');
    expect(html).toContain('Instalasi dan versi situs tetap dipilih manual');
  });
  it('blocks installation when a preselected Node version is outside supported catalog', () => {
    const html = renderToString(<NodeInstallModal isOpen onClose={() => {}} onInstall={async () => {}} availableVersions={[]} preselectedVersion="18.20.0" />);
    expect(html).toContain('Versi ini tidak tersedia dalam katalog LTS');
    expect(html).toMatch(/type="submit" disabled=""/);
  });
});
