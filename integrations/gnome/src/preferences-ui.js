// SPDX-FileCopyrightText: 2026 Pavel Obruchnikov <me@inkyquill.net>
// SPDX-License-Identifier: GPL-3.0-or-later
import Gtk from 'gi://Gtk?version=4.0';
import Adw from 'gi://Adw?version=1';
import {Feed} from './feed.js';
import * as Mini from './mini.mjs';
import {text} from './strings.js';

/** Separate preferences process. Only explicit Save writes the six-slot setting. */
export class PreferencesController {
    constructor(window, settings) {
        this.window = window;
        this.settings = settings;
        this._base = settings.get_string('slot-config');
        this._draft = Mini.parseConfig(this._base);
        this._language = settings.get_string('language-cache') === 'ru-RU' ? 'ru-RU' : 'en-US';
        this._updating = false;
        this._saving = false;
        this._notice = '';
        this._closed = false;
        this.page = new Adw.PreferencesPage({icon_name: 'utilities-system-monitor-symbolic'});
        window.add(this.page);
        this.info = new Adw.PreferencesGroup();
        this.page.add(this.info);
        this.status = new Adw.ActionRow({activatable: false});
        this.info.add(this.status);
        this.rows = [];
        for (let index = 0; index < 6; index++) {
            const group = new Adw.PreferencesGroup();
            const reading = new Adw.ComboRow();
            const source = new Adw.ComboRow();
            group.add(reading); group.add(source); this.page.add(group);
            const row = {group, reading, source, kindSignature: '', sourceSignature: '', choices: []};
            this.rows.push(row);
            reading.connect('notify::selected', () => {
                if (this._updating || !this._draft) return;
                const kind = reading.selected ? Mini.kinds[reading.selected - 1] : null;
                this._draft.slots[index] = kind ? {kind, source: null} : null;
                this._notice = ''; this._render();
            });
            source.connect('notify::selected', () => {
                if (this._updating || !this._draft?.slots[index]) return;
                const choice = row.choices[source.selected];
                if (!choice) return;
                this._draft.slots[index].source = choice.id;
                this._notice = ''; this._render();
            });
        }
        this.actions = new Adw.PreferencesGroup();
        this.page.add(this.actions);
        const buttons = new Gtk.Box({orientation: Gtk.Orientation.HORIZONTAL, spacing: 8, margin_top: 8, margin_bottom: 8});
        this.saveButton = new Gtk.Button();
        this.reloadButton = new Gtk.Button();
        this.resetButton = new Gtk.Button();
        for (const button of [this.saveButton, this.reloadButton, this.resetButton]) buttons.append(button);
        this.actions.add(buttons);
        this.fps = new Adw.ActionRow({activatable: false});
        this.actions.add(this.fps);
        this.saveButton.connect('clicked', () => this.save());
        this.reloadButton.connect('clicked', () => this.reload());
        this.resetButton.connect('clicked', () => {this._draft = Mini.defaultConfig(); this._notice = ''; this._render();});
        this._changed = settings.connect('changed', () => {if (!this._saving) this._render();});
        this._close = window.connect('close-request', () => {this.destroy(); return false;});
        this.feed = new Feed();
        this.feed.start(() => this._render());
        this._render();
    }

    reload() {
        this._base = this.settings.get_string('slot-config');
        this._draft = Mini.parseConfig(this._base);
        this._notice = ''; this._render();
    }

    save() {
        if (!this._draft || this.settings.get_string('slot-config') !== this._base) {
            this._render(); return;
        }
        const encoded = JSON.stringify(this._draft);
        this._saving = true;
        try {
            if (!Mini.parseConfig(encoded) || !this.settings.set_string('slot-config', encoded))
                throw new Error('Unconfirmed write');
            this._base = this.settings.get_string('slot-config');
            if (this._base !== encoded) throw new Error('Readback changed');
            this._notice = 'saved';
        } catch { this._notice = 'failed'; }
        finally { this._saving = false; this._render(); }
    }

    _render() {
        if (this._closed) return;
        const frame = this.feed?.frame ?? null;
        const cached = this.settings.get_string('language-cache') === 'ru-RU' ? 'ru-RU' : 'en-US';
        this._language = Mini.languageOf(frame, cached);
        if (this._language !== cached) this.settings.set_string('language-cache', this._language);
        const language = this._language;
        this.page.title = Mini.text('configuration', language);
        this.info.description = `${text('description', language)} ${Mini.text('experimental', language)}`;
        const conflict = this.settings.get_string('slot-config') !== this._base;
        const message = !this._draft ? Mini.text('invalid', language) : conflict ? text('changed', language) : this._notice ? text(this._notice, language) : '';
        this.status.title = message;
        this.status.visible = !!message;
        this.status.use_markup = false;
        this._updating = true;
        try {
            for (let index = 0; index < 6; index++) {
                const row = this.rows[index], tile = this._draft?.slots[index];
                row.group.title = `${Mini.text('slot', language)} ${index + 1}`;
                row.reading.title = Mini.text('reading', language);
                row.source.title = Mini.text('source', language);
                row.reading.use_markup = row.source.use_markup = false;
                const names = [Mini.text('empty', language), ...Mini.kinds.map(kind => Mini.label(kind, language))];
                const kindSignature = JSON.stringify(names);
                if (row.kindSignature !== kindSignature) {row.reading.model = Gtk.StringList.new(names); row.kindSignature = kindSignature;}
                row.reading.selected = tile ? Mini.kinds.indexOf(tile.kind) + 1 : 0;
                row.reading.sensitive = !!this._draft;
                row.source.visible = !!tile && tile.kind !== 'ram';
                row.choices = tile ? [{id: null, label: Mini.text(tile.kind === 'network' ? 'total' : tile.kind === 'fps' ? 'unselected' : 'shared', language)}, ...Mini.sources(tile.kind, frame)] : [];
                if (tile?.source && !row.choices.some(choice => choice.id === tile.source))
                    row.choices.push({id: tile.source, label: `${Mini.text('missing', language)}: ${tile.source}`});
                const sourceSignature = JSON.stringify(row.choices);
                if (row.sourceSignature !== sourceSignature) {row.source.model = Gtk.StringList.new(row.choices.map(choice => choice.label)); row.sourceSignature = sourceSignature;}
                row.source.selected = tile && row.choices.length ? Math.max(0, row.choices.findIndex(choice => choice.id === tile.source)) : Gtk.INVALID_LIST_POSITION;
            }
        } finally { this._updating = false; }
        this.saveButton.label = text('save', language);
        this.saveButton.sensitive = !!this._draft && !conflict;
        this.reloadButton.label = text('reload', language);
        this.resetButton.label = Mini.text('reset', language);
        this.fps.title = Mini.text('fps', language);
    }

    destroy() {
        if (this._closed) return;
        this._closed = true;
        this.feed.stop();
        this.settings.disconnect(this._changed);
        // Widget callbacks are owned by the closing preferences window.
    }
}
