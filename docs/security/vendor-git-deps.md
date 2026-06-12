# C4 — Vendor personal git dependencies

## Vấn đề

`src-tauri/Cargo.toml` hiện phụ thuộc vào 4 repo cá nhân trên GitHub.
Nếu maintainer xóa hoặc force-push repo đó → build sẽ fail và không thể khôi phục.

```toml
rdev       = { git = "https://github.com/rustdesk-org/rdev", rev = "a90dbe11…" }
vad-rs     = { git = "https://github.com/cjpais/vad-rs",     rev = "88b3a01f…" }
rodio      = { git = "https://github.com/cjpais/rodio",      rev = "fed30292…" }
tauri-nspanel = { git = "https://github.com/ahkohd/tauri-nspanel", rev = "da9c9a8d…" }
```

## Hành động cần thực hiện (manual — cần GitHub account)

### Bước 1: Fork các repo về org của bạn

```bash
# Thực hiện trên https://github.com — nút "Fork" trên mỗi repo:
# https://github.com/rustdesk-org/rdev          → fork thành chaudl113/rdev
# https://github.com/cjpais/vad-rs              → fork thành chaudl113/vad-rs
# https://github.com/cjpais/rodio               → fork thành chaudl113/rodio
# https://github.com/ahkohd/tauri-nspanel       → fork thành chaudl113/tauri-nspanel
```

### Bước 2: Cập nhật Cargo.toml trỏ về fork

```toml
rdev = { git = "https://github.com/chaudl113/rdev", rev = "a90dbe11…" }
vad-rs = { git = "https://github.com/chaudl113/vad-rs", rev = "88b3a01f…" }
rodio = { git = "https://github.com/chaudl113/rodio", rev = "fed30292…" }
tauri-nspanel = { git = "https://github.com/chaudl113/tauri-nspanel", rev = "da9c9a8d…" }
```

### Bước 3: Verify build vẫn pass

```bash
cd src-tauri && cargo build
```

### Bước 4: Lock Cargo.lock

Commit `src-tauri/Cargo.lock` sau khi fork để pin đúng revision.

## Lý do không tự động hóa được

Fork repo yêu cầu GitHub OAuth token với quyền `repo` — không thể thực hiện trong CI mà không có credentials người dùng.

## Rủi ro hiện tại

| Dep | Fork owner | Risk |
|-----|-----------|------|
| rdev | rustdesk-org | Org tích cực, thấp |
| vad-rs | cjpais | Personal account, trung bình |
| rodio | cjpais | Personal fork của rodio chính, trung bình |
| tauri-nspanel | ahkohd | Personal, repo nhỏ, trung bình-cao |
