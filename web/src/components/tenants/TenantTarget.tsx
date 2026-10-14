import React, { useEffect, useState } from 'react';
import { useAuth } from '../../context/AuthContext';
import { fetchTenants } from '../../api/client';
import type { TenantListItem } from '../../types/api';
import { navigate, RouteLink, useBrowserRoute } from '../../lib/router';

export function TenantTarget({children}: {children:(tenant?:string)=>React.ReactNode}) {
  const {user}=useAuth();
  const route=useBrowserRoute();
  const [tenants,setTenants]=useState<TenantListItem[] | null>(null);
  const [error,setError]=useState('');
  const admin=user?.role==='admin';
  useEffect(()=>{
    let active=true;
    if(admin) fetchTenants().then(rows=>{if(active)setTenants(rows.filter(t=>t.status==='active' && !!t.linux_username && !!t.linux_uid));}).catch(e=>{if(active)setError(e.message);});
    return ()=>{active=false;};
  },[admin]);
  if(!admin) return <>{children()}</>;
  if(error) return <div role="alert">Gagal memuat tenant: {error}</div>;
  if(!tenants) return <div role="status">Memuat tenant…</div>;
  if(tenants.length===0) return <div className="p-6"><h2>Belum ada tenant yang dapat dikelola.</h2><p>Buat tenant terlebih dahulu.</p><RouteLink href="/tenants" className="button-primary">Buka tenant</RouteLink></div>;
  const selected=tenants.find(t=>String(t.id)===route.search.get('tenant'));
  return <div className="space-y-4"><div className="flex flex-wrap items-center gap-3 p-4 border-b border-[var(--line)]">
    <label htmlFor="execution-tenant">Tenant</label>
    <select id="execution-tenant" value={selected?.id || ''} onChange={e=>{
      const search=new URLSearchParams(route.search); if(e.target.value)search.set('tenant',e.target.value);else search.delete('tenant');
      navigate(`${route.pathname}${search.size ? `?${search}` : ''}`);
    }} className="rounded border border-[var(--line)] bg-[var(--surface)] p-2"><option value="">Pilih tenant</option>{tenants.map(t=><option key={t.id} value={t.id}>{t.username}</option>)}</select>
    <RouteLink href="/tenants">Kelola tenant</RouteLink>
  </div>{selected ? children(String(selected.id)) : <p role="status" className="p-4">Pilih tenant untuk membuka berkas atau terminalnya.</p>}</div>;
}
