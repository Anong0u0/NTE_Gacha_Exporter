from __future__ import annotations

from dataclasses import dataclass
from typing import Any

TCP_STATE_ESTAB = 5
KNOWN_TARGET_PORTS = [30031, 10012, 30230]
DEFAULT_VPN_PATTERNS = [
    "hyper-v",
    "vethernet",
    "wireguard",
    "wg-",
    "openvpn",
    "tap-windows",
    "tailscale",
    "zerotier",
]


@dataclass(frozen=True)
class CaptureTarget:
    pid: str
    interface: str
    ports: list[int]
    selected_by_port: int | None
    bpf: str


def _require_windows() -> None:
    import sys

    if sys.platform != "win32":
        raise OSError(f"Windows APIs required. Current platform: {sys.platform}")


def _winapi_module():
    _require_windows()
    from nte_gacha_exporter.capture import _winapi_min

    return _winapi_min


def find_process_pid(exe_name: str) -> str | None:
    winapi = _winapi_module()
    for pid, exe in winapi.get_pid_exe_map().items():
        if exe.lower() == exe_name.lower():
            return str(pid)
    return None


def find_htgame_pid() -> str | None:
    for name in ("HTGame.exe", "HTGame"):
        pid = find_process_pid(name)
        if pid:
            return pid
    return None


def _is_localhost(ip: str) -> bool:
    return ip == "127.0.0.1" or ip == "::1" or ip.startswith("127.")


def _npcap_interfaces(scapy_conf: Any) -> tuple[dict[str, str], str | None]:
    ip_to_name: dict[str, str] = {}
    loopback_name = None
    for key, iface_obj in scapy_conf.ifaces.items():
        name = getattr(iface_obj, "name", "") or ""
        ip = getattr(iface_obj, "ip", "") or ""
        key_text = str(key)
        if "loopback" in name.lower() or "loopback" in key_text.lower():
            loopback_name = name
        elif ip and ip not in {"0.0.0.0", "127.0.0.1"}:
            ip_to_name[ip] = name
    return ip_to_name, loopback_name


def _find_vpn_interface(scapy_conf: Any) -> str | None:
    for key, iface_obj in scapy_conf.ifaces.items():
        name = getattr(iface_obj, "name", "") or ""
        ip = getattr(iface_obj, "ip", "") or ""
        haystack = f"{name} {key}".lower()
        if "loopback" in haystack:
            continue
        if ip and ip not in {"0.0.0.0", "127.0.0.1"} and any(pattern in haystack for pattern in DEFAULT_VPN_PATTERNS):
            return name
    return None


def _fallback_iface(scapy_conf: Any, ip_to_name: dict[str, str]) -> str | None:
    winapi = _winapi_module()
    route_ip = winapi.get_default_route_ip()
    if route_ip and route_ip in ip_to_name:
        return ip_to_name[route_ip]
    if ip_to_name:
        return next(iter(ip_to_name.values()))
    return _find_vpn_interface(scapy_conf)


def _udp_local_ports_windows(pid: str) -> list[int]:
    import subprocess

    script = (
        f"Get-NetUDPEndpoint -OwningProcess {int(pid)} -ErrorAction SilentlyContinue "
        "| Select-Object -ExpandProperty LocalPort"
    )
    try:
        result = subprocess.run(
            ["powershell.exe", "-NoProfile", "-Command", script],
            check=False,
            capture_output=True,
            text=True,
            timeout=5,
        )
    except Exception:
        return []

    ports: list[int] = []
    for line in result.stdout.splitlines():
        try:
            port = int(line.strip())
        except ValueError:
            continue
        if port not in ports:
            ports.append(port)
    return ports


def candidate_ports(pid: str) -> list[int]:
    winapi = _winapi_module()
    ports = list(KNOWN_TARGET_PORTS)
    for conn in winapi.get_tcp_table():
        if str(conn.pid) != str(pid):
            continue
        if conn.remote_port and not _is_localhost(conn.remote_ip):
            port = int(conn.remote_port)
            if port == 443:
                continue
            if port not in ports:
                ports.append(port)
    for port in _udp_local_ports_windows(pid):
        if port not in ports:
            ports.append(port)
    return ports


def select_capture_target(pid: str, scapy_conf: Any) -> CaptureTarget:
    winapi = _winapi_module()
    ports = candidate_ports(pid)
    ip_to_name, _loopback = _npcap_interfaces(scapy_conf)
    fallback = _fallback_iface(scapy_conf, ip_to_name)

    pid_int = int(pid)
    conns = winapi.get_tcp_table()
    for port in ports:
        direct = [
            conn
            for conn in conns
            if conn.pid == pid_int
            and conn.state == TCP_STATE_ESTAB
            and conn.remote_port == port
            and not _is_localhost(conn.remote_ip)
        ]
        if direct:
            iface = ip_to_name.get(direct[0].local_ip) or _find_vpn_interface(scapy_conf) or fallback
            if iface:
                return CaptureTarget(pid, iface, ports, port, " or ".join(f"port {p}" for p in ports))

    if fallback:
        return CaptureTarget(pid, fallback, ports, None, " or ".join(f"port {p}" for p in ports))
    raise RuntimeError("Could not select Npcap interface. Pass --iface.")


def resolve_scapy_iface(iface_name: str) -> Any:
    try:
        from scapy.config import conf  # type: ignore

        for key, iface in conf.ifaces.items():
            if getattr(iface, "name", None) == iface_name or str(key) == iface_name:
                return iface
    except Exception:
        pass
    return iface_name
