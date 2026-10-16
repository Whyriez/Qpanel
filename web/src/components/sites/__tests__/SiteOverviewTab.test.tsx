import { describe, it, expect, vi } from "vitest";
import { renderToString } from "react-dom/server";
import { SiteOverviewTab, SiteOverviewContent } from "../SiteOverviewTab";
import { Site, SiteOverviewResponse } from "../../../types/api";
import "../../../i18n";

describe("SiteOverviewTab Component", () => {
  const mockSite: Site = {
    id: 42,
    user_id: 1,
    primary_domain: "mysite.test",
    kind: "php",
    version: "8.3",
    preset: "standard",
    docroot: "/var/www/vhosts/p_user/mysite.test",
    status: "active",
    spa_fallback: false,
    force_https: true,
    created_at: 1770000000,
    updated_at: 1770000000,
    domains: [],
  };

  it("renders initial loading state cleanly", () => {
    const html = renderToString(
      <SiteOverviewTab site={mockSite} onNavigateTab={vi.fn()} />,
    );

    expect(html).toContain("Memuat ringkasan situs...");
  });

  const overview: SiteOverviewResponse = {
    site_id: 42,
    domain: "mysite.test",
    status: {
      webserver_status: "active",
      runtime_status: "running",
      healthy: true,
      uptime_secs: 3600,
      pid: 123,
    },
    resources: {
      cpu_usage_usec: 0,
      cpu_percent: 3.2,
      memory_bytes: 0,
      memory_limit_bytes: 1024,
      pids_current: 0,
      pids_limit: 100,
    },
    ssl: {
      has_certificate: true,
      issuer: "Let's Encrypt",
      days_remaining: 80,
      valid_until: 1800000000,
      auto_renew: true,
      status: "valid",
      force_https: true,
    },
    runtime: {
      kind: "php",
      version: "8.3",
      entrypoint_or_target: "/home/p_user/sites/mysite.test/public",
    },
    storage: {
      docroot_bytes: 0,
      databases_bytes: 0,
      backups_bytes: 0,
      total_site_bytes: 0,
      plan_disk_limit_bytes: 1024,
      usage_percentage: 0,
    },
    quick_actions_available: ["restart", "reload"],
    warnings: [],
    latest_deployment: null,
    latest_backup: null,
  };
  const render = (data = overview) =>
    renderToString(
      <SiteOverviewContent
        site={mockSite}
        overview={data}
        onNavigateTab={vi.fn()}
        onQuickAction={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

  it("shows actual status, available actions and useful empty states", () => {
    const html = render();
    expect(html).toContain("Situs beroperasi normal");
    expect(html).toContain("Berjalan");
    expect(html).toContain("Mulai ulang");
    expect(html).not.toContain("Hentikan");
    expect(html).toContain("Belum ada deployment");
    expect(html).toContain("Belum ada cadangan");
    expect(html).toContain('href="https://mysite.test"');
    expect(html).not.toMatch(/bg-slate|text-white|backdrop-blur|shadow-lg/);
  });
  it("keeps zero usage empty and does not invent tenant limits", () => {
    expect(render()).toContain('aria-valuenow="0"');
    const html = render({
      ...overview,
      resources: {
        ...overview.resources,
        memory_limit_bytes: 0,
        pids_limit: 0,
      },
      storage: { ...overview.storage, plan_disk_limit_bytes: 0 },
    });
    expect(html).toContain("Tanpa batas terkonfigurasi");
    expect(html).toContain("Kuota penyimpanan belum ditetapkan");
    expect(html).not.toContain("aria-valuenow");
    expect(html).not.toContain("1 GB");
    expect(html).not.toContain("10 GB");
  });
  it("does not force HTTPS without a certificate and shows warnings and failed history", () => {
    const html = render({
      ...overview,
      ssl: {
        ...overview.ssl,
        has_certificate: false,
        status: "none",
        issuer: null,
      },
      warnings: [
        {
          id: "ssl",
          level: "warning",
          title: "Sertifikat belum tersedia",
          message: "Terbitkan sertifikat untuk HTTPS.",
          action_tab: "ssl",
        },
      ],
      latest_backup: {
        backup_id: 1,
        filename: "backup.tar.gz",
        size_bytes: 1024,
        status: "failed",
        includes_files: true,
        includes_databases: false,
        created_at: 1770000000,
      },
    });
    expect(html).toContain('href="http://mysite.test"');
    expect(html).toContain("Sertifikat belum tersedia");
    expect(html).toContain("backup.tar.gz");
    expect(html).toContain("Gagal");
  });
});
