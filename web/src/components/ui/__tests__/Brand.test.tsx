import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { Brand, BrandLoading } from '../Brand';
import '../../../i18n';
describe('QonPanel identity', () => {
  it('shares the canonical decorative mark and accessible product name', () => {
    const html = renderToStaticMarkup(<Brand />);
    expect(html).toContain('aria-label="QonPanel"');
    expect(html).toContain('/brand/qonpanel-mark.svg');
    expect(html).toContain('aria-hidden="true"');
    expect(html).not.toMatch(/Qpanel|Nusa|Vpanel/);
  });
  it('announces loading and uses the same brand in full screen and route fallbacks', () => {
    for (const fullScreen of [true, false]) {
      const html = renderToStaticMarkup(<BrandLoading fullScreen={fullScreen} message="Memeriksa sesi Anda?" />);
      expect(html).toContain('role="status"');
      expect(html).toContain('aria-live="polite"');
      expect(html).toContain('QonPanel');
      expect(html.includes('brand-loading--full')).toBe(fullScreen);
    }
  });
});
