import { describe, it, expect, vi } from 'vitest';
import { renderToString } from 'react-dom/server';
import { LoginPage } from '../LoginPage';
import { AuthProvider } from '../../context/AuthContext';
import '../../i18n';

describe('LoginPage Component', () => {
  it('renders login form with username, password and submit button', () => {
    const onOpenWizard = vi.fn();

    const html = renderToString(
      <AuthProvider>
        <LoginPage onOpenWizard={onOpenWizard} />
      </AuthProvider>
    );

    expect(html).toContain('Masuk');
    expect(html).toContain('Nama Pengguna');
    expect(html).toContain('Kata Sandi');
    expect(html).toContain('Buka Wizard Setup');
  });
});
