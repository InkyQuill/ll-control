#!/usr/bin/env python3
"""Synthetic portal on the harness's private session bus. Never captures a display."""
import json, os
from pathlib import Path
import gi
gi.require_version('Gio','2.0')
from gi.repository import Gio, GLib
root=Path(os.environ['LLCONTROL_SMOKE_ROOT'])
assert os.environ.get('LLCONTROL_KEYBOARD_ISOLATED')=='1' and (root/'fixture-marker').is_file()
assert os.environ.get('DBUS_SESSION_BUS_ADDRESS')
connection=Gio.bus_get_sync(Gio.BusType.SESSION,None)
reply=connection.call_sync('org.freedesktop.DBus','/org/freedesktop/DBus','org.freedesktop.DBus','RequestName',GLib.Variant('(su)',('org.freedesktop.portal.Desktop',0)),GLib.VariantType.new('(u)'),Gio.DBusCallFlags.NONE,5000,None)
assert reply.unpack()[0]==1
portal=Gio.DBusNodeInfo.new_for_xml('''<node><interface name="org.freedesktop.portal.Screenshot"><method name="Screenshot"><arg name="parent" type="s" direction="in"/><arg name="options" type="a{sv}" direction="in"/><arg name="handle" type="o" direction="out"/></method></interface></node>''')
request=Gio.DBusNodeInfo.new_for_xml('''<node><interface name="org.freedesktop.portal.Request"><method name="Close"/></interface></node>''')
observations={'requests':[],'closed':[],'close_attempts':[]}
def save():
    (root/'portal.tmp').write_text(json.dumps(observations));(root/'portal.tmp').replace(root/'portal.json')
def close(conn,sender,path,interface,method,parameters,invocation):
    assert method=='Close';observations['close_attempts'].append(path);save()
    if os.environ.get('LLCONTROL_SCREENSHOT_RECOVERY')=='1' and not (root/'allow-close').exists():
        invocation.return_dbus_error('org.freedesktop.DBus.Error.Failed','fixture Close rejected');return
    observations['closed'].append(path);save();invocation.return_value(GLib.Variant('()',()))
def screenshot(conn,sender,path,interface,method,parameters,invocation):
    assert method=='Screenshot'
    parent,options=parameters.unpack();assert options['interactive'] and options['modal']
    handle='/org/freedesktop/portal/desktop/request/'+sender[1:].replace('.','_')+'/'+options['handle_token']
    connection.register_object(handle,request.interfaces[0],close,None,None)
    observations['requests'].append({'path':handle,'parent':parent,'interactive':options['interactive']});save()
    count=len(observations['requests'])
    if count<=2 and os.environ.get('LLCONTROL_SCREENSHOT_RECOVERY')!='1':
        result={'uri':GLib.Variant('s',(root/'capture.png').as_uri())} if count==1 else {}
        connection.emit_signal(sender,handle,'org.freedesktop.portal.Request','Response',GLib.Variant('(ua{sv})',(0 if count==1 else 1,result)))
    invocation.return_value(GLib.Variant('(o)',(handle,)))
connection.register_object('/org/freedesktop/portal/desktop',portal.interfaces[0],screenshot,None,None)
save();(root/'portal-ready').touch()
loop=GLib.MainLoop()
def stopped():
    if (root/'stop').exists():loop.quit();return False
    return True
GLib.timeout_add(100,stopped);loop.run()
