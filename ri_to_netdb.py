#!/usr/bin/env python3
"""
Конвертирует router.info файл i2pd в корректный netDb-путь.
RouterIdentity hash = SHA256(enc_key[256] + sign_key[128] + certificate)
Base64url с заменой +→- и /→~ (I2P стиль, без padding).

Использование:
  python3 ri_to_netdb.py <router.info> <netdb_dir>
"""
import sys
import hashlib
import base64
import os
import struct

def i2p_base64(data: bytes) -> str:
    b64 = base64.b64encode(data).decode()
    return b64.replace('+', '-').replace('/', '~').rstrip('=')

def read_router_identity(data: bytes) -> bytes:
    """Читает RouterIdentity из RouterInfo и возвращает её байты."""
    offset = 0
    # Первые 256 байт — encryption public key (ElGamal или padding для X25519)
    enc_key = data[offset:offset+256]
    offset += 256
    # Следующие 128 байт — signing public key (DSA или padding для Ed25519)
    sign_key = data[offset:offset+128]
    offset += 128
    # Certificate
    cert_type = data[offset]
    offset += 1
    cert_len = struct.unpack('>H', data[offset:offset+2])[0]
    offset += 2
    cert_data = data[offset:offset+cert_len]
    offset += cert_len
    identity_bytes = enc_key + sign_key + bytes([cert_type]) + struct.pack('>H', cert_len) + cert_data
    return identity_bytes

def router_info_hash(router_info_path: str) -> str:
    with open(router_info_path, 'rb') as f:
        data = f.read()
    identity_bytes = read_router_identity(data)
    digest = hashlib.sha256(identity_bytes).digest()
    return i2p_base64(digest)

def install_to_netdb(router_info_path: str, netdb_dir: str) -> str:
    h = router_info_hash(router_info_path)
    subdir = h[:2]
    target_dir = os.path.join(netdb_dir, subdir)
    os.makedirs(target_dir, exist_ok=True)
    filename = f"routerInfo-{h}.dat"
    target_path = os.path.join(target_dir, filename)
    with open(router_info_path, 'rb') as f:
        content = f.read()
    with open(target_path, 'wb') as f:
        f.write(content)
    return target_path

if __name__ == '__main__':
    if len(sys.argv) < 3:
        print(f"Usage: {sys.argv[0]} <router.info> <netdb_dir>", file=sys.stderr)
        sys.exit(1)
    ri_path = sys.argv[1]
    netdb_dir = sys.argv[2]
    result = install_to_netdb(ri_path, netdb_dir)
    print(result)
