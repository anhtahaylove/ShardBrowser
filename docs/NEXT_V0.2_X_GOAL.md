# ShardBrowser v0.2.x goal — superseded draft pointer

> **Superseded draft:** nội dung goal cũ trong file này không còn là implementation hoặc release authority.

- Runtime/baseline được giữ nguyên ở **v0.1.28**.
- Canonical v0.2.x goal, PRD, ADR, gates và handoff nằm tại [`.omx/plans/shardbrowser-v0.2.x-team-fleet-encrypted-backup.md`](../.omx/plans/shardbrowser-v0.2.x-team-fleet-encrypted-backup.md).
- Hard gates còn mở được theo dõi tại [`.omx/plans/open-questions.md`](../.omx/plans/open-questions.md).
- Current design status: Architect `APPROVE` và Critic `APPROVE` đã persist thành durable consensus; G2 chưa chạy, runtime v0.1.28 chưa đổi.
- Formal order: persisted Architect accept → persisted Critic approve → durable consensus complete → bounded G2 spike lane → independent verifier PASS → production implementation.
- Không được dùng old **v0.1.27** text đã bị thay thế để implementation, commit, tag, publish, release hoặc production migration.

Pointer này không tự cấp execution hoặc release authority.
