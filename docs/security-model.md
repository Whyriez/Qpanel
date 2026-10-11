# Model Keamanan & Threat Modeling Nusa Panel

Dokumen ini menjelaskan arsitektur keamanan, model ancaman (*threat model*), mekanisme mitigasi, dan standar pengerasan (*hardening*) yang diterapkan di Nusa Panel sesuai standar OWASP ASVS (*Application Security Verification Standard*) Level 2.

---

## 1. Filosofi Keamanan: Pertahanan Berlapis (*Defense-in-Depth*)

Nusa Panel dibangun dengan asumsi bahwa **aplikasi web tenant pada akhirnya dapat disusupi** (misalnya melalui kerentanan plugin CMS pihak ketiga, SQL injection pada skrip pengguna, atau kredensial yang bocor). Oleh karena itu, arsitektur Nusa Panel tidak hanya melindungi panel itu sendiri, tetapi membatasi dampak intrusi agar:
1. Penyerang **TIDAK BISA** melakukan eskalasi hak akses ke pengguna sistem lain atau root (*Zero Lateral Movement*).
2. Penyerang **TIDAK BISA** membaca data sensitif milik situs atau basis data tenant lain (*Zero Cross-Tenant Leakage*).
3. Penyerang **TIDAK BISA** melumpuhkan server secara keseluruhan (*Resource Exhaustion Prevention*).

---

## 2. Threat Modeling & Analisis Vektor Ancaman

Tabel berikut merangkum vektor serangan utama dan mitigasi bawaan di Nusa Panel:

| Vektor Ancaman | Deskripsi Skenario | Mitigasi Nusa Panel |
| :--- | :--- | :--- |
| **Penyusupan Situs Tenant (RCE)** | Penyerang mengunggah webshell ke situs WordPress/PHP tenant. | Tenant berjalan di bawah UID unik (`p_<tenant>`), home directory izin `0700`, ACL terbatas, proses diisolasi cgroup v2. |
| **Eskalasi Hak Akses ke Root** | Penyerang mencoba memanfaatkan kerentanan biner lokal atau panel API untuk menjadi root. | `panel-api` berjalan sebagai non-root (`panel-api:panel`). Soket daemon `panel-agent` hanya menerima perintah bertipe; tidak ada eksekusi shell bebas (`Exec(String)`). |
| **Pengintaian Proses Antar-Tenant** | Skrip jahat menjalankan `ps aux` untuk memantau aktivitas proses tenant lain atau token sensitif. | File system `/proc` dipasang dengan opsi `hidepid=2,gid=panel`. Pengguna tenant hanya dapat melihat proses milik dirinya sendiri. |
| **Penetrasi Jaringan Loopback (Local SSRF)** | Skrip tenant melakukan *port scanning* ke `127.0.0.1` untuk mengakses API lokal, Redis, atau port aplikasi tenant lain. | Aturan kernel `nftables` memfilter paket loopback berdasarkan UID (`meta skuid`), memblokir akses antar-port pengguna Linux yang berbeda. |
| **Serangan DoS / Starvation Sumber Daya** | Serangan *fork bomb*, konsumsi 100% memori, atau flooding I/O disk oleh satu penyewa. | cgroup v2 controller menetapkan batas ketat pada `pids.max`, `memory.max`, `cpu.max`, dan `io.weight` per tenant slice. |
| **Manipulasi Path Traversal** | Penyerang mengirim parameter jalur file `../../../../etc/shadow` ke Web File Manager. | Validasi canonical path ketat (`std::fs::canonicalize`) dengan verifikasi bahwa jalur tujuan berada di bawah akar direktori tenant. |
| **Credential Stuffing / Brute Force** | Penyerang mencoba ribuan kata sandi pada formulir login panel admin. | Rate limiting terpadu, algoritma hashing Argon2id berbiaya komputasi tinggi, penguncian akun progresif, dan kewajiban TOTP 2FA. |
| **Kebocoran Kunci Rahasia / Database Vault** | Penyerang membaca cadangan basis data panel SQLite. | Kredensial sensitif dienkripsi menggunakan ChaCha20-Poly1305 AEAD dengan kunci master yang disimpan terpisah dari file database. |

---

## 3. Pemisahan Hak Istimewa (Privilege Separation)

### 3.1 Arsitektur Non-Root API
Sebagian besar kontrol panel konvensional menjalankan daemon web langsung sebagai `root`. Pendekatan ini merupakan risiko fatal: satu celah Remote Code Execution (RCE) di web framework akan langsung memberikan kendali penuh atas server kepada penyerang.

