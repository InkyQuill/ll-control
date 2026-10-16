// SPDX-FileCopyrightText: 2026 Pavel Obruchnikov <me@inkyquill.net>
// SPDX-License-Identifier: GPL-3.0-or-later
import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';
import {PreferencesController} from './preferences-ui.js';

export default class LConnectMiniPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        window.set_default_size(720, 780);
        window._lconnectMini = new PreferencesController(window, this.getSettings());
    }
}
