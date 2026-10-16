import { describe, it, expect } from 'vitest';
import { renderToString } from 'react-dom/server';
import { RuntimePage } from '../RuntimePage';
import { PhpExtensionsModal } from '../../components/runtime/PhpExtensionsModal';
import { NodeInstallModal } from '../../components/runtime/NodeInstallModal';
import { AuthProvider } from '../../context/AuthContext';
import '../../i18n';

describe('RuntimePage Component', () => {
  it('renders PHP runtime manager header, repository status, and security notice', () => {
    const html = renderToString(
      <AuthProvider>
        <RuntimePage />
      </AuthProvider>
    );

    expect(html).toContain('Manajer Runtime PHP');
    expect(html).toContain('Status repositori PHP');
    expect(html).toContain('Isolasi FPM Multi-Tenant &amp; Kebijakan Masking OS');
    expect(html).toContain('Konfigurasi repositori PHP');
  });

  it('renders tab switcher for PHP and Node.js runtimes', () => {
    const html = renderToString(
      <AuthProvider>
        <RuntimePage />
      </AuthProvider>
    );

    expect(html).toContain('PHP');
    expect(html).toContain('Node.js');
  });

  it('renders PhpExtensionsModal with version title and category filter buttons', () => {
    const html = renderToString(
      <PhpExtensionsModal
        version="8.3"
        isOpen={true}
        onClose={() => {}}
      />
    );

    expect(html).toContain('Ekstensi PHP 8.3');
    expect(html).toContain('Semua Kategori');
    expect(html).toContain('Total:');
    expect(html).toContain('Terpasang:');
    expect(html).toContain('Aktif:');
  });

  it('renders NodeInstallModal with version options, Corepack toggle, and SHA256 integrity notice', () => {
    const mockVersions = [
      {
        version: '22.14.0',
        raw_version: 'v22.14.0',
        is_installed: false,
        installed_path: null,
        is_lts: true,
        lts_codename: 'Jod',
        release_date: '2025-02-11',
        npm_version: '10.9.2',
        corepack_enabled: false,
        used_by_sites_count: 0,
        used_by_domains: [],
        can_delete: false,
      },
      {
        version: '20.18.3',
        raw_version: 'v20.18.3',
        is_installed: true,
        installed_path: '/opt/panel/runtimes/node/20.18.3',
        is_lts: true,
        lts_codename: 'Iron',
        release_date: '2025-02-11',
        npm_version: '10.8.2',
        corepack_enabled: true,
        used_by_sites_count: 1,
        used_by_domains: ['api.acme.test'],
        can_delete: false,
      },
    ];

    const html = renderToString(
      <NodeInstallModal
        isOpen={true}
        onClose={() => {}}
        availableVersions={mockVersions}
        onInstall={async () => {}}
        preselectedVersion="22.14.0"
      />
    );

    expect(html).toContain('Pasang Versi Node.js');
    expect(html).toContain('v22.14.0 (LTS Jod)');
    expect(html).toContain('Aktifkan Corepack (pnpm &amp; yarn shims)');
    expect(html).toContain('Pengaturan Lanjutan &amp; Mirror');
    expect(html).toContain('Integritas berkas divalidasi terhadap manifest SHASUMS256 resmi sebelum diekstrak.');
  });
});
