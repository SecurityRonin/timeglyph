# Test data — provenance

Every artifact here carries a documented provenance entry (fleet standard):
source, identity/metadata, original download URL, checksums, contents,
redistribution/license, and the use case. Small, clearly-licensed fixtures are
committed; artifacts whose redistribution is unclear are **not committed** —
they are documented here and downloaded manually.

## `msgstore.db` — WhatsApp (Android) message store, synthetic sample

**Not stored in this repository.** The upstream source has no license, so the
file is not redistributed here — download it manually to reproduce local,
exploratory testing (it is git-ignored via `tests/data/.gitignore`):

```bash
curl -L -o tests/data/msgstore.db \
  https://raw.githubusercontent.com/trevordixon/whatsapp-msgstore-web-viewer/main/msgstore.db
# verify you got the same artifact this entry documents:
shasum -a 256 tests/data/msgstore.db   # -> 53a4d694…c175d2 (see manifest below)
```

| Field | Value |
|---|---|
| **Source** | Trevor Dixon — [`trevordixon/whatsapp-msgstore-web-viewer`](https://github.com/trevordixon/whatsapp-msgstore-web-viewer) (a demo fixture shipped with a WhatsApp `msgstore.db` web viewer). |
| **Downloaded** | 2026-07-02, from branch `main`. |
| **Original URL** | `https://raw.githubusercontent.com/trevordixon/whatsapp-msgstore-web-viewer/main/msgstore.db` |
| **Size** | 24 KB (24,576 bytes) |
| **MD5** | `3e7aeb9a9e2e59c0c72c5f56049c0e40` |
| **SHA-256** | `53a4d694b9a5159f9aca62b5e5e2d0f9b59404ae5e23c8bff915c11136c175d2` |
| **Format** | SQLite 3, modern WhatsApp/Android schema — tables `chat`, `jid`, `message`, `message_quoted`. |
| **Contents** | **Synthetic demo data.** 3 chats (one named "Family Group"), 9 generic dummy messages ("Hey! Are you free for coffee?", "Did you see the game last night?", …). No real names, phone numbers, or personal information. |
| **Timestamps** | `message.timestamp` in **Unix milliseconds**, spanning 2024-05-13T07:26:40Z → 2024-05-14T17:41:40Z. |
| **License / redistribution** | ⚠ **The upstream repository has no `LICENSE` file** — redistribution rights are unspecified (default: all rights reserved). The file is therefore **not committed to this repository**; it is documented here and downloaded manually (git-ignored). |
| **Use case** | Ground-truth for WhatsApp `msgstore.db` timestamp decoding: `message.timestamp` (Unix ms) should decode via timeglyph to the `unix_ms` reading — e.g. `1715708500000` → `2024-05-14T17:41:40Z`. |
| **Consumed by** | No automated test yet — manual/exploratory corpus. A validation test can read it env-gated (skip cleanly when absent). |

### Checksum manifest

```
3e7aeb9a9e2e59c0c72c5f56049c0e40  msgstore.db
```
