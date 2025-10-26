import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from '@solana/web3.js';
import * as fs from 'fs';
import * as crypto from 'crypto';

function sha256(data: Buffer): Buffer {
  return crypto.createHash('sha256').update(data).digest();
}

function getDiscriminator(name: string): Buffer {
  const preimage = `global:${name}`;
  const hash = sha256(Buffer.from(preimage));
  return hash.slice(0, 8);
}

async function initialize() {
  try {
    const connection = new Connection('https://api.devnet.solana.com', 'confirmed');

    const keypairPath = '/Users/ldm/.config/solana/id.json';
    const keypairData = JSON.parse(fs.readFileSync(keypairPath, 'utf-8'));
    const adminKeypair = Keypair.fromSecretKey(new Uint8Array(keypairData));

    console.log('Admin wallet:', adminKeypair.publicKey.toString());

    const programId = new PublicKey('Fo5yHR18hNooLoFzxYcjpi5BoUx5rhnxhzVRetpVeSsY');

    const [globalStatePDA, bump] = PublicKey.findProgramAddressSync(
      [Buffer.from('global_state')],
      programId
    );

    console.log('Global State PDA:', globalStatePDA.toString());
    console.log('Bump:', bump);

    const discriminator = getDiscriminator('initialize');

    const data = Buffer.concat([
      discriminator,
      adminKeypair.publicKey.toBuffer(),
    ]);

    const instruction = new TransactionInstruction({
      keys: [
        { pubkey: globalStatePDA, isSigner: false, isWritable: true },
        { pubkey: adminKeypair.publicKey, isSigner: true, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      programId,
      data,
    });

    const transaction = new Transaction().add(instruction);

    console.log('Sending transaction...');

    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [adminKeypair],
      { commitment: 'confirmed' }
    );

    console.log('Success!');
    console.log('Transaction signature:', signature);
    console.log('View on explorer: https://explorer.solana.com/tx/' + signature + '?cluster=devnet');

  } catch (error: any) {
    console.error('Error:', error);
    if (error.logs) {
      console.error('Program logs:', error.logs);
    }
    process.exit(1);
  }
}

initialize();
