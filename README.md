# SoLeague Backend - Solana Smart Contract

[![Solana](https://img.shields.io/badge/Solana-000?style=flat&logo=solana)](https://solana.com)
[![Anchor](https://img.shields.io/badge/Anchor-0.32.1-orange)](https://anchor-lang.com)
[![Rust](https://img.shields.io/badge/Rust-1.89.0-red)](https://rust-lang.org)

A comprehensive Solana program implementing a blockchain-based competitive gaming platform with token-based wagering, external betting systems, and cryptographic winner verification through merkle proofs.

## Architecture Overview

### Core Program Structure

**Program ID:** `Cd5T6WzasA4ThobZv7LDYKZv3TpaH9rpkJ56dLfESRKC`

The smart contract consists of five primary account types and eight instruction handlers implementing a complete battle and betting ecosystem:

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   GlobalState   │    │     Battle      │    │  BettingPool    │
│                 │    │                 │    │                 │
│ • admin         │    │ • creator       │    │ • battle        │
│ • merkle_root   │    │ • battle_type   │    │ • total_pool    │
│ • bump          │    │ • players[6]    │    │ • bets_on_a/b   │
└─────────────────┘    │ • league_amount │    │ • is_settled    │
                       │ • start/end_time│    └─────────────────┘
                       │ • total_pool    │
                       └─────────────────┘
                               │
                    ┌─────────────────┐    ┌─────────────────┐
                    │ PlayerCommit    │    │      Bet        │
                    │                 │    │                 │
                    │ • battle        │    │ • bettor        │
                    │ • player        │    │ • battle        │
                    │ • balance_hash  │    │ • predicted_win │
                    │ • timestamp     │    │ • amount        │
                    │ • is_verified   │    │ • is_claimed    │
                    └─────────────────┘    └─────────────────┘
```

### Battle System Architecture

#### Battle Types
- **OneVsOne**: 2-player competitive battles with external betting support
- **Friends**: Group battles supporting up to 6 players (no betting allowed)

#### Battle Lifecycle
1. **Creation** (`create_battle`) - Creator deposits entry fee, sets parameters
2. **Joining** (`join_battle`) - Players join with matching entry fee deposits
3. **State Commitment** (`commit_initial_state`) - Players submit wallet state hashes
4. **Resolution** - Off-chain game execution and merkle tree generation
5. **Payout** (`claim_winnings`) - Winners claim via merkle proof verification

### Token Economics & Vault System

#### Vault Architecture
```
Battle Entry Fees:
Creator/Players → Battle Vault → Winners (via merkle proof)

External Betting:
Bettors → Betting Vault → Bet Winners (via merkle proof)
```

#### Fund Flow
- **Entry Fees**: Locked in battle-specific vaults until resolution
- **Betting Pools**: Separate token vaults for 1v1 battle betting
- **Payouts**: All distributions require valid merkle proofs
- **Security**: PDA-based vault authorities prevent unauthorized access

### Cryptographic Security Model

#### Merkle Proof Verification
Custom implementation providing tamper-proof winner verification:

```rust
fn verify_merkle_proof(proof: Vec<[u8; 32]>, root: [u8; 32], leaf: [u8; 32]) -> bool
```

**Security Properties:**
- Admin-controlled merkle root updates via `set_merkle_root()`
- Deterministic leaf ordering for consistent proof generation
- Custom hash function implementation for reduced compute costs
- Prevents double-claiming through bet/player state tracking

#### State Commitment System
Players commit wallet state hashes before battle execution:
- Pre-battle state verification
- Anti-cheating mechanisms
- Audit trail for dispute resolution

### Betting System (1v1 Only)

#### Betting Pool Mechanics
```rust
// Odds calculation based on pool distribution
total_pool = bets_on_player_a + bets_on_player_b
payout_ratio = total_pool / winning_side_total
```

#### Betting Constraints
- **Participant Restriction**: Battle players cannot bet on their own battles
- **Battle Type**: Only OneVsOne battles accept external bets
- **Timing**: Bets only accepted before battle end_time
- **Capacity**: Battles must be full (2/2 players) before betting opens

### Program Instructions

#### Core Battle Functions

**`initialize(admin: Pubkey)`**
- Establishes global program state with designated admin
- Initializes empty merkle root for future winner verification
- Seeds: `["global_state"]`

**`create_battle(battle_type: BattleType, league_amount: u64, duration_days: u8)`**
- Instantiates new battle with creator as first participant
- Transfers entry fee to battle vault
- Enforces max duration of 7 days
- Seeds: `["battle", creator.key(), league_amount.to_le_bytes()]`

**`join_battle()`**
- Adds player to existing battle with entry fee deposit
- Validates battle capacity and timing constraints
- Updates battle pool and participant tracking
- Requires: Active battle, available slots, valid timing

**`commit_initial_state(wallet_balance_hash: [u8; 32])`**
- Records player wallet state before battle execution
- Creates tamper-proof audit trail
- Seeds: `["commit", battle.key(), player.key()]`

#### Payout Functions

**`claim_winnings(merkle_proof: Vec<[u8; 32]>, amount: u64, leaf_hash: [u8; 32])`**
- Distributes battle winnings to verified winners
- Requires valid merkle proof against admin-set root
- Transfers from battle vault to winner's token account

**`place_bet(predicted_winner: Pubkey, bet_amount: u64)`**
- Enables external betting on 1v1 battle outcomes
- Creates/updates betting pool statistics
- Seeds: `["bet", battle.key(), bettor.key()]`

**`claim_bet_winnings(merkle_proof: Vec<[u8; 32]>, payout_amount: u64, leaf_hash: [u8; 32])`**
- Distributes betting winnings to successful predictors
- Prevents double-claiming via bet state tracking
- Calculated payouts based on pool distribution

#### Admin Functions

**`set_merkle_root(merkle_root: [u8; 32])`**
- Updates global merkle root for winner verification
- Admin-only function with signature validation
- Emits event for transparency and auditability

### Security Constraints & Validations

#### Temporal Security
- Battle duration: 1-7 days maximum
- Join deadline: Before battle end_time
- Claim period: Only after battle end_time
- State commitment: Before battle conclusion

#### Access Control
- Admin-only merkle root updates
- Participant-only state commitments
- Winner-only prize claims
- Bettor-only bet winnings

#### Business Logic Validation
- Duplicate participation prevention
- Battle capacity enforcement (2 for OneVsOne, 6 for Friends)
- Betting restrictions (participants cannot bet on own battles)
- Amount validation (sufficient token balances)

## Development Setup

### Prerequisites
```bash
# Rust toolchain
rustup install 1.89.0
rustup default 1.89.0
rustup component add rustfmt clippy

# Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/v1.18.0/install)"

# Anchor framework
npm install -g @coral-xyz/anchor-cli@0.32.1
```

### Build & Test

```bash
# Build program
anchor build

# Run tests (with extended timeout for blockchain ops)
anchor test
# or with explicit timeout
yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/**/*.ts

# Code formatting
yarn lint:fix
```

### Deployment

```bash
# Deploy to devnet
anchor deploy

# Run migrations
anchor migrate
```

## Technical Specifications

### Account Space Allocations
- `GlobalState`: 8 + 32 + 32 + 1 = 73 bytes
- `Battle`: 8 + 32 + 1 + 8 + 1 + 1 + (32*6) + 8 + 8 + 1 + 8 + 1 = 270 bytes
- `BettingPool`: 8 + 32 + 8 + 8 + 8 + 1 = 65 bytes
- `Bet`: 8 + 32 + 32 + 32 + 8 + 1 = 113 bytes
- `PlayerCommit`: 8 + 32 + 32 + 32 + 8 + 1 = 113 bytes

### PDA Derivation Seeds
```rust
// Global state
["global_state"]

// Battle accounts
["battle", creator.key(), league_amount.to_le_bytes()]
["battle_vault", battle.key()]

// Betting system
["betting_pool", battle.key()]
["betting_vault", battle.key()]
["bet", battle.key(), bettor.key()]

// Player commitments
["commit", battle.key(), player.key()]
```

### Event Emissions
- `BattleCreated`: Battle instantiation with metadata
- `PlayerJoined`: Participant additions to battles
- `PlayerCommitted`: State commitment confirmations
- `BetPlaced`: External bet placements
- `WinningsClaimed`: Battle prize distributions
- `BetWinningsClaimed`: Betting pool payouts
- `MerkleRootUpdated`: Admin root updates for verification

### Error Handling
Comprehensive error codes covering all failure scenarios:
- Authentication failures (`NotAdmin`, `NotBetOwner`)
- Timing violations (`BattleEnded`, `BattleNotEnded`)
- Capacity constraints (`BattleFull`, `BattleNotFull`)
- Business logic violations (`ParticipantCannotBet`, `AlreadyClaimed`)
- Cryptographic failures (`InvalidMerkleProof`)

## Configuration Files

### Anchor.toml
- **Cluster**: Devnet deployment target
- **Wallet**: Phantom wallet integration
- **Scripts**: Custom test runner with extended timeouts

### package.json
- **Testing**: Mocha/Chai framework with TypeScript support
- **Linting**: Prettier for code formatting consistency
- **Dependencies**: Anchor client libraries

### rust-toolchain.toml
- **Version**: Rust 1.89.0 stable
- **Profile**: Minimal installation footprint
- **Components**: rustfmt + clippy for development

## License

ISC License - See package.json for details.
