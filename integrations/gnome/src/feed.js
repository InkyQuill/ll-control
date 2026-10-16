// SPDX-FileCopyrightText: 2026 Pavel Obruchnikov <me@inkyquill.net>
// SPDX-License-Identifier: GPL-3.0-or-later
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const MAX_LINE = 1024 * 1024;
const unavailable = () => ({version: 1, available: false, data: null});

/** One owned, fixed read-only CLI helper. No shell, hardware collector or GUI dependency. */
export class Feed {
    constructor() {
        this.frame = unavailable();
        this._active = false;
        this._generation = 0;
        this._process = null;
        this._cancel = null;
        this._retry = 0;
        this._clock = 0;
        this._received = 0;
        this._pending = new Set();
        this._waiters = [];
        this._change = null;
    }

    get helperPid() { return this._process?.get_identifier() ?? null; }
    get elapsedMs() { return this._received ? Math.max(0, (GLib.get_monotonic_time() - this._received) / 1000) : Number.MAX_SAFE_INTEGER; }

    start(onChange) {
        if (this._active)
            return;
        this._active = true;
        this._change = onChange;
        this._clock = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 250, () => {
            if (this.elapsedMs > 5000) {
                this.frame = unavailable();
                this._received = 0;
            }
            this._notify();
            return GLib.SOURCE_CONTINUE;
        });
        this._launch();
    }

    stop() {
        this._active = false;
        this._generation++;
        this._change = null;
        for (const id of [this._retry, this._clock]) {
            if (id)
                GLib.Source.remove(id);
        }
        this._retry = this._clock = 0;
        this._cancel?.cancel();
        this._cancel = null;
        const process = this._process;
        this._process = null;
        process?.force_exit();
        this.frame = unavailable();
        this._received = 0;
    }

    async shutdown() {
        this.stop();
        if (this._pending.size)
            await new Promise(resolve => this._waiters.push(resolve));
    }

    _notify() {
        try { this._change?.(this.frame, this.elapsedMs); }
        catch (error) { console.error(error); }
    }

    _offline() {
        this.frame = unavailable();
        this._received = 0;
        this._notify();
    }

    _schedule() {
        if (!this._active || this._retry)
            return;
        this._retry = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1000, () => {
            this._retry = 0;
            this._launch();
            return GLib.SOURCE_REMOVE;
        });
    }

    _launch() {
        if (!this._active || this._process)
            return;
        const executable = GLib.find_program_in_path('lconnectctl');
        if (!executable) {
            this._offline();
            this._schedule();
            return;
        }
        const generation = ++this._generation;
        let process;
        try {
            process = Gio.Subprocess.new([executable, 'mini', 'watch'],
                Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_SILENCE);
        } catch {
            this._offline();
            this._schedule();
            return;
        }
        this._process = process;
        this._cancel = new Gio.Cancellable();
        this._pending.add(process);
        process.wait_async(null, (child, result) => {
            try { child.wait_finish(result); } catch (error) { console.error(error); }
            this._pending.delete(child);
            if (this._active && generation === this._generation && this._process === child) {
                this._process = null;
                this._cancel?.cancel();
                this._cancel = null;
                this._offline();
                this._schedule();
            }
            if (!this._pending.size) {
                for (const resolve of this._waiters.splice(0))
                    resolve();
            }
        });
        const current = () => this._active && generation === this._generation && this._process === process;
        const stream = process.get_stdout_pipe();
        const cancel = this._cancel;
        let parts = [], size = 0;
        const decoder = new TextDecoder('utf-8', {fatal: true});
        const fail = () => {
            if (current()) {
                this._offline();
                cancel.cancel();
                process.force_exit();
            }
        };
        const read = () => stream.read_bytes_async(16384, GLib.PRIORITY_DEFAULT, cancel, (input, result) => {
            let bytes;
            try { bytes = input.read_bytes_finish(result).toArray(); }
            catch {
                if (current()) fail();
                else { try { input.close(null); } catch { /* Completed cancelled read. */ } }
                return;
            }
            if (!current()) {
                try { input.close(null); } catch { /* No UI references after stop. */ }
                return;
            }
            if (!bytes.length) { fail(); return; }
            try {
                let start = 0;
                while (start < bytes.length) {
                    const newline = bytes.indexOf(10, start);
                    const end = newline < 0 ? bytes.length : newline;
                    const part = bytes.subarray(start, end);
                    size += part.length;
                    if (size > MAX_LINE) throw new Error('Mini frame exceeds budget');
                    parts.push(part);
                    if (newline >= 0) {
                        const line = new Uint8Array(size);
                        let offset = 0;
                        for (const part of parts) { line.set(part, offset); offset += part.length; }
                        const frame = JSON.parse(decoder.decode(line));
                        if (!frame || frame.version !== 1 || typeof frame.available !== 'boolean' ||
                            (frame.available && (!frame.data || typeof frame.data !== 'object' || Array.isArray(frame.data))))
                            throw new Error('Invalid Mini frame');
                        parts = []; size = 0;
                        this.frame = frame;
                        this._received = GLib.get_monotonic_time();
                        this._notify();
                    }
                    start = end + (newline >= 0 ? 1 : 0);
                }
            } catch { fail(); return; }
            if (current()) read();
        });
        read();
    }
}
