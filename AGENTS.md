# AGENTS.md

Baca `PRD (1).md` sebelum bekerja. Aturan ringkas:

1. Kerjakan satu task (ID `P<fase>-T<n>`) per sesi atau PR. Jangan loncat fase dan jangan mengerjakan task fase berikutnya "sekalian".
2. Develop dan test hanya di VM (Multipass / Lima / Vagrant, Ubuntu 24.04). Jangan pernah jalankan `panel-agent` di mesin host/laptop developer karena agent mengubah konfigurasi sistem (user, nginx, systemd, ACL).
3. `panel-api` TIDAK PERNAH dijalankan sebagai root. Agent hanya menerima command bertipe (`Request::UserCreate`, `Request::SiteApply`, dll). Tidak ada `Exec(String)` dan tidak ada path bebas mentah.
4. Dilarang: membangun command shell dengan string interpolation, memakai `unwrap()`/`expect()` di kode non-test, menerima path/command mentah dari API di agent, dan menambah dependency besar tanpa ADR.
5. Definition of Done:
   - Kode compile bersih dengan `cargo fmt --check` dan `cargo clippy -- -D warnings`.
   - Ada test (unit dan/atau integration) yang membuktikan acceptance criteria task.
   - Perubahan UI lolos `tsc`, lint, dan ada minimal satu test komponen atau e2e.
   - `docs/PROGRESS.md` diperbarui dan ADR dicatat di `docs/adr/` bila ada keputusan arsitektur.
6. Ambigu? Pilih default aman yang tertulis di PRD, catat ADR di `docs/adr/`, lanjutkan.
7. Desain UI mengikuti PRD bagian 8 (design tokens, font self-host, copy bahasa Indonesia sentence case yang jelas, aksesibilitas WCAG 2.2 AA).
8. Commit kecil dengan conventional commits (`feat(agent): ...`, `fix(web): ...`).
