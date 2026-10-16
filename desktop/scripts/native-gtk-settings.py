#!/usr/bin/env python3
"""Read effective GTK font settings only inside the private native fixture."""
import json
import os
from pathlib import Path

assert os.environ.get('LCONNECT_KEYBOARD_ISOLATED') == '1'
root = Path(os.environ['LCONNECT_SMOKE_ROOT'])
assert root.is_absolute() and (root / 'fixture-marker').is_file()
assert os.environ['XDG_CONFIG_HOME'] == str(root / 'config')
assert os.environ['XDG_RUNTIME_DIR'] == str(root)

import gi

gi.require_version('Gtk', '3.0')
from gi.repository import Gtk

Gtk.init([])
settings = Gtk.Settings.get_default()
print(json.dumps({'font': settings.get_property('gtk-font-name'),
                  'dpi': settings.get_property('gtk-xft-dpi')}))
