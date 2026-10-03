import { describe, it, expect } from 'vitest';
import { renderToString } from 'react-dom/server';
import { SecurityProfilePage } from '../SecurityProfilePage';
import { AuthProvider } from '../../context/AuthContext';
import '../../i18n';

describe('SecurityProfilePage Component', () => {
  it('renders profile, password change and 2FA sections', () => {
    const html = renderToString(
      <AuthProvider>
        <SecurityProfilePage />
      </AuthProvider>
    );

    expect(html).toContain('Profil &amp; Keamanan');
    expect(html).toContain('Informasi Akun');
    expect(html).toContain('Ganti Kata Sandi');
    expect(html).toContain('Autentikasi Dua Faktor (2FA)');
  });
});
