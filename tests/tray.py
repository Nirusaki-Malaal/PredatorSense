#!/usr/bin/env python3
"""Exercise the tray against an isolated StatusNotifier watcher and mock daemon.

Run with: dbus-run-session -- python3 tests/tray.py
Requires distro python-dbus and PyGObject; neither is a product dependency.
"""
import json
import os
import pathlib
import socket
import struct
import subprocess
import tempfile
import threading

import dbus
import dbus.service
from dbus.mainloop.glib import DBusGMainLoop
from gi.repository import GLib

ROOT = pathlib.Path(__file__).resolve().parents[1]
DBusGMainLoop(set_as_default=True)
bus = dbus.SessionBus()
loop = GLib.MainLoop()
registered = []
failures = []
requests = []


class Watcher(dbus.service.Object):
    @dbus.service.method('org.kde.StatusNotifierWatcher', in_signature='s')
    def RegisterStatusNotifierItem(self, service):
        registered.append(str(service))

    @dbus.service.method('org.freedesktop.DBus.Properties', in_signature='ss', out_signature='v')
    def Get(self, interface, name):
        values = {
            'IsStatusNotifierHostRegistered': dbus.Boolean(True),
            'ProtocolVersion': dbus.Int32(0),
            'RegisteredStatusNotifierItems': dbus.Array(registered, signature='s'),
        }
        return values[str(name)]

    @dbus.service.method('org.freedesktop.DBus.Properties', in_signature='s', out_signature='a{sv}')
    def GetAll(self, interface):
        return {key: self.Get(interface, key) for key in (
            'IsStatusNotifierHostRegistered', 'ProtocolVersion', 'RegisteredStatusNotifierItems')}


name = dbus.service.BusName('org.kde.StatusNotifierWatcher', bus)
watcher = Watcher(bus, '/StatusNotifierWatcher')
with tempfile.TemporaryDirectory(prefix='predator-tray-test-') as directory:
    path = pathlib.Path(directory) / 'control.sock'
    server = socket.socket(socket.AF_UNIX)
    server.bind(str(path))
    server.listen()

    def serve():
        connection, _ = server.accept()
        with connection:
            length = struct.unpack('!I', connection.recv(4))[0]
            data = b''
            while len(data) < length:
                data += connection.recv(length - len(data))
            requests.append(json.loads(data))
            response = json.dumps({'ok': True, 'error': None, 'status': None}).encode()
            connection.sendall(struct.pack('!I', len(response)) + response)

    threading.Thread(target=serve, daemon=True).start()
    env = dict(os.environ, XDG_RUNTIME_DIR=directory, PREDATOR_SENSE_SOCKET=str(path))
    binary = ROOT / os.environ.get('PREDATOR_TRAY_BINARY', 'target/debug/predator-tray')
    tray = subprocess.Popen([binary], env=env, stderr=subprocess.PIPE, text=True)
    checks = 0

    def verify():
        global checks
        checks += 1
        if not registered and checks < 50:
            return True
        try:
            assert registered, 'tray did not register with StatusNotifierWatcher'
            item = bus.get_object(registered[0], '/StatusNotifierItem')
            properties = dbus.Interface(item, 'org.freedesktop.DBus.Properties')
            assert str(properties.Get('org.kde.StatusNotifierItem', 'Title')) == 'Predator Sense'
            menu_path = properties.Get('org.kde.StatusNotifierItem', 'Menu')
            menu = dbus.Interface(bus.get_object(registered[0], menu_path), 'com.canonical.dbusmenu')
            _, layout = menu.GetLayout(0, -1, [])
            entries = {str(child[1].get('label', '')): int(child[0]) for child in layout[2]}
            assert {'Open Predator Sense', 'Cycle performance mode', 'Quit tray'} <= entries.keys()
            menu.Event(entries['Cycle performance mode'], 'clicked', dbus.String(''), dbus.UInt32(0))
            second = subprocess.run([binary], env=env, capture_output=True, timeout=5)
            assert second.returncode == 0, second.stderr
            status = pathlib.Path(f'/proc/{tray.pid}/status').read_text()
            rss = next(line for line in status.splitlines() if line.startswith('VmRSS:'))
            linked = subprocess.check_output(['ldd', binary], text=True)
            assert 'libgtk' not in linked and 'libgdk' not in linked
            try:
                menu.Event(entries['Quit tray'], 'clicked', dbus.String(''), dbus.UInt32(0))
            except dbus.exceptions.DBusException as error:
                if error.get_dbus_name() not in ('org.freedesktop.DBus.Error.NoReply', 'org.freedesktop.DBus.Error.ServiceUnknown'): raise
            tray.wait(timeout=5)
            assert requests == [{'command': 'cycle'}], requests
            print(f'Tray registration, title, menu, cycle IPC, singleton and quit passed; {rss}; no GTK dependency')
        except Exception as error:
            failures.append(error)
        finally:
            loop.quit()
        return False

    GLib.timeout_add(100, verify)
    try:
        loop.run()
    finally:
        if tray.poll() is None:
            tray.terminate()
            tray.wait(timeout=5)
        server.close()
    if failures:
        raise failures[0]
