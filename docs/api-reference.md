# Referensi REST API Nusa Panel

Nusa Panel menyediakan antarmuka REST API modern yang sepenuhnya terstruktur, bertipe kuat, dan terdokumentasi melalui spesifikasi OpenAPI 3.1. Dokumen ini menjelaskan konvensi pemanggilan API, otentikasi, format respons, dan katalog endpoint per modul.

---

## 1. Konvensi Umum API

### 1.1 Base URL & Format Data
- **Base URL**: `https://<ip-atau-domain-panel>:3000`
- **Prefix Versi API**: `/api/v1`
- **Format Pertukaran Data**: JSON (`Content-Type: application/json; charset=utf-8`)
- **Penanganan Waktu**: Seluruh nilai timestamp menggunakan format detik UNIX epoch (`u64`) atau ISO-8601 UTC string.

### 1.2 Format Error Standar (RFC 9457 Problem Details)
Ketika terjadi galat (HTTP 4xx atau 5xx), API selalu mengembalikan struktur respons standar RFC 9457:
```json
{
  "type": "https://vpanel.internal/errors/resource-not-found",
  "title": "Resource Not Found",
  "status": 404,
  "detail": "Situs dengan domain 'app.example.com' tidak ditemukan dalam sistem.",
  "instance": "/api/v1/sites/4b7a42ce-6d09-411a-829b-90f7193798ef"
}
```

---

## 2. Autentikasi & Keamanan Sesi

### 2.1 Skema Bearer Token JWT
Sebagian besar endpoint memerlukan token otentikasi Bearer pada header permintaan:
```http
Authorization: Bearer <JWT_ACCESS_TOKEN>
```

### 2.2 Proteksi CSRF
Untuk interaksi peramban menggunakan cookie sesi, setiap mutasi `POST`, `PUT`, `PATCH`, dan `DELETE` wajib menyertakan token CSRF:
```http
X-CSRF-Token: <CSRF_TOKEN_FROM_AUTH_ENDPOINT>
```

### 2.3 Tingkat Otorisasi (RBAC)
- **`Admin`**: Akses tak terbatas ke seluruh sistem, manajemen host, konfigurasi paket, dan audit log.
- **`Operator`**: Akses operasional untuk pemeliharaan rutin tanpa wewenang mengubah master key atau pengaturan inti.
- **`Tenant`**: Akses terbatas yang secara ketat diisolasi hanya pada resource (situs, database, file, cron) milik tenant itu sendiri.

---

## 3. Spesifikasi OpenAPI 3.1 & Swagger UI

