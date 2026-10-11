import React, { useState } from 'react';
import { Sliders, Cpu, Activity, ShieldCheck, Wrench } from 'lucide-react';
import { ServerOptimizerCard } from '../components/system/ServerOptimizerCard';
import { PanelMemoryProfileCard } from '../components/system/PanelMemoryProfileCard';
import { SecurityHardeningCard } from '../components/system/SecurityHardeningCard';
import { SystemReliabilityCard } from '../components/system/SystemReliabilityCard';

export const SettingsPage: React.FC = () => {
  const [activeSection, setActiveSection] = useState<'optimizer' | 'security_hardening' | 'reliability' | 'memory_profile' | 'info'>('optimizer');

  return (
    <div className="space-y-6" data-testid="settings-page">
      {/* Page Header */}
      <div>
        <h1 className="text-xl font-bold text-[var(--ink)]">
          Pengaturan Sistem & Pengoptimal Server
        </h1>
        <p className="text-xs text-[var(--ink-muted)] mt-1">
          Konfigurasi otomatis performa server, profiling hardware, audit keamanan OWASP ASVS L2, isolasi nftables, dan validasi efisiensi PRD Bab 9.
        </p>
      </div>

      {/* Navigation Tabs */}
      <div className="flex flex-wrap items-center gap-2 border-b border-[var(--line)] pb-2 text-xs">
        <button
          type="button"
          onClick={() => setActiveSection('optimizer')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] font-medium transition-colors ${
            activeSection === 'optimizer'
              ? 'bg-[var(--primary)] text-white shadow-sm'
              : 'text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--surface-sunken)]'
          }`}
        >
          <Sliders className="w-3.5 h-3.5" />
          Pengoptimal Server (Hardware Tuning)
        </button>

        <button
          type="button"
          onClick={() => setActiveSection('security_hardening')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] font-medium transition-colors ${
            activeSection === 'security_hardening'
              ? 'bg-[var(--primary)] text-white shadow-sm'
              : 'text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--surface-sunken)]'
          }`}
        >
          <ShieldCheck className="w-3.5 h-3.5" />
          Hardening & Firewall (OWASP & nftables)
        </button>

        <button
          type="button"
          onClick={() => setActiveSection('reliability')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] font-medium transition-colors ${
            activeSection === 'reliability'
              ? 'bg-[var(--primary)] text-white shadow-sm'
              : 'text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--surface-sunken)]'
          }`}
          data-testid="tab-reliability"
        >
          <Wrench className="w-3.5 h-3.5" />
          Keandalan & Diagnostik (Doctor & Reconcile)
        </button>

        <button
          type="button"
          onClick={() => setActiveSection('memory_profile')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] font-medium transition-colors ${
            activeSection === 'memory_profile'
              ? 'bg-[var(--primary)] text-white shadow-sm'
              : 'text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--surface-sunken)]'
          }`}
        >
          <Activity className="w-3.5 h-3.5" />
          Profil Memori & Efisiensi (PRD Bab 9)
        </button>

        <button
          type="button"
          onClick={() => setActiveSection('info')}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-[var(--radius-md)] font-medium transition-colors ${
            activeSection === 'info'
              ? 'bg-[var(--primary)] text-white shadow-sm'
              : 'text-[var(--ink-muted)] hover:text-[var(--ink)] hover:bg-[var(--surface-sunken)]'
          }`}
        >
          <Cpu className="w-3.5 h-3.5" />
          Pedoman Profil & Toleransi
        </button>
      </div>

      {/* Section Content */}
      {activeSection === 'optimizer' && (
        <ServerOptimizerCard />
      )}

      {activeSection === 'security_hardening' && (
        <SecurityHardeningCard />
      )}

      {activeSection === 'reliability' && (
        <SystemReliabilityCard />
      )}

      {activeSection === 'memory_profile' && (
        <PanelMemoryProfileCard />
      )}

      {activeSection === 'info' && (
        <div className="border border-[var(--line)] bg-[var(--surface)] rounded-[var(--radius-md)] p-6 space-y-4">
          <div className="flex items-center gap-2">
            <Cpu className="w-5 h-5 text-[var(--primary)]" />
            <h2 className="text-base font-semibold text-[var(--ink)]">
              Kriteria Tingkatan Profil Tuning (P10-T2)
            </h2>
          </div>
          <p className="text-xs text-[var(--ink-muted)] leading-relaxed">
            Sistem pengoptimal secara otomatis menghitung metrik perangkat keras host dan mengkategorikan ke dalam 4 tingkatan profil:
          </p>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-xs">
            <div className="p-3.5 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface-sunken)] space-y-1.5">
              <span className="font-semibold text-[var(--ink)]">1. Micro (&lt; 2 GB RAM)</span>
              <p className="text-[var(--ink-muted)]">
                Didesain untuk VPS hemat resource (1-2 vCPU, RAM 512MB-1GB). Menggunakan alokasi buffer minimal, worker Nginx auto-capped, dan PHP-FPM ondemand dengan max_children rendah untuk mencegah Out-Of-Memory (OOM).
              </p>
            </div>

            <div className="p-3.5 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface-sunken)] space-y-1.5">
              <span className="font-semibold text-[var(--ink)]">2. Standard (2 - 4 GB RAM)</span>
              <p className="text-[var(--ink-muted)]">
                Profil seimbang untuk VPS hosting umum. Menggunakan koneksi worker menengah (2.048 koneksi), somaxconn 4.096, dan buffer fastcgi 64k.
              </p>
            </div>

            <div className="p-3.5 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface-sunken)] space-y-1.5">
              <span className="font-semibold text-[var(--ink)]">3. High Performance (4 - 16 GB RAM)</span>
              <p className="text-[var(--ink-muted)]">
                Untuk server production dengan lalu lintas tinggi. Mendukung hingga 4.096 koneksi per worker, somaxconn 16.384, tcp_tw_reuse aktif, dan cluster runtime multi-core.
              </p>
            </div>

            <div className="p-3.5 rounded-[var(--radius-md)] border border-[var(--line)] bg-[var(--surface-sunken)] space-y-1.5">
              <span className="font-semibold text-[var(--ink)]">4. Dedicated Large (&gt; 16 GB RAM)</span>
              <p className="text-[var(--ink-muted)]">
                Konfigurasi ultra-throughput untuk server bare-metal/dedicated. File descriptor 1.000.000, somaxconn 65.535, worker connections 8.192, dan alokasi buffer bervolume besar.
              </p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default SettingsPage;