Nusa Panel memisahkan tanggung jawab menjadi dua lapisan:
- **`panel-api`**: Berjalan sebagai pengguna non-root `panel-api:panel` tanpa hak akses `sudo`. Layanan ini menangani seluruh parsing HTTP yang kompleks, upload file web, dan interaksi peramban.
- **`panel-agent`**: Berjalan sebagai `root` tetapi **TIDAK TERHUBUNG** ke jaringan internet. Agent hanya mendengarkan pada soket lokal UNIX Domain Socket di `/run/panel/agent.sock` dengan izin berkas `0660` milik `root:panel`.

```
[ Internet ] ──► [ panel-api (Non-Root) ] ──[ UNIX Socket: Typed RPC ]──► [ panel-agent (Root) ]
```

### 3.2 Larangan String Interpolation & Shell Execution
Di seluruh basis kode Nusa Panel, aturan berikut ditegakkan oleh compiler dan linter CI:
- **Dilarang memanggil `sh -c` atau `bash -c` dengan string yang mengandung input pengguna.**
- Semua eksekusi program sistem wajib memisahkan biner dan argumen secara eksplisit:
  ```rust
  // Benar dan Aman: Argumen dipisahkan, tidak ada interpretasi shell
  Command::new("/usr/sbin/useradd")
      .arg("-m")
      .arg("-u").arg(uid.to_string())
      .arg("-s").arg("/bin/bash")
      .arg(&validated_username)
      .output()?;
  ```
- Komunikasi IPC menggunakan Rust Enum bertipe. Tidak ada tipe pesan `ExecuteArbitraryCommand(String)`.

---

## 4. Isolasi Multi-Tenant Berlapis

### 4.1 Linux POSIX User & Permissions
- Setiap akun tenant memiliki identitas Linux tersendiri dengan pola nama `p_<username>` dan UID >= 1000.
- Direktori beranda dibuat dengan izin `0700` (hanya dapat dibaca/ditulis oleh pemilik akun).
- Akses untuk Nginx web server dikelola via POSIX Access Control Lists (ACL):
  ```bash
  setfacl -m u:www-data:rx /home/p_<username>
  setfacl -R -m u:www-data:rx /home/p_<username>/sites/<site_name>/public
  ```
  Dengan konfigurasi ini, tenant lain sama sekali tidak memiliki izin lintasan (*traverse*) ke direktori tersebut.

### 4.2 Isolasi Visibilitas Proses (`hidepid=2`)
Konfigurasi kernel mount:
```bash
mount -o remount,rw,hidepid=2,gid=panel /proc
```
Hasil:
- Tenant `p_budi` yang membuka terminal web hanya melihat proses miliknya sendiri.
- Upaya melihat proses tenant lain menghasilkan *empty output*.
- Informasi sensitif (seperti parameter command line atau environment variable aplikasi lain) tidak dapat diintip.

### 4.3 Isolasi Sumber Daya via Linux cgroup v2
Setiap tenant ditempatkan pada slice tersendiri:
`/sys/fs/cgroup/panel.slice/tenant-<username>.slice`

| Pengendali | File Pembatas | Penjelasan Keamanan |
| :--- | :--- | :--- |
| **PIDs** | `pids.max` | Mencegah *fork bomb* yang dapat melumpuhkan tabel proses kernel host. |
| **Memori** | `memory.max` & `memory.high` | Mencegah Out-Of-Memory (OOM) global. Jika batas terlampaui, hanya proses tenant bersangkutan yang terkena OOM killer. |
| **CPU** | `cpu.max` | Membatasi alokasi core CPU agar tidak ada satu tenant yang memonopoli 100% kapasitas server. |
| **I/O** | `io.weight` | Mencegah kelebihan antrean baca/tulis disk yang memperlambat situs lain. |

### 4.4 Isolasi Loopback Kernel (nftables)
Untuk mengatasi celah umum pada shared hosting di mana tenant dapat menghubungkan skripnya ke port internal tenant lain, Nusa Panel menyuntikkan aturan loopback filter:
```nftables
table inet panel_filter {
    chain input {
        type filter hook input priority 0; policy accept;
        # Tolak akses loopback jika UID sumber tidak cocok dengan UID target layanan
        iif lo meta skuid >= 1000 meta skuid != @authorized_uids drop
    }
}
```

---

## 5. Autentikasi, Otorisasi, & Sesi