- **Spesifikasi Lengkap**: Tersedia di berkas [docs/openapi.json](file:///e:/Latihan/Panel%20Hosting/Vpanel/docs/openapi.json).
- **Swagger UI Interaktif**: Saat server `panel-api` berjalan, dokumentasi interaktif dapat diakses langsung pada peramban di URL:
  ```
  http://<server-ip>:3000/api/docs/
  ```

---

## 4. Ringkasan Endpoint Utama

### 4.1 Sistem & Kesehatan (System & Health)
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `GET` | `/healthz` | Publik | Healthcheck probe ringan untuk load balancer atau monitoring. |
| `GET` | `/api/v1/system/info` | Publik/Auth | Menampilkan versi API, OS, arsitektur, dan status koneksi socket agent. |
| `GET` | `/api/v1/system/memory-profile` | Admin | Laporan alokasi memori biner, heap tracker, dan target < 65 MB. |
| `GET` | `/api/v1/system/doctor` | Admin | Menjalankan diagnosa komprehensif 8 kategori keandalan host. |
| `POST` | `/api/v1/system/reconcile` | Admin | Memperbaiki drift konfigurasi antara database panel dan sistem operasi. |

### 4.2 Autentikasi (Authentication)
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `POST` | `/api/v1/auth/login` | Publik | Otentikasi username/password, verifikasi rate limit, dan pembuatan sesi. |
| `POST` | `/api/v1/auth/logout` | Auth | Membatalkan sesi aktif dan memasukkan JWT ke revocation list. |
| `GET` | `/api/v1/auth/me` | Auth | Mengambil profil pengguna terautentikasi dan daftar permission. |
| `POST` | `/api/v1/auth/password` | Auth | Mengganti kata sandi dengan verifikasi password lama (Argon2id). |
| `POST` | `/api/v1/auth/2fa/setup` | Auth | Menghasilkan secret TOTP baru dan URI QR code autentikator. |
| `POST` | `/api/v1/auth/2fa/verify` | Auth | Memvalidasi 6-digit TOTP code dan mengaktifkan 2FA. |

### 4.3 Manajemen Situs Web (Sites)
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/sites` | Auth | Daftar seluruh situs (difilter per-tenant jika bukan admin). |
| `POST` | `/api/v1/sites` | Tenant+ | Membuat situs baru (Static, PHP-FPM, Node.js, Python, Reverse Proxy). |
| `GET` | `/api/v1/sites/{id}` | Tenant+ | Mengambil detail konfigurasi virtual host dan runtime situs. |
| `PUT` | `/api/v1/sites/{id}` | Tenant+ | Memperbarui parameter situs (root dir, port proxy, env vars). |
| `DELETE`| `/api/v1/sites/{id}` | Tenant+ | Menghapus situs beserta vhost dan opsi pembersihan direktori. |
| `POST` | `/api/v1/sites/{id}/status` | Tenant+ | Mengubah status situs (`start`, `stop`, `restart`, `maintenance`). |
| `GET` | `/api/v1/sites/{id}/rules` | Tenant+ | Mengambil aturan kustom Nginx (redirect, security header, IP list). |
| `PUT` | `/api/v1/sites/{id}/rules` | Tenant+ | Menerapkan aturan kustom Nginx secara atomik. |
| `GET` | `/api/v1/sites/{id}/logs` | Tenant+ | Mengambil isi log Nginx access/error log atau stdout proses. |

### 4.4 Domain & Sertifikat SSL / TLS
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/ssl/certificates` | Tenant+ | Daftar sertifikat SSL aktif, masa berlaku, dan penerbit. |
| `POST` | `/api/v1/ssl/order-letsencrypt`| Tenant+ | Memicu penerbitan sertifikat Let's Encrypt via ACME HTTP-01. |
| `POST` | `/api/v1/ssl/custom` | Tenant+ | Memasang sertifikat kustom publik dan private key. |
| `POST` | `/api/v1/ssl/{id}/force-https` | Tenant+ | Mengaktifkan/menonaktifkan pengalihan permanen HTTP ke HTTPS. |

### 4.5 Pengelola Berkas (Web File Manager)
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/files/list` | Tenant+ | Daftar entri berkas dan direktori (terkunci di home tenant). |
| `GET` | `/api/v1/files/read` | Tenant+ | Membaca konten teks suatu file (dengan batas ukuran aman). |
| `POST` | `/api/v1/files/write` | Tenant+ | Menyimpan perubahan isi file teks. |
| `POST` | `/api/v1/files/upload` | Tenant+ | Mengunggah file via multipart form-data. |
| `POST` | `/api/v1/files/archive/zip`| Tenant+ | Membuat arsip ZIP dari daftar file terpilih. |
| `POST` | `/api/v1/files/archive/unzip`| Tenant+ | Mengekstrak arsip ZIP di lokasi direktori target. |
| `POST` | `/api/v1/files/permissions` | Tenant+ | Mengubah mode izin POSIX file (*chmod* aman). |

### 4.6 Basis Data Relasional (Databases)
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/databases` | Tenant+ | Daftar database MariaDB dan PostgreSQL milik tenant. |
| `POST` | `/api/v1/databases` | Tenant+ | Membuat database baru dengan prefix tenant otomatis. |
| `DELETE`| `/api/v1/databases/{id}` | Tenant+ | Menghapus database. |
| `GET` | `/api/v1/databases/users` | Tenant+ | Daftar pengguna database. |
| `POST` | `/api/v1/databases/users` | Tenant+ | Membuat pengguna database dan mengatur hak aksesnya. |
| `POST` | `/api/v1/databases/{id}/dump` | Tenant+ | Memulai proses ekspor SQL dump terkompresi. |

### 4.7 SSH & Sesi Web Terminal
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/ssh/keys` | Tenant+ | Mengambil daftar kunci publik SSH yang terdaftar. |
| `POST` | `/api/v1/ssh/keys` | Tenant+ | Mendaftarkan kunci publik SSH baru (validasi format Ed25519/RSA). |
| `DELETE`| `/api/v1/ssh/keys/{id}` | Tenant+ | Menghapus kunci publik SSH dari akun tenant. |
| `POST` | `/api/v1/terminal/session` | Tenant+ | Menginisialisasi token sesi PTY web terminal terisolasi. |
| `WS` | `/api/v1/terminal/ws` | Tenant+ | WebSocket bridge interaktif streaming input/output PTY xterm.js. |

### 4.8 Penjadwal Tugas (Cron Jobs)
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/cron/jobs` | Tenant+ | Daftar cron job aktif milik tenant. |
| `POST` | `/api/v1/cron/jobs` | Tenant+ | Mendaftarkan cron job baru dengan validasi ekspresi 5-field. |
| `PUT` | `/api/v1/cron/jobs/{id}` | Tenant+ | Memperbarui jadwal atau perintah cron job. |
| `DELETE`| `/api/v1/cron/jobs/{id}` | Tenant+ | Menghapus tugas cron. |
| `POST` | `/api/v1/cron/jobs/{id}/run` | Tenant+ | Memicu eksekusi manual seketika di luar jadwal. |

### 4.9 Optimasi Performa & Keamanan Host
| Metode | Jalur (Path) | Peran | Deskripsi |
| :--- | :--- | :--- | :--- |
| `GET` | `/api/v1/system/optimizer/status` | Admin | Analisis hardware dan rekomendasi tuning Nginx, Sysctl, & PHP-FPM. |
| `POST` | `/api/v1/system/optimizer/apply` | Admin | Menerapkan profil optimasi performa terpilih. |
| `GET` | `/api/v1/security/overview` | Admin | Ringkasan insiden keamanan, deteksi probing, dan brute force. |
| `POST` | `/api/v1/security/ip/ban` | Admin | Memblokir IP penyerang di level firewall nftables. |
| `POST` | `/api/v1/security/ip/unban` | Admin | Membuka blokir IP. |
| `GET` | `/api/v1/security/hardening/overview`| Admin | Laporan kepatuhan OWASP ASVS L2 dan skor systemd unit. |
| `POST` | `/api/v1/security/hardening/nftables/sync`| Admin | Sinkronisasi aturan isolasi loopback multi-tenant di kernel. |
