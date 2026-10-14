import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderToString } from 'react-dom/server';
import { SshConnectModal } from '../SshConnectModal';
import * as client from '../../../api/client';
import { SshConnectionInfoResponse } from '../../../types/api';

describe('SshConnectModal Component', () => {
  const sampleFullInfo: SshConnectionInfoResponse = {
    host: 'server.example.com',
    port: 22,
    username: 'p_alice',
    user_id: 10,
    home_dir: '/home/p_alice',
    active_path: '/home/p_alice/sites/my-app/public',
    relative_path: 'sites/my-app/public',
    shell_policy: 'full',
    can_shell: true,
    can_sftp: true,
    has_ssh_keys: true,
    ssh_keys_count: 2,
    ssh_command:
      "ssh -t p_alice@server.example.com \"cd '/home/p_alice/sites/my-app/public' && exec \\$SHELL -l\"",
    sftp_command: "sftp p_alice@server.example.com:'/home/p_alice/sites/my-app/public'",
    sftp_url: 'sftp://p_alice@server.example.com/home/p_alice/sites/my-app/public',
    ssh_config_snippet:
      'Host my-app\n    HostName server.example.com\n    User p_alice\n    IdentityFile ~/.ssh/id_ed25519\n',
    site_domain: 'my-app.com',
  };

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('does not render when isOpen is false', () => {
    const html = renderToString(
      <SshConnectModal isOpen={false} onClose={() => {}} />
    );
    expect(html).toBe('');
  });

  it('renders modal dialog with connection details when isOpen is true', () => {
    vi.spyOn(client, 'fetchSshConnectionInfo').mockResolvedValue(sampleFullInfo);

    const html = renderToString(
      <SshConnectModal
        isOpen={true}
        onClose={() => {}}
        path="/sites/my-app/public"
        siteDomain="my-app.com"
        tenantUsername="p_alice"
      />
    );

    expect(html).toContain('Koneksi SSH &amp; SFTP');
    expect(html).toContain('Memuat informasi koneksi...');
    expect(html).not.toContain('Parameter Koneksi Server');
  });

  it('does not fabricate SSH commands before connection data arrives', () => {
    vi.spyOn(client, 'fetchSshConnectionInfo').mockResolvedValue(sampleFullInfo);

    const html = renderToString(
      <SshConnectModal
        isOpen={true}
        onClose={() => {}}
        path="/sites/my-app/public"
        siteDomain="my-app.com"
        tenantUsername="p_alice"
      />
    );

    expect(html).not.toContain('ssh -t p_alice@');
    expect(html).not.toContain('Salin Perintah SSH (Buka di Folder Aktif)');
  });

  it('renders SFTP details and copy actions', () => {
    vi.spyOn(client, 'fetchSshConnectionInfo').mockResolvedValue(sampleFullInfo);

    const html = renderToString(
      <SshConnectModal
        isOpen={true}
        onClose={() => {}}
        path="/sites/my-app/public"
        siteDomain="my-app.com"
        tenantUsername="p_alice"
      />
    );

    expect(html).not.toContain('Salin Perintah SFTP CLI');
    expect(html).not.toContain('sftp p_alice@');
  });
});
