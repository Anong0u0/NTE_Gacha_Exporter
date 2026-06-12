from __future__ import annotations

import contextlib
import ctypes
import ctypes.wintypes as wt
import socket
import struct
from dataclasses import dataclass

iphlpapi = ctypes.windll.iphlpapi
kernel32 = ctypes.windll.kernel32

AF_INET = 2
AF_INET6 = 23
TCP_TABLE_OWNER_PID_ALL = 5
NO_ERROR = 0
ERR_INSUFFICIENT_BUF = 122
TH32CS_SNAPPROCESS = 0x02
INVALID_HANDLE = wt.HANDLE(-1).value
MAX_RETRIES = 3


@dataclass(frozen=True)
class TcpConnection:
    state: int
    local_ip: str
    local_port: int
    remote_ip: str
    remote_port: int
    pid: int


class MIB_TCPROW_OWNER_PID(ctypes.Structure):
    _fields_ = [
        ("dwState", wt.DWORD),
        ("dwLocalAddr", wt.DWORD),
        ("dwLocalPort", wt.DWORD),
        ("dwRemoteAddr", wt.DWORD),
        ("dwRemotePort", wt.DWORD),
        ("dwOwningPid", wt.DWORD),
    ]


class MIB_TCP6ROW_OWNER_PID(ctypes.Structure):
    _fields_ = [
        ("ucLocalAddr", ctypes.c_ubyte * 16),
        ("dwLocalScopeId", wt.DWORD),
        ("dwLocalPort", wt.DWORD),
        ("ucRemoteAddr", ctypes.c_ubyte * 16),
        ("dwRemoteScopeId", wt.DWORD),
        ("dwRemotePort", wt.DWORD),
        ("dwState", wt.DWORD),
        ("dwOwningPid", wt.DWORD),
    ]


class PROCESSENTRY32W(ctypes.Structure):
    _fields_ = [
        ("dwSize", wt.DWORD),
        ("cntUsage", wt.DWORD),
        ("th32ProcessID", wt.DWORD),
        ("th32DefaultHeapID", ctypes.c_size_t),
        ("th32ModuleID", wt.DWORD),
        ("cntThreads", wt.DWORD),
        ("th32ParentProcessID", wt.DWORD),
        ("pcPriClassBase", ctypes.c_long),
        ("dwFlags", wt.DWORD),
        ("szExeFile", ctypes.c_wchar * 260),
    ]


class MIB_IPFORWARDROW(ctypes.Structure):
    _fields_ = [
        ("dwForwardDest", wt.DWORD),
        ("dwForwardMask", wt.DWORD),
        ("dwForwardPolicy", wt.DWORD),
        ("dwForwardNextHop", wt.DWORD),
        ("dwForwardIfIndex", wt.DWORD),
        ("dwForwardType", wt.DWORD),
        ("dwForwardProto", wt.DWORD),
        ("dwForwardAge", wt.DWORD),
        ("dwForwardNextHopAS", wt.DWORD),
        ("dwForwardMetric1", wt.DWORD),
        ("dwForwardMetric2", wt.DWORD),
        ("dwForwardMetric3", wt.DWORD),
        ("dwForwardMetric4", wt.DWORD),
        ("dwForwardMetric5", wt.DWORD),
    ]


class MIB_IPADDRROW(ctypes.Structure):
    _fields_ = [
        ("dwAddr", wt.DWORD),
        ("dwIndex", wt.DWORD),
        ("dwMask", wt.DWORD),
        ("dwBCastAddr", wt.DWORD),
        ("dwReasmSize", wt.DWORD),
        ("unused1", ctypes.c_ushort),
        ("wType", ctypes.c_ushort),
    ]


