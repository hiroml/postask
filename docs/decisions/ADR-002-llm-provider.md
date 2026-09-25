---
id: ADR-002
title: LLM プロバイダの抽象化（Claude から始め、ローカル LLM に差し替え可能にする）
status: accepted # Q-001 で決定
date: 2026-09-25
---

# ADR-002: LLM プロバイダの抽象化

## 状況

ポストの分類（F2）と AI リプライ（F4）に LLM を使う。
まずは Claude API で品質を確かめ、あとからローカル LLM（Ollama など）も試したい。

## 選択肢

1. Claude API を直接呼ぶコードを書く — 手早いが、後で差し替えるとき呼び出し箇所をすべて直すことになる
2. `LlmProvider` トレイトで抽象化する — 最初に少し手間がかかるが、差し替えが設定変更だけで済み、テスト用のモックも作れる
3. OpenAI 互換 API に揃えて1つの実装にする — ローカル LLM とは相性が良いが、Claude の構造化出力などの機能を使えなくなる

## 決定

案2を採用する。

- 用途ごとのメソッド（`classify`、後で `reply` など）を持つ `LlmProvider` トレイトを `src-tauri/src/llm/` に定義する
- 実装: `ClaudeProvider`（最初に作る）/ `MockProvider`（テスト用）/ `LocalLlmProvider`（後で作る）
- どのプロバイダを使うかは起動時の設定で選ぶ
- 出力の形（JSON スキーマ）はアプリ側で決めて、各プロバイダがそれに合わせる

## 影響

- 分類ロジックのテストは `MockProvider` で行えるので、API を呼ばず（料金もかからず）に済む
- ローカル LLM は構造化出力の対応がモデルによって違うので、`LocalLlmProvider` では JSON の検証とリトライを厚めに作る必要がある
- Claude 固有の機能（effort、フォールバックなど）は `ClaudeProvider` の中だけで使う
