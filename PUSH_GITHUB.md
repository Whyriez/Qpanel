# GitHub Push Commands — Nusa Panel

> Jalankan dari root repository.

## Push normal

```bash
git status
git pull --rebase origin main
git add -A
git status
git commit -m "fix(installer): make fresh deployment production-ready"
git push origin main
```

Kalau automated release sudah aktif, berhenti di sini. Release Please akan membuat/memperbarui Release PR.

## Kalau automated release BELUM aktif

Setelah push ke `main`, buat tag manual:

```bash
git tag -a v0.1.1 -m "Nusa Panel v0.1.1"
git push origin v0.1.1
```

## Bersihkan file yang seharusnya di-ignore

Jalankan sekali jika file ini sudah pernah masuk staging/tracking Git:

```bash
git rm -r --cached --ignore-unmatch docs

git rm --cached --ignore-unmatch   AGENTS.md   BOOTSTRAP_INSTALLER.md   "PRD (1).md"   PRD.md   FIX_V0.1.1.md   GITHUB_AUTOMATED_RELEASE.md

git add .gitignore
git status
```

`--cached` tidak menghapus file lokal.

## Cek apa yang akan di-commit

```bash
git status
git diff --cached
```

## Kalau sudah commit tapi belum push

```bash
git push origin main
```

## Kalau ada perubahan tambahan

```bash
git add -A
git commit -m "fix: complete release fixes"
git push origin main
```

## Cek commit terakhir

```bash
git log --oneline -5
```

## Cek tag

```bash
git tag --sort=-version:refname | head
git ls-remote --tags origin
```

## One-liner

```bash
git pull --rebase origin main && git add -A && git status && git commit -m "fix(installer): make fresh deployment production-ready" && git push origin main
```

## Conventional Commit singkat

Bug fix:

```bash
git commit -m "fix: deskripsi perubahan"
```

Feature:

```bash
git commit -m "feat: deskripsi fitur"
```

Docs:

```bash
git commit -m "docs: update documentation"
```

Maintenance:

```bash
git commit -m "chore: maintenance"
```

Untuk release sekarang:

```bash
git commit -m "fix(installer): make fresh deployment production-ready"
```
