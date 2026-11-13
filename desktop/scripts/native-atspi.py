#!/usr/bin/env python3
"""Inspect only the accessibility bus owned by the isolated keyboard fixture."""
import json
import os
import sys
from pathlib import Path
import gi

root = Path(os.environ['LLCONTROL_SMOKE_ROOT'])
assert os.environ.get('LLCONTROL_KEYBOARD_ISOLATED') == '1'
assert (root / 'fixture-marker').is_file()
assert os.environ.get('AT_SPI_BUS_ADDRESS') == f'unix:path={root}/a11y.sock'
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi
Atspi.set_timeout(500, 500)
Atspi.init()
rows = []
def walk(node, depth=0):
    if depth > 25 or len(rows) > 1000:
        raise RuntimeError('Unexpected accessibility tree size')
    state = node.get_state_set()
    rows.append({'role': node.get_role_name(), 'name': node.get_name(), 'depth': depth,
                 'focused': state.contains(Atspi.StateType.FOCUSED),
                 'enabled': state.contains(Atspi.StateType.ENABLED),
                 'invalid': state.contains(Atspi.StateType.INVALID_ENTRY),
                 'interfaces': node.get_interfaces(), 'attributes': node.get_attributes()})
    if 'Text' in rows[-1]['interfaces']:
        text = node.get_text_iface()
        rows[-1]['text'] = Atspi.Text.get_text(text, 0, min(Atspi.Text.get_character_count(text), 4096))
    if 'Value' in rows[-1]['interfaces']:
        rows[-1]['value'] = {'current': node.get_current_value(), 'minimum': node.get_minimum_value(), 'maximum': node.get_maximum_value()}
    for index in range(node.get_child_count()):
        child = node.get_child_at_index(index)
        if child is not None:
            walk(child, depth + 1)
desktop = Atspi.get_desktop(0)
apps = [desktop.get_child_at_index(i) for i in range(desktop.get_child_count())]
app = next((app for app in apps if app and app.get_name() == 'llcontrol-gui'), None)
assert app is not None, 'Native app missing from AT-SPI'
walk(app)
connection = [row for row in rows if row['attributes'].get('id') == 'connection-status']
assert len(connection) == 1, 'Stable connection live region missing from AT-SPI'
assert connection[0]['attributes'].get('live') == 'polite' and connection[0]['attributes'].get('atomic') == 'true', connection
assert 'Daemon подключён' in connection[0].get('text', ''), connection
page = sys.argv[1] if len(sys.argv) > 1 else 'settings'
requirements = {
    'reset': ([('button', 'Повторно применить к LCD')], ('button', 'Проверить результат сброса')),
    'settings': ([('combo box', 'Ориентация'), ('button', 'Применить яркость'), ('button', 'Применить поворот')], ('slider', 'Яркость LCD:')),
    'cooling': ([('combo box', 'Канал'), ('button', 'Применить охлаждение')], ('spin button', 'PWM, %')),
    'lighting': ([('button', 'Применить подсветку')], ('slider', 'Яркость RGB:')),
    'sensors': ([('button', 'Применить источники')], ('combo box', 'CPU · Температура')),
    'display': ([('button', '+ Текст')], ('button', 'Применить к LCD')),
}
variant = 'curve' if page == 'cooling' and any(row['name'] == 'Точка 1: °C' for row in rows) else 'main'
if page == 'cooling' and any(row['name'] == 'Целевые RPM (reference mapping)' for row in rows):
    variant = 'rpm'
    requirements['cooling'] = ([('button', 'Применить охлаждение')], ('spin button', 'Целевые RPM (reference mapping)'))
if variant == 'curve' and any(row['name'] == 'RPM' for row in rows):
    variant = 'curve_rpm'
if variant in ('curve', 'curve_rpm'):
    requirements['cooling'] = ([('button', 'Применить охлаждение')], ('spin button', 'Точка 1: °C'))
if page == 'lighting' and any(row['name'] == 'Порог CPU, °C' for row in rows):
    variant = 'thermal'
    requirements['lighting'] = ([('button', 'Применить оповещения')], ('spin button', 'Порог CPU, °C'))
