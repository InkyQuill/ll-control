#!/usr/bin/env python3
"""Record live-region events only on the keyboard fixture's private AT-SPI bus."""
import json
import os
from pathlib import Path

import gi

gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

root = Path(os.environ['LCONNECT_SMOKE_ROOT'])
assert os.environ.get('LCONNECT_KEYBOARD_ISOLATED') == '1'
assert (root / 'fixture-marker').is_file()
assert os.environ.get('AT_SPI_BUS_ADDRESS') == f'unix:path={root}/a11y.sock'
Atspi.set_timeout(500, 500)
Atspi.init()

with (root / 'accessibility-events.jsonl').open('w') as output:
    def record(event, _data):
        try:
            node = event.any_data if event.type == 'object:children-changed:add' and isinstance(event.any_data, Atspi.Accessible) else event.source
            node.clear_cache()
            application = node.get_application()
            if not application or application.get_name() != 'lconnect-gui':
                return
            attributes = node.get_attributes() or {}
            live = attributes.get('live') or attributes.get('container-live')
            if event.type == 'object:announcement':
                live = {int(Atspi.Live.POLITE): 'polite', int(Atspi.Live.ASSERTIVE): 'assertive'}.get(event.detail1)
            for _ in range(12):
                if live in ('polite', 'assertive'):
                    break
                node = node.get_parent()
                if node is None:
                    return
                node.clear_cache()
                attributes = node.get_attributes() or {}
                live = attributes.get('live') or attributes.get('container-live')
            if live not in ('polite', 'assertive'):
                return
            text = ''
            if 'Text' in node.get_interfaces():
                interface = node.get_text_iface()
                text = Atspi.Text.get_text(interface, 0, min(Atspi.Text.get_character_count(interface), 4096))
            row = {'type': event.type, 'name': node.get_name(), 'live': live,
                   'attributes': attributes, 'text': text,
                   'delta': event.any_data if isinstance(event.any_data, str) else None}
            output.write(json.dumps(row, ensure_ascii=False) + '\n')
            output.flush()
        except Exception as error:
            # Nodes can disappear before an asynchronous remove event is handled.
            print(f'Event source unavailable: {error}', flush=True)

    listener = Atspi.EventListener.new(record, None)
    for event_type in ['object:text-changed', 'object:children-changed', 'object:announcement']:
        assert listener.register(event_type), event_type
    (root / 'accessibility-events-ready').touch()
    Atspi.event_main()
