import { describe, it, expect } from 'vitest';
import { renderToString } from 'react-dom/server';
import '../../i18n';
import { Brand } from '../../components/ui/Brand';
import { NotFoundPage } from '../NotFoundPage';

describe('Qpanel branding and missing page', () => {
  it('renders the Qpanel identity and useful recovery links', () => {
    const html = renderToString(<><Brand /><NotFoundPage /></>);
    expect(html).toContain('aria-label="Qpanel"');
    expect(html).toContain('Halaman tidak ditemukan.');
    expect(html).toContain('href="/dashboard"');
    expect(html).toContain('href="/sites"');
    expect(html).not.toContain('Nusa');
    expect(html).not.toContain('nginx');
  });
});
