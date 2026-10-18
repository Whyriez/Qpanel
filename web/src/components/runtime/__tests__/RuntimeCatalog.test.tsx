import { describe, expect, it } from 'vitest';
import { renderToString } from 'react-dom/server';
import { PythonRuntimesTab } from '../PythonRuntimesTab';
import { CatalogScheduleCard } from '../CatalogScheduleCard';
import { NodeInstallModal } from '../NodeInstallModal';

describe('Runtime catalog controls', () => {
  it('shows Python installation surface and loading status', () => {
    const html = renderToString(<PythonRuntimesTab />);
    expect(html).toContain('Runtime Python');
    expect(html).toContain('Memuat runtime Python');
    expect(html).toContain('Muat ulang Python');
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
