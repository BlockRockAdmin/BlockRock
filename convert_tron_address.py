import base58

def base58_to_hex(base58_address):
    try:
        decoded = base58.b58decode(base58_address)
        hex_address = decoded[1:-4].hex()
        return "0x" + hex_address
    except Exception as e:
        return f"Errore: {str(e)}"

# Usa un indirizzo TRON di esempio
address = "TNaRAoLUyYEV2uF7d8x1xzkag7wCY83gDF"
print(f"Indirizzo Base58: {address}")
print(f"Indirizzo Hex: {base58_to_hex(address)}")
