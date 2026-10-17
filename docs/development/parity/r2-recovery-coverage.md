# R2 recovery coverage and remaining checks

This ledger maps the operation requirement in [R2](release-milestones.md#r2--stabilize-the-existing-core-as-one-application)
to inspected assertions. It is not a completion claim. Native here means private
WebKitGTK/Tauri, Unix IPC and MockBackend; browser fixtures exercise UI state.
Run results are recorded in [core qualification](r2-core-qualification.md).

| Operation | Demonstrated recovery | Remaining check before closure |
|---|---|---|
| Edited/plain image publication, image replacement, video import/replacement | `native-interface-language.py`: seven bad readbacks and actual disconnect/10-second withheld reply in EN/RU; proxy verifies committed membership before damaging the reply; one publication, preserved adjacent membership/draft, no implicit Apply | Reconcile publication-time close with owner shutdown tests; do not treat closing a gallery panel as closing the app |
| Media preparation/publication | `native-media.py`: cancellation and normal close terminate/reap the owned encoder; `media_import.rs` tests hold preparation/publication during shutdown, fence new work and retain ambiguous publication status | Final workspace result must include these lifecycle tests; synthetic publication fixture is separate from real encoder cancellation |
| Image/video delete | `image-delete.spec.ts` / `media-delete.spec.ts`: lost reply refreshes catalog without replay, references/defaults protected; image panel close/reopen waits for settlement | Actual IPC disconnect/deadline and app-close outcome for delete have not yet been mapped; panel collapse is not app close |
| Profile Save/rename/reset/delete | `native-smoke.py`: proxy drops committed replies once; UI reads back library, preserves draft and does not replay. `profiles-interrupted.spec.ts`: precommit rejection, disconnect and late success/error with exact write/revision assertions | Operation-specific deadline/close coverage still needs reconciliation; generic brightness timeout is not reported as a profile timeout |
| LCD Apply/Clear (including loaded profiles) | Native lost-reply Apply/Clear and daemon restart; `display-interrupted.spec.ts`: reconnect discards stale result/error, preserves newer draft, explicit readback without automatic Apply | Map deadline/close to the shared bridge and daemon owner guarantees, or add missing operation-specific checks |
| Screen reset | `screen-reset.spec.ts`: uncertain receipt fences Apply/device switch, original-ID retry, stale rejection. Native keyboard: committed reset/output failure, explicit reapply. Daemon storage/lifecycle: caller loss, joined in-flight reset, replay/defaults and neighbors | Native deadline/reconnect UI recovery remains to be established; backend caller loss alone does not prove UI timeout behavior |
| FanAndLighting reset | `group_jobs_tests.rs`: real Unix reply loss, explicit Finish reads original receipt without another apply; shutdown joins admitted reset, prepared close leaves library intact; native reset survives GUI/daemon restart and newer manual intent | Explicit release test passed after the real 60-second worker deadline: Uncertain status, exact timeout diagnostic, original receipt and unchanged library/runtime revision after explicit retry. This is worker/IPC evidence; the ordinary bridge has a different 10-second deadline |
| Native profile/group archives | Native EN/RU export/import/readback/restart/normal close. `profile_archive_ipc.rs`: actual unconsumed Finish ACK, read-only status before idempotent retry, no resurrected reset slot. Group upload/Finish tests protect bindings and newer intent | Final worker tests must include prepared cancellation/shutdown, corrupt staging and uncertain cleanup |
| Optional Windows imports | Native Control/Screen EN/RU: cancel old grant, private preview, explicit Apply, original receipt after source deletion/restart. Corrupt media retains active frame/assets and releases private upload | Final `group_jobs_tests.rs` must include prepared/offline cancel and shutdown releasing private Screen preparation |

## Interpretation rules

A successful readback is not another mutation. One request/one receipt or unchanged
revision assertions establish that retries did not silently duplicate writes.
“Lost reply” is not automatically a measured deadline. The native proxy brightness
case proves the ordinary bridge deadline; media collection cases prove deadlines
for those operations. Neither is relabeled as the group worker's longer deadline.

Browser fixture errors prove UI handling of that error/result, not filesystem or
transport behavior. Storage owner tests prove commit/cleanup independently of
UI tests. A valid combined argument must name both and preserve their limits.

## Storage ownership reconciliation

The complete publication suite exercises staged assets, actual SIGKILL/startup,
manifest failure after link creation, foreign inode replacement, retained pins,
offline references, ambiguous commit retention and joined cleanup. The added
`ownership_link_collision_vetoes_publication_and_preserves_the_foreign_inode`
case forces actual `linkat` EEXIST, verifies unchanged manifest/no final asset,
then explicitly rolls back and reopens the store. It proves cleanup and subsequent
readback, not crash recovery at the collision point. That distinction was checked
in Rust review. Real write/fsync/rename failures, transaction installation crashes,
quotas and byte/frame maxima are covered by separate recorded R2 batches.