if page == 'lighting' and any(row['name'].startswith('Нижний порог (') for row in rows):
    variant = 'dynamic_rpm' if any(row['name'] == 'Нижний порог (rpm)' for row in rows) else 'dynamic_celsius'
    requirements['lighting'] = ([('button', 'Применить режим по датчику')], ('spin button', 'Нижний порог ('))
if page == 'lighting' and any(row['name'].startswith('Верхний порог (') and row.get('value', {}).get('current') == 30 for row in rows):
    variant = 'dynamic_invalid'
    requirements['lighting'] = ([('button', 'Применить режим по датчику')], ('spin button', 'Верхний порог ('))
required, focused = requirements[page]
required = [('document web', 'LL-Control'), ('link', 'Перейти к содержимому'),
            *[('button', name) for name in ['Экран помпы', 'Датчики', 'Охлаждение', 'Подсветка', 'Настройки']], *required]
for role, name in required:
    assert any(row['role'] == role and row['name'] == name for row in rows), (page, role, name, rows)
targets = [row for row in rows if row['role'] == focused[0] and row['name'].startswith(focused[1])]
assert len(targets) == 1 and targets[0]['focused'] and targets[0]['enabled'], (page, 'Focused control not exposed', focused, rows)
if page == 'cooling' and variant == 'main':
    value = targets[0].get('value')
    assert value and value['minimum'] == 10 and value['maximum'] == 100 and value['current'] in (80, 79), ('PWM range interface', value)
if variant in ('dynamic_celsius', 'dynamic_rpm'):
    unit = 'rpm' if variant == 'dynamic_rpm' else '°C'
    maximum = 3600 if unit == 'rpm' else 100
    for name, current, high in [(f'Нижний порог ({unit})', 801 if unit=='rpm' else 31, maximum), (f'Верхний порог ({unit})', 1201 if unit=='rpm' else 61, maximum), ('Нижняя скорость, %', 75 if unit=='rpm' else 1, 100), ('Верхняя скорость, %', 75 if unit=='rpm' else 99, 100)]:
        nodes = [node for node in rows if node['name'] == name and node['role'] == 'spin button']
        assert len(nodes) == 1 and nodes[0]['enabled'] and not nodes[0]['invalid'], (name, nodes)
        assert nodes[0].get('value') == {'current': current, 'minimum': 0, 'maximum': high}, (name, nodes)
if variant == 'dynamic_invalid':
    for name, current in [('Нижний порог (°C)', 31), ('Верхний порог (°C)', 30)]:
        nodes = [node for node in rows if node['name'] == name and node['role'] == 'spin button']
        assert len(nodes) == 1 and nodes[0]['invalid'] and nodes[0]['enabled'], (name, nodes)
        assert nodes[0].get('value') == {'current': current, 'minimum': 0, 'maximum': 100}, (name, nodes)
    assert any('Пороги температуры должны соответствовать целым' in node['name'] or 'Пороги температуры должны соответствовать целым' in node.get('text', '') for node in rows), 'Accessible validation text missing'
if variant == 'thermal':
    assert targets[0].get('value') == {'current': 86, 'minimum': 0, 'maximum': 150}, targets[0]
if variant == 'rpm':
    assert targets[0].get('value') == {'current': 2201, 'minimum': 800, 'maximum': 3600}, targets[0]
if variant in ('curve', 'curve_rpm'):
    duty_min = 20 if variant == 'curve_rpm' else 10
    power = ('RPM', 800, 3600) if variant == 'curve_rpm' else ('PWM %', 10, 100)
    for name, minimum, maximum in [('Точка 1: °C', -50, 150), ('Точка 2: °C', -50, 150), power, ('Гистерезис, °C', 0, 10), ('Fallback PWM, %', duty_min, 100)]:
        controls = [node for node in rows if node['name'] == name]
        assert controls, name
        for node in controls:
            assert node['role'] == 'spin button' and node.get('value'), (name, node)
            assert node['value']['minimum'] == minimum and node['value']['maximum'] == maximum, (name, node['value'])
    assert targets[0]['value']['current'] == 31, 'Curve temperature did not update in AT-SPI'
print(json.dumps({'variant': variant, 'checks': ['navigation names/roles', page + ' controls names/roles', 'target focused/enabled'], 'nodes': rows}, ensure_ascii=False))
Atspi.exit()
