import { describe, it, expect } from 'vitest';
import { renderToString } from 'react-dom/server';
import { RuntimePage } from '../RuntimePage';
import { AuthProvider } from '../../context/AuthContext';
import '../../i18n';

describe('RuntimePage Component', () => {
  it('renders PHP runtime manager header, Sury PPA status, and security notice', () => {
    const html = renderToString(
      <AuthProvider>
        <RuntimePage />
      </AuthProvider>
    );

    expect(html).toContain('Manajer Runtime PHP');
    expect(html).toContain('Status Repositori Sury PPA');
    expect(html).toContain('Isolasi FPM Multi-Tenant &amp; Kebijakan Masking OS');
    expect(html).toContain('Konfigurasi Repositori Sury PPA');
  });
});
