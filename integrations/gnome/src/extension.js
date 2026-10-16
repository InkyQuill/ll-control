// SPDX-FileCopyrightText: 2026 Pavel Obruchnikov <me@inkyquill.net>
// SPDX-License-Identifier: GPL-3.0-or-later
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {Feed} from './feed.js';
import * as Mini from './mini.mjs';
import {text} from './strings.js';

export default class LConnectMini extends Extension {
    enable() {
        this._settings = this.getSettings();
        this._indicator = new PanelMenu.Button(0.0, 'L-Connect Mini', false);
        this._summary = new St.Label({text: 'L-Connect', y_align: Clutter.ActorAlign.CENTER});
        this._indicator.add_child(this._summary);
        this._status = new PopupMenu.PopupMenuItem('', {reactive: false, can_focus: false});
        this._indicator.menu.addMenuItem(this._status);
        this._rows = Array.from({length: 6}, () => {
            const row = new PopupMenu.PopupMenuItem('', {reactive: false, can_focus: false});
            this._indicator.menu.addMenuItem(row);
            return row;
        });
        this._indicator.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        this._preferences = this._indicator.menu.addAction('Preferences', () => this.openPreferences());
        Main.panel.addToStatusArea(this.uuid, this._indicator);
        this._changed = this._settings.connect('changed', () => this._update());
        this._feed = new Feed();
        this._feed.start(() => this._update());
        this._update();
    }

    _update() {
        if (!this._indicator || !this._feed)
            return;
        const cached = this._settings.get_string('language-cache') === 'ru-RU' ? 'ru-RU' : 'en-US';
        const view = Mini.render(this._feed.frame, this._settings.get_string('slot-config'), this._feed.elapsedMs, cached);
        if (view.language !== cached)
            this._settings.set_string('language-cache', view.language);
        this._status.label.text = view.status;
        this._status.visible = !!view.status;
        const summary = [];
        for (let i = 0; i < 6; i++) {
            const card = view.cards[i];
            this._rows[i].label.text = `${card.title}: ${card.available ? card.values.join(' / ') : '—'}`;
            if (card.available && summary.length < 2)
                summary.push(`${card.title} ${card.values[0]}`);
        }
        this._summary.text = summary.join(' · ') || 'L-Connect';
        this._indicator.accessible_name = view.status || summary.join(' · ') || 'L-Connect Mini';
        this._preferences.label.text = text('preferences', view.language);
    }

    disable() {
        this._feed?.stop();
        this._feed = null;
        if (this._changed)
            this._settings?.disconnect(this._changed);
        this._changed = 0;
        this._indicator?.destroy();
        this._indicator = this._summary = this._status = this._preferences = null;
        this._rows = null;
        this._settings = null;
    }
}
