"""Fixture bridge: lose LCD/source/font/profile replies and withhold an LCD reply."""
import json
import select
import socket
import socketserver
import threading


class FaultProxy:
    def __init__(self, path, backend, cleanup_profile_fault=None):
        self.cleanup_profile_fault = cleanup_profile_fault
        self.cleanup_profile_armed = False
        self.cleanup_profile_checks = []
        self.backend = str(backend)
        self.lock = threading.Lock()
        self.stopped = threading.Event()
        self.requests = 0
        self.dropped = 0
        self.timeout_requests = 0
        self.withheld = 0
        self.theme_requests = 0
        self.theme_dropped = 0
        self.clear_requests = 0
        self.clear_dropped = 0
        self.profile_requests = 0
        self.profile_dropped = 0
        self.profile_extra_requests = {}
        self.profile_extra_dropped = {}
        self.profile_rejected = 0
        self.source_requests = 0
        self.source_dropped = 0
        self.source_total = 0
        self.font_begins = 0
        self.font_finishes = 0
        self.font_dropped = 0
        self.font_status_reads = 0
        self.partial_font_id = None
        self.partial_chunks = 0
        self.partial_cancelled = False
        self.font_id = None
        self.font_asset = None
        self.errors = []
        owner = self

        class Handler(socketserver.BaseRequestHandler):
            def handle(self):
                with socket.socket(socket.AF_UNIX) as upstream:
                    try:
                        upstream.connect(owner.backend)
                        peers = {self.request: upstream, upstream: self.request}
                        buffers = {sock: bytearray() for sock in peers}
                        target_ids = {}
                        while not owner.stopped.is_set():
                            ready, _, _ = select.select(list(peers), [], [], .2)
                            for source in ready:
                                chunk = source.recv(65536)
                                if not chunk:
                                    return
                                buffers[source].extend(chunk)
                                if len(buffers[source]) > 8 * 1024 * 1024:
                                    raise ValueError('fixture proxy line exceeded bound')
                                while b'\n' in buffers[source]:
                                    line, _, rest = buffers[source].partition(b'\n')
                                    buffers[source] = bytearray(rest)
                                    obj = json.loads(line)
                                    if source is self.request and obj.get('method') in ('cleanup_theme_assets', 'cleanup_theme_fonts') and owner.cleanup_profile_armed:
                                        owner.cleanup_profile_armed = False
                                        if owner.cleanup_profile_fault is None:
                                            raise ValueError('profile cleanup fixture callback missing')
                                        response = owner.cleanup_profile_fault(obj)
                                        owner.cleanup_profile_checks.append(obj['method'])
                                        self.request.sendall(json.dumps(response).encode() + b'\n')
                                        continue
                                    brightness = obj.get('params', {}).get('patch', {}).get('brightness_percent')
                                    if source is self.request and obj.get('method') == 'patch_lcd_config' and brightness in (66, 67):
                                        target_ids[obj['id']] = brightness
                                        with owner.lock:
                                            if brightness == 66:
                                                owner.requests += 1
                                            else:
                                                owner.timeout_requests += 1
                                    if source is self.request and obj.get('method') == 'set_sensor_defaults':
                                        with owner.lock:
                                            owner.source_total += 1
                                        if obj.get('params', {}).get('config', {}).get('revision') == 1:
                                            target_ids[obj['id']] = 'source'
                                            with owner.lock:
                                                owner.source_requests += 1
                                    if source is self.request and obj.get('method') == 'set_lcd_theme' and obj.get('params', {}).get('theme', {}).get('name') == 'Native lost theme reply':
                                        target_ids[obj['id']] = 'theme'
                                        with owner.lock:
                                            owner.theme_requests += 1
                                    if source is self.request and obj.get('method') == 'clear_lcd':
                                        with owner.lock:
                                            if owner.theme_dropped:
                                                target_ids[obj['id']] = 'clear'
                                                owner.clear_requests += 1
                                    if source is self.request and obj.get('method') == 'edit_profile':
                                        edit = obj.get('params', {}).get('edit', {})
                                        if edit.get('action') == 'save' and edit.get('profile', {}).get('name') == 'Native reject before forward':
                                            with owner.lock:
                                                owner.profile_rejected += 1
                                            # Never forward this frame to the daemon.
                                            self.request.shutdown(socket.SHUT_RDWR)
                                            return
                                        if edit.get('action') == 'rename' and edit.get('slot') == 6 and edit.get('name') == 'Native cleanup profile fault':
                                            owner.cleanup_profile_armed = True
                                        action = edit.get('action')
                                        if edit.get('group') == 'screen' and edit.get('slot') == 7 and action in ('rename', 'reset', 'delete'):
                                            target_ids[obj['id']] = ('profile_extra', action)
                                            with owner.lock:
                                                owner.profile_extra_requests[action] = owner.profile_extra_requests.get(action, 0) + 1
                                        if edit.get('action') == 'save' and edit.get('profile', {}).get('name') == 'Native lost profile reply':
                                            target_ids[obj['id']] = 'profile'
                                            with owner.lock:
                                                owner.profile_requests += 1
                                    if source is self.request and obj.get('method') == 'font_upload':
                                        request = obj.get('params', {}).get('request', {})
                                        with owner.lock:
                                            if request.get('action') == 'begin':
                                                owner.font_begins += 1
                                            elif request.get('action') == 'chunk':
                                                if owner.partial_font_id is None and len(request.get('bytes', [])) == 65536:
                                                    owner.partial_font_id = request['id']
                                                if request['id'] == owner.partial_font_id:
                                                    owner.partial_chunks += 1
                                                    target_ids[obj['id']] = ('partial', request['id'])
                                            elif request.get('action') == 'cancel' and request['id'] == owner.partial_font_id:
                                                target_ids[obj['id']] = ('cancel_partial', request['id'])
                                            elif request.get('action') == 'finish':
                                                owner.font_finishes += 1
                                                target_ids[obj['id']] = ('font', request['id'])
                                            elif request.get('action') == 'status' and request.get('id') == owner.font_id:
                                                owner.font_status_reads += 1
                                    if source is upstream and obj.get('id') in target_ids:
                                        mode = target_ids.pop(obj['id'])
                                        if isinstance(mode, tuple) and mode[0] == 'profile_extra':
                                            if 'error' in obj:
                                                raise ValueError('profile mutation failed before intended response loss')
                                            with owner.lock:
                                                owner.profile_extra_dropped[mode[1]] = owner.profile_extra_dropped.get(mode[1], 0) + 1
                                            self.request.shutdown(socket.SHUT_RDWR)
                                            return
                                        if isinstance(mode, tuple) and mode[0] == 'partial':
                                            state = obj.get('result', {})
                                            if state.get('received') != 65536 or state.get('asset') is not None:
                                                raise ValueError('partial font did not accept exactly one chunk')
                                            self.request.shutdown(socket.SHUT_RDWR)
                                            return
                                        if isinstance(mode, tuple) and mode[0] == 'cancel_partial':
                                            state = obj.get('result', {})
                                            if not state.get('cancelled') or state.get('asset') is not None:
                                                raise ValueError('partial font was not cancelled without an asset')
                                            with owner.lock:
                                                owner.partial_cancelled = True
                                            peers[source].sendall(line + b'\n')
                                            continue
                                        if isinstance(mode, tuple) and mode[0] == 'font':
                                            # Invalid-file retry precedes the valid upload; lose only a committed result.
                                            if 'result' in obj and obj['result'].get('asset'):
                                                with owner.lock:
                                                    font_drop = owner.font_dropped == 0
                                                    if font_drop:
                                                        owner.font_dropped += 1
                                                        owner.font_id = mode[1]
                                                        owner.font_asset = obj['result']['asset']
                                                if font_drop:
                                                    self.request.shutdown(socket.SHUT_RDWR)
                                                    return
                                            peers[source].sendall(line + b'\n')
                                            continue
                                        if 'error' in obj:
                                            raise ValueError('fault target failed before response loss')
                                        with owner.lock:
                                            theme_drop = mode == 'theme' and owner.theme_dropped == 0
                                            clear_drop = mode == 'clear' and owner.clear_dropped == 0
                                            if theme_drop:
                                                owner.theme_dropped += 1
                                            if clear_drop:
                                                owner.clear_dropped += 1
                                            profile_drop = mode == 'profile' and owner.profile_dropped == 0
                                            if profile_drop:
                                                owner.profile_dropped += 1
                                            source_drop = mode == 'source' and owner.source_dropped == 0
                                            drop = mode == 66 and owner.dropped == 0
                                            withhold = mode == 67 and owner.withheld == 0
                                            if source_drop:
                                                owner.source_dropped += 1
                                            if drop:
                                                owner.dropped += 1
                                            if withhold:
                                                owner.withheld += 1
                                        if drop or source_drop or profile_drop or theme_drop or clear_drop:
                                            self.request.shutdown(socket.SHUT_RDWR)
                                            return
                                        if withhold:
                                            # Keep forwarding reads on this same socket while the write times out.
                                            continue
                                    peers[source].sendall(line + b'\n')
                    except (ConnectionError, FileNotFoundError):
                        # Expected while the fixture is stopped by the reconnect scenarios.
                        pass
                    except Exception as error:
                        with owner.lock:
                            owner.errors.append(str(error))

        class Server(socketserver.ThreadingUnixStreamServer):
            daemon_threads = True

        self.server = Server(str(path), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def close(self):
        self.stopped.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)