def _query_tcp_table(af: int, row_cls, parse_row) -> list[TcpConnection]:
    conns: list[TcpConnection] = []
    for _ in range(MAX_RETRIES):
        size = wt.DWORD(0)
        iphlpapi.GetExtendedTcpTable(None, ctypes.byref(size), False, af, TCP_TABLE_OWNER_PID_ALL, 0)
        buf = (ctypes.c_byte * size.value)()
        ret = iphlpapi.GetExtendedTcpTable(buf, ctypes.byref(size), False, af, TCP_TABLE_OWNER_PID_ALL, 0)
        if ret == NO_ERROR:
            break
        if ret != ERR_INSUFFICIENT_BUF:
            return conns
    else:
        return conns

    count = wt.DWORD.from_buffer_copy(buf, 0).value
    offset = ctypes.sizeof(wt.DWORD)
    row_size = ctypes.sizeof(row_cls)
    for index in range(count):
        row = row_cls.from_buffer_copy(buf, offset + index * row_size)
        conns.append(parse_row(row))
    return conns


def _parse_v4_row(row: MIB_TCPROW_OWNER_PID) -> TcpConnection:
    return TcpConnection(
        state=row.dwState,
        local_ip=socket.inet_ntoa(struct.pack("<I", row.dwLocalAddr)),
        local_port=socket.ntohs(row.dwLocalPort & 0xFFFF),
        remote_ip=socket.inet_ntoa(struct.pack("<I", row.dwRemoteAddr)),
        remote_port=socket.ntohs(row.dwRemotePort & 0xFFFF),
        pid=row.dwOwningPid,
    )


def _parse_v6_row(row: MIB_TCP6ROW_OWNER_PID) -> TcpConnection:
    return TcpConnection(
        state=row.dwState,
        local_ip=socket.inet_ntop(socket.AF_INET6, bytes(row.ucLocalAddr)),
        local_port=socket.ntohs(row.dwLocalPort & 0xFFFF),
        remote_ip=socket.inet_ntop(socket.AF_INET6, bytes(row.ucRemoteAddr)),
        remote_port=socket.ntohs(row.dwRemotePort & 0xFFFF),
        pid=row.dwOwningPid,
    )


def get_tcp_table() -> list[TcpConnection]:
    conns: list[TcpConnection] = []
    with contextlib.suppress(OSError):
        conns.extend(_query_tcp_table(AF_INET, MIB_TCPROW_OWNER_PID, _parse_v4_row))
    with contextlib.suppress(OSError):
        conns.extend(_query_tcp_table(AF_INET6, MIB_TCP6ROW_OWNER_PID, _parse_v6_row))
    return conns


def get_pid_exe_map() -> dict[int, str]:
    result: dict[int, str] = {}
    snap = kernel32.CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
    if snap == INVALID_HANDLE:
        return result
    try:
        pe = PROCESSENTRY32W()
        pe.dwSize = ctypes.sizeof(PROCESSENTRY32W)
        if kernel32.Process32FirstW(snap, ctypes.byref(pe)):
            while True:
                result[pe.th32ProcessID] = pe.szExeFile
                if not kernel32.Process32NextW(snap, ctypes.byref(pe)):
                    break
    finally:
        kernel32.CloseHandle(snap)
    return result


def get_default_route_ip() -> str | None:
    try:
        route = MIB_IPFORWARDROW()
        if iphlpapi.GetBestRoute(0, 0, ctypes.byref(route)) != NO_ERROR:
            return None
        target_idx = route.dwForwardIfIndex

        size = wt.DWORD(0)
        iphlpapi.GetIpAddrTable(None, ctypes.byref(size), False)
        buf = (ctypes.c_byte * size.value)()
        if iphlpapi.GetIpAddrTable(buf, ctypes.byref(size), False) != NO_ERROR:
            return None

        count = wt.DWORD.from_buffer_copy(buf, 0).value
        offset = ctypes.sizeof(wt.DWORD)
        row_size = ctypes.sizeof(MIB_IPADDRROW)
        for index in range(count):
            row = MIB_IPADDRROW.from_buffer_copy(buf, offset + index * row_size)
            if row.dwIndex == target_idx:
                ip = socket.inet_ntoa(struct.pack("<I", row.dwAddr))
                if ip not in {"127.0.0.1", "0.0.0.0"}:
                    return ip
    except OSError:
        pass
    return None
