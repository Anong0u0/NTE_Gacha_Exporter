from __future__ import annotations

import base64
import struct
import time
from typing import Any, TypedDict

from nte_gacha_exporter.core.schema import RawPacketRecord


class ParsedNetworkPacket(TypedDict, total=False):
    proto: str
    sport: int
    dport: int
    seq: int
    ack: int
    flags: int
    payload: bytes
    parser: str


def parse_raw_ipv4(raw: bytes) -> ParsedNetworkPacket | None:
    """Fallback parser for Scapy packets that did not decode as IP/Raw."""

    for ip_off in (14, 0):
        if len(raw) < ip_off + 20 or raw[ip_off] >> 4 != 4:
            continue

        ihl = (raw[ip_off] & 0x0F) * 4
        if ihl < 20 or len(raw) < ip_off + ihl:
            continue

        total_len = struct.unpack_from("!H", raw, ip_off + 2)[0]
        ip_end = min(len(raw), ip_off + total_len)
        proto_num = raw[ip_off + 9]
        l4_off = ip_off + ihl

        if proto_num == 6 and ip_end >= l4_off + 20:
            sport, dport, seq, ack, off_flags = struct.unpack_from("!HHIIH", raw, l4_off)
            tcp_header_len = (off_flags >> 12) * 4
            if tcp_header_len < 20 or ip_end < l4_off + tcp_header_len:
                continue
            payload = raw[l4_off + tcp_header_len : ip_end]
            if payload:
                return {
                    "proto": "tcp",
                    "sport": int(sport),
                    "dport": int(dport),
                    "seq": int(seq),
                    "ack": int(ack),
                    "flags": int(off_flags & 0x01FF),
                    "payload": payload,
                    "parser": "raw-ipv4",
                }

        if proto_num == 17 and ip_end >= l4_off + 8:
            sport, dport, udp_len, _checksum = struct.unpack_from("!HHHH", raw, l4_off)
            payload_end = min(ip_end, l4_off + udp_len)
            payload = raw[l4_off + 8 : payload_end]
            if payload:
                return {
                    "proto": "udp",
                    "sport": int(sport),
                    "dport": int(dport),
                    "payload": payload,
                    "parser": "raw-ipv4",
                }

    return None


def packet_to_raw_record(packet: Any, *, capture_index: int) -> RawPacketRecord | None:
    """Convert a Scapy packet to the private raw JSONL packet schema."""

    parsed = None
    try:
        from scapy.layers.inet import IP, TCP, UDP  # type: ignore
        from scapy.packet import Raw  # type: ignore
    except Exception:
        pass
    else:
        if IP in packet and Raw in packet:
            if TCP in packet:
                parsed = {
                    "proto": "tcp",
                    "sport": int(packet[TCP].sport),
                    "dport": int(packet[TCP].dport),
                    "seq": int(packet[TCP].seq),
                    "ack": int(packet[TCP].ack),
                    "flags": int(packet[TCP].flags),
                    "payload": bytes(packet[Raw].load),
                    "parser": "scapy",
                }
            elif UDP in packet:
                parsed = {
                    "proto": "udp",
                    "sport": int(packet[UDP].sport),
                    "dport": int(packet[UDP].dport),
                    "payload": bytes(packet[Raw].load),
                    "parser": "scapy",
                }

    if parsed is None:
        try:
            parsed = parse_raw_ipv4(bytes(packet))
        except Exception:
            parsed = None
    if parsed is None:
        return None

    payload = parsed.pop("payload")
    return {
        "type": "packet",
        "schema_version": 1,
        "captured_at": time.time(),
        "capture_index": capture_index,
        **parsed,
        "size": len(payload),
        "payload_b64": base64.b64encode(payload).decode("ascii"),
    }
