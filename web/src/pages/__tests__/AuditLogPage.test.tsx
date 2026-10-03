import { describe, it, expect } from 'vitest';
import { renderToString } from 'react-dom/server';
import { AuditLogPage } from '../AuditLogPage';
import { AuthProvider } from '../../context/AuthContext';
import '../../i18n';

describe('AuditLogPage Component', () => {
  it('renders audit log page title and table columns', () => {
    const html = renderToString(
      <AuthProvider>
        <AuditLogPage />
      </AuthProvider>
    );

    expect(html).toContain('Catatan Audit Sistem');
    expect(html).toContain('Aktor');
    expect(html).toContain('Aksi');
    expect(html).toContain('Resource Target');
  });
});
