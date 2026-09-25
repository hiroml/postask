---
id: ADR-001
title: 技術スタックの選定
status: accepted
date: 2026-09-25
---

# ADR-001: 技術スタックの選定

## 状況

X クローンをデスクトップアプリとして実装する。フロントからバックエンドまで Rust で統一したい。

## 決定

- デスクトップシェル: Tauri 2
- フロントエンド: Leptos 0.8（CSR, WASM）、ビルドは Trunk
- 永続化: SQLite（rusqlite, bundled）

## 影響

- フロントは WASM のため、ネイティブ処理（DB 等）はすべて Tauri コマンド経由で行う
- `backend/` クレートは現時点で未使用