### 5.1 Hashing Kata Sandi: Argon2id
Nusa Panel menggunakan algoritma hashing **Argon2id** (pemenang Password Hashing Competition) dengan parameter standar tinggi:
- **Memory Cost ($m$)**: 65.536 KiB (64 MiB).
- **Time Cost ($t$)**: 3 iterasi.
- **Parallelism ($p$)**: 4 thread.
- **Salt**: 16 byte acak kriptografis (CSPRNG) yang dihasilkan per kata sandi.

### 5.2 Autentikasi Dua Faktor (TOTP 2FA)
- Mengikuti spesifikasi RFC 6238 berbasis algoritma HMAC-SHA1 dengan jendela waktu 30 detik.
- Secret key TOTP disimpan dalam format terenkripsi di vault.
- Dilengkapi dengan 8 kode pemulihan (*backup recovery codes*) sekali pakai (*single-use*).

### 5.3 Otorisasi Berbasis Peran (RBAC)
Akses API divalidasi pada setiap endpoint menggunakan tiga tingkatan peran:
1. **Admin**: Hak penuh atas konfigurasi server, manajemen paket, pengguna, dan backup sistem.
2. **Operator**: Hak operasional terbatas (pemantauan metrik, melihat log, mengelola situs tanpa akses ke rahasia sistem).
3. **Tenant**: Hak terbatas penuh hanya pada sumber daya yang dimiliki sendiri (`owner_id == authenticated_user_id`).

---

## 6. Kriptografi & Vault Rahasia

### 6.1 ChaCha20-Poly1305 AEAD
Data sensitif (token API, private key SSL kustom, password database, environment variables) dienkripsi sebelum disimpan ke SQLite:
- **Algoritma**: ChaCha20-Poly1305 (IETF RFC 8439), cipher aliran berkinerja tinggi dengan integritas terotentikasi (*Authenticated Encryption with Associated Data*).
- **Nonce**: 96-bit nonce unik yang dihasilkan oleh CSPRNG untuk setiap operasi enkripsi.
- **Key Derivation**: Kunci enkripsi diturunkan dari master key menggunakan HKDF-SHA256 dengan string konteks domain eksplisit (`vpanel-vault-encryption-v1`).

### 6.2 Rotasi Kunci Tanpa Downtime
Nusa Panel mendukung rotasi kunci master (*Dual-Key Rotation*):
1. Sistem mengenali `Current Master Key` dan `Previous Master Key`.
2. Saat proses dekripsi, sistem mencoba kunci aktif terlebih dahulu; jika gagal, sistem mencoba kunci sebelumnya.
3. Job latar belakang secara transparan membaca kembali rekaman lama, mendekripsinya dengan kunci lama, mengenkripsinya kembali dengan kunci baru, dan memperbarui basis data tanpa gangguan operasional.

---

## 7. Keamanan Jaringan & Web (OWASP ASVS L2)

- **Strict Transport Security (HSTS)**: Dikirimkan dengan header `Strict-Transport-Security: max-age=31536000; includeSubDomains; preload`.
- **Content Security Policy (CSP)**: Mencegah serangan Cross-Site Scripting (XSS) dengan membatasi sumber eksekusi skrip.
- **Proteksi Frame & Sniffing**: Header `X-Frame-Options: SAMEORIGIN` dan `X-Content-Type-Options: nosniff`.
- **Mitigasi SSRF pada Reverse Proxy**:
  - Validasi URL target secara ketat.
  - Memblokir forwarding ke rentang IP privat/internal (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.169.254` cloud metadata), kecuali secara eksplisit diizinkan oleh administrator.
- **Audit Logging yang Tidak Dapat Dimanipulasi**:
  - Setiap tindakan administratif dicatat dalam tabel audit log *append-only*.
  - Log mencakup: timestamp UTC, actor ID, alamat IP asal, jenis aksi, entitas target, dan status keberhasilan.

---

## 8. Prosedur Tanggap Insiden Malware Tenant

Jika satu situs tenant terindikasi mengirimkan spam atau terinfeksi webshell:
1. **Isolasi Cepat**:
   - Jalankan `panelctl site suspend <domain>` atau ubah status situs di antarmuka menjadi *Suspended*.
   - Unit systemd dan Nginx vhost situs langsung dinonaktifkan seketika.
2. **Karantina Berkas**:
   - Berkas situs di `/home/p_<user>/sites/<domain>` dapat diubah izinnya menjadi `0000` untuk mencegah eksekusi lebih lanjut.
3. **Pembersihan & Investigasi**:
   - Periksa Access Log di `/var/log/panel/nginx/<domain>-access.log` untuk menemukan vektor injeksi awal.
   - Karena isolasi `hidepid=2` dan cgroup, situs lain di server dipastikan aman dari kompromi lanjutan.
