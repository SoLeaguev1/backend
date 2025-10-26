#!/usr/bin/env python3
import json
import subprocess
import base58
from solders.pubkey import Pubkey
from solders.keypair import Keypair
from solders.system_program import ID as SYSTEM_PROGRAM_ID
from solders.transaction import Transaction
from solders.message import Message
from solders.instruction import Instruction, AccountMeta
from solana.rpc.api import Client

def find_program_address(seeds, program_id):
    seeds_bytes = [s.encode() if isinstance(s, str) else s for s in seeds]
    return Pubkey.find_program_address(seeds_bytes, program_id)

def main():
    rpc_url = "https://api.devnet.solana.com"
    program_id = Pubkey.from_string("Cd5T6WzasA4ThobZv7LDYKZv3TpaH9rpkJ56dLfESRKC")

    with open("/Users/ldm/.config/solana/id.json") as f:
        keypair_data = json.load(f)
    admin_keypair = Keypair.from_bytes(bytes(keypair_data))

    print(f"Admin wallet: {admin_keypair.pubkey()}")

    global_state_pda, bump = find_program_address(
        ["global_state"],
        program_id
    )

    print(f"Global State PDA: {global_state_pda}")
    print(f"Bump: {bump}")

    discriminator = bytes([175, 175, 109, 31, 13, 152, 155, 237])

    admin_bytes = bytes(admin_keypair.pubkey())

    data = discriminator + admin_bytes

    accounts = [
        AccountMeta(pubkey=global_state_pda, is_signer=False, is_writable=True),
        AccountMeta(pubkey=admin_keypair.pubkey(), is_signer=True, is_writable=True),
        AccountMeta(pubkey=SYSTEM_PROGRAM_ID, is_signer=False, is_writable=False),
    ]

    instruction = Instruction(
        program_id=program_id,
        accounts=accounts,
        data=data,
    )

    client = Client(rpc_url)

    recent_blockhash = client.get_latest_blockhash().value.blockhash

    message = Message.new_with_blockhash(
        [instruction],
        admin_keypair.pubkey(),
        recent_blockhash,
    )

    transaction = Transaction([admin_keypair], message, recent_blockhash)

    result = client.send_transaction(transaction)

    print(f"Success!")
    print(f"Transaction signature: {result.value}")
    print(f"View on explorer: https://explorer.solana.com/tx/{result.value}?cluster=devnet")

if __name__ == "__main__":
    main()
