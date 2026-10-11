import { describe, it, expect, vi } from 'vitest';
import { renderToString } from 'react-dom/server';
import { SiteOverviewTab } from '../SiteOverviewTab';
import { Site } from '../../../types/api';
import '../../../i18n';

describe('SiteOverviewTab Component', () => {
  const mockSite: Site = {
    id: 42,
    user_id: 1,
    primary_domain: 'mysite.test',
    kind: 'php',
    version: '8.3',
    preset: 'standard',
    docroot: '/var/www/vhosts/p_user/mysite.test',
    status: 'active',
    spa_fallback: false,
    force_https: true,
    created_at: 1770000000,
    updated_at: 1770000000,
    domains: [],
  };

  it('renders initial loading state cleanly', () => {
    const html = renderToString(
      <SiteOverviewTab
        site={mockSite}
        onNavigateTab={vi.fn()}
      />
    );

    expect(html).toContain('Memuat ringkasan performa &amp; status situs...');
  });
});
