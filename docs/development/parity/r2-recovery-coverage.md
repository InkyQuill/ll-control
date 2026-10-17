# R2 recovery coverage

This ledger maps the operation requirement in [R2](release-milestones.md#r2--stabilize-the-existing-core-as-one-application)
to inspected assertions. The final reconciliation below closes the software scope. Native here means private
WebKitGTK/Tauri, Unix IPC and MockBackend; browser fixtures exercise UI state.
Run results are recorded in [core qualification](r2-core-qualification.md).

| Operation | Demonstrated recovery | Final reconciliation / scope |
|---|---|---|
| Edited/plain image publication, image replacement, video import/replacement | `native-interface-language.py`: seven bad readbacks and actual disconnect/10-second withheld reply in EN/RU; proxy verifies committed membership before damaging the reply; one publication, preserved adjacent membership/draft, no implicit Apply | Closed by native encoder close plus sealed-publication and media-import owner shutdown tests in the full workspace run; combined evidence, not a native close injection at every publication stage |
| Media preparation/publication | `native-media.py`: cancellation and normal close terminate/reap the owned encoder; `media_import.rs` tests hold preparation/publication during shutdown, fence new work and retain ambiguous publication status | Both lifecycle tests passed in the final workspace run; synthetic publication fixture remains distinct from real encoder cancellation |
| Image/video delete | `image-delete.spec.ts` / `media-delete.spec.ts`: lost reply refreshes catalog without replay, references/defaults protected; image panel close/reopen waits for settlement | Native operation harness passed actual disconnect, measured 10-second deadline and normal app close after commit in EN/RU; one mutation, unchanged neighbors and daemon-restart readback |
| Profile Save/rename/reset/delete | `native-smoke.py`: proxy drops committed replies once; UI reads back library, preserves draft and does not replay. `profiles-interrupted.spec.ts`: precommit rejection, disconnect and late success/error with exact write/revision assertions | Native profile Save disconnect/deadline/app-close passed EN/RU with one revision, preserved neighboring slot and draft, explicit Refresh and restart readback. Rename/reset/delete retain the integrated lost-reply and browser interruption coverage |
| LCD Apply/Clear (including loaded profiles) | Native lost-reply Apply/Clear and daemon restart; `display-interrupted.spec.ts`: reconnect discards stale result/error, preserves newer draft, explicit readback without automatic Apply | Native Apply and Clear each passed disconnect/deadline/app-close in EN/RU, one mutation, correct active state, neighboring state and restart readback |
| Screen reset | `screen-reset.spec.ts`: uncertain receipt fences Apply/device switch, original-ID retry, stale rejection. Native keyboard: committed reset/output failure, explicit reapply. Daemon storage/lifecycle: caller loss, joined in-flight reset, replay/defaults and neighbors | Native disconnect/deadline/app-close passed EN/RU, original operation receipt, one revision, explicit result check, preserved draft and restart readback |
| FanAndLighting reset | `group_jobs_tests.rs`: real Unix reply loss, explicit Finish reads original receipt without another apply; shutdown joins admitted reset, prepared close leaves library intact; native reset survives GUI/daemon restart and newer manual intent | Explicit release test passed after the real 60-second worker deadline: Uncertain status, exact timeout diagnostic, original receipt and unchanged library/runtime revision after explicit retry. This is worker/IPC evidence; the ordinary bridge has a different 10-second deadline |
| Native profile/group archives | Native EN/RU export/import/readback/restart/normal close. `profile_archive_ipc.rs`: actual unconsumed Finish ACK, read-only status before idempotent retry, no resurrected reset slot. Group upload/Finish tests protect bindings and newer intent | Final workspace includes prepared cancellation/shutdown, corrupt staging and uncertain cleanup; native close and restart passed |
| Optional Windows imports | Native Control/Screen EN/RU: cancel old grant, private preview, explicit Apply, original receipt after source deletion/restart. Corrupt media retains active frame/assets and releases private upload | Final workspace passed prepared/offline cancel and shutdown releasing private Screen preparation; optional fidelity is still R4 |

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
