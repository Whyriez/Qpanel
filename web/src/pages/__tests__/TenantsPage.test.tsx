import { describe, it, expect } from 'vitest';
import { renderToString } from 'react-dom/server';
import { TenantsPage } from '../TenantsPage';
import { AuthProvider } from '../../context/AuthContext';
import '../../i18n';

describe('TenantsPage Component', () => {
  it('renders tenant management header, buttons and search filter', () => {
    const html = renderToString(
      <AuthProvider>
        <TenantsPage />
      </AuthProvider>
    );

    expect(html).toContain('Manajemen Tenant');
    expect(html).toContain('Tambah Tenant');
    expect(html).toContain('Paket Hosting');
  });
});
