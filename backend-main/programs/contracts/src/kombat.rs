use anchor_lang::prelude::*;

declare_id!("Fo5yHR18hNooLoFzxYcjpi5BoUx5rhnxhzVRetpVeSsY");

const MAX_PLAYERS_FRIENDS: u8 = 6;
const MAX_BATTLE_DURATION_DAYS: u8 = 7;
const SECONDS_PER_DAY: i64 = 86400;

#[program]
pub mod contracts {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, admin: Pubkey) -> Result<()> {
        let global_state = &mut ctx.accounts.global_state;
        global_state.admin = admin;
        global_state.merkle_root = [0u8; 32];
        global_state.bump = ctx.bumps.global_state;
        Ok(())
    }

    pub fn set_merkle_root(ctx: Context<SetMerkleRoot>, merkle_root: [u8; 32]) -> Result<()> {
        require!(
            ctx.accounts.admin.key() == ctx.accounts.global_state.admin,
            ErrorCode::NotAdmin
        );
        
        ctx.accounts.global_state.merkle_root = merkle_root;
        emit!(MerkleRootUpdated {
            merkle_root,
            admin: ctx.accounts.admin.key(),
        });
        Ok(())
    }

    pub fn create_battle(
        ctx: Context<CreateBattle>,
        battle_type: BattleType,
        league_amount: u64,
        duration_days: u8,
    ) -> Result<()> {
        require!(duration_days <= MAX_BATTLE_DURATION_DAYS, ErrorCode::InvalidDuration);
        
        let max_players = match battle_type {
            BattleType::OneVsOne => 2,
            BattleType::Friends => MAX_PLAYERS_FRIENDS,
        };

        let battle = &mut ctx.accounts.battle;
        battle.creator = ctx.accounts.creator.key();
        battle.battle_type = battle_type.clone();
        battle.league_amount = league_amount;
        battle.max_players = max_players;
        battle.current_players = 1;
        battle.players = vec![ctx.accounts.creator.key()];
        battle.start_time = Clock::get()?.unix_timestamp;
        battle.end_time = battle.start_time + (duration_days as i64 * SECONDS_PER_DAY);
        battle.is_active = true;
        battle.total_pool = league_amount;
        battle.bump = ctx.bumps.battle;

        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.creator.to_account_info(),
                    to: ctx.accounts.battle_vault.to_account_info(),
                },
            ),
            league_amount,
        )?;

        emit!(BattleCreated {
            battle: battle.key(),
            creator: ctx.accounts.creator.key(),
            battle_type,
            league_amount,
            end_time: battle.end_time,
        });

        Ok(())
    }

    pub fn join_battle(ctx: Context<JoinBattle>) -> Result<()> {
        let battle = &mut ctx.accounts.battle;
        
        require!(battle.is_active, ErrorCode::BattleNotActive);
        require!(battle.current_players < battle.max_players, ErrorCode::BattleFull);
        require!(
            Clock::get()?.unix_timestamp < battle.end_time,
            ErrorCode::BattleEnded
        );
        require!(
            !battle.players.contains(&ctx.accounts.player.key()),
            ErrorCode::AlreadyJoined
        );

        battle.players.push(ctx.accounts.player.key());
        battle.current_players += 1;
        battle.total_pool += battle.league_amount;

        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.battle_vault.to_account_info(),
                },
            ),
            battle.league_amount,
        )?;

        emit!(PlayerJoined {
            battle: battle.key(),
            player: ctx.accounts.player.key(),
            total_players: battle.current_players,
        });

        Ok(())
    }

    pub fn commit_initial_state(
        ctx: Context<CommitInitialState>,
        wallet_balance_hash: [u8; 32],
    ) -> Result<()> {
        let battle = &ctx.accounts.battle;

        require!(battle.is_active, ErrorCode::BattleNotActive);
        require!(
            battle.players.contains(&ctx.accounts.player.key()),
            ErrorCode::NotParticipant
        );
        require!(
            Clock::get()?.unix_timestamp < battle.end_time,
            ErrorCode::BattleEnded
        );

        let commit = &mut ctx.accounts.player_commit;
        commit.battle = battle.key();
        commit.player = ctx.accounts.player.key();
        commit.wallet_balance_hash = wallet_balance_hash;
        commit.timestamp = Clock::get()?.unix_timestamp;
        commit.is_verified = false;

        emit!(PlayerCommitted {
            battle: battle.key(),
            player: ctx.accounts.player.key(),
            wallet_balance_hash,
            timestamp: commit.timestamp,
        });

        Ok(())
    }

    pub fn claim_winnings(
        ctx: Context<ClaimWinnings>,
        merkle_proof: Vec<[u8; 32]>,
        amount: u64,
        leaf_hash: [u8; 32],
    ) -> Result<()> {
        let battle = &ctx.accounts.battle;

        require!(
            Clock::get()?.unix_timestamp > battle.end_time,
            ErrorCode::BattleNotEnded
        );
        require!(
            battle.players.contains(&ctx.accounts.winner.key()),
            ErrorCode::NotParticipant
        );

        require!(
            verify_merkle_proof(
                merkle_proof,
                ctx.accounts.global_state.merkle_root,
                leaf_hash
            ),
            ErrorCode::InvalidMerkleProof
        );

        **ctx.accounts.battle_vault.to_account_info().try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.winner.to_account_info().try_borrow_mut_lamports()? += amount;

        emit!(WinningsClaimed {
            battle: battle.key(),
            winner: ctx.accounts.winner.key(),
            amount,
        });

        Ok(())
    }

    pub fn place_bet(
        ctx: Context<PlaceBet>,
        predicted_winner: Pubkey,
        bet_amount: u64,
    ) -> Result<()> {
        let battle = &ctx.accounts.battle;

        require!(
            battle.battle_type == BattleType::OneVsOne,
            ErrorCode::BettingOnlyFor1v1
        );
        require!(
            Clock::get()?.unix_timestamp < battle.end_time,
            ErrorCode::BattleEnded
        );
        require!(
            battle.current_players == 2,
            ErrorCode::BattleNotFull
        );
        require!(
            battle.players.contains(&predicted_winner),
            ErrorCode::InvalidPredictedWinner
        );
        require!(
            !battle.players.contains(&ctx.accounts.bettor.key()),
            ErrorCode::ParticipantCannotBet
        );

        let betting_pool = &mut ctx.accounts.betting_pool;

        if betting_pool.battle == Pubkey::default() {
            betting_pool.battle = battle.key();
            betting_pool.total_pool = 0;
            betting_pool.bets_on_player_a = 0;
            betting_pool.bets_on_player_b = 0;
            betting_pool.is_settled = false;
        }

        betting_pool.total_pool += bet_amount;

        if predicted_winner == battle.players[0] {
            betting_pool.bets_on_player_a += bet_amount;
        } else {
            betting_pool.bets_on_player_b += bet_amount;
        }

        let bet = &mut ctx.accounts.bet;
        bet.bettor = ctx.accounts.bettor.key();
        bet.battle = battle.key();
        bet.predicted_winner = predicted_winner;
        bet.amount = bet_amount;
        bet.is_claimed = false;

        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.bettor.to_account_info(),
                    to: ctx.accounts.betting_vault.to_account_info(),
                },
            ),
            bet_amount,
        )?;

        emit!(BetPlaced {
            battle: battle.key(),
            bettor: ctx.accounts.bettor.key(),
            predicted_winner,
            amount: bet_amount,
        });

        Ok(())
    }

    pub fn claim_bet_winnings(
        ctx: Context<ClaimBetWinnings>,
        merkle_proof: Vec<[u8; 32]>,
        payout_amount: u64,
        leaf_hash: [u8; 32],
    ) -> Result<()> {
        let battle = &ctx.accounts.battle;
        let bet = &mut ctx.accounts.bet;

        require!(
            Clock::get()?.unix_timestamp > battle.end_time,
            ErrorCode::BattleNotEnded
        );
        require!(
            bet.bettor == ctx.accounts.bettor.key(),
            ErrorCode::NotBetOwner
        );
        require!(
            !bet.is_claimed,
            ErrorCode::AlreadyClaimed
        );

        require!(
            verify_merkle_proof(
                merkle_proof,
                ctx.accounts.global_state.merkle_root,
                leaf_hash
            ),
            ErrorCode::InvalidMerkleProof
        );

        bet.is_claimed = true;

        **ctx.accounts.betting_vault.to_account_info().try_borrow_mut_lamports()? -= payout_amount;
        **ctx.accounts.bettor.to_account_info().try_borrow_mut_lamports()? += payout_amount;

        emit!(BetWinningsClaimed {
            battle: battle.key(),
            bettor: ctx.accounts.bettor.key(),
            amount: payout_amount,
        });

        Ok(())
    }
}

fn verify_merkle_proof(proof: Vec<[u8; 32]>, root: [u8; 32], leaf: [u8; 32]) -> bool {
    let mut computed_hash = leaf;
    
    for proof_element in proof.iter() {
        if computed_hash <= *proof_element {
            let combined = [computed_hash, *proof_element].concat();
            computed_hash = simple_hash(&combined);
        } else {
            let combined = [*proof_element, computed_hash].concat();
            computed_hash = simple_hash(&combined);
        }
    }
    
    computed_hash == root
}

fn simple_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = [0u8; 32];
    for (i, byte) in data.iter().enumerate() {
        hasher[i % 32] ^= *byte;
    }
    hasher
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + GlobalState::INIT_SPACE,
        seeds = [b"global_state"],
        bump
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SetMerkleRoot<'info> {
    #[account(
        mut,
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(battle_type: BattleType, league_amount: u64, duration_days: u8)]
pub struct CreateBattle<'info> {
    #[account(
        init,
        payer = creator,
        space = 8 + Battle::INIT_SPACE,
        seeds = [b"battle", creator.key().as_ref(), &league_amount.to_le_bytes()],
        bump
    )]
    pub battle: Account<'info, Battle>,
    #[account(
        mut,
        seeds = [b"battle_vault", battle.key().as_ref()],
        bump
    )]
    pub battle_vault: SystemAccount<'info>,
    #[account(mut)]
    pub creator: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct JoinBattle<'info> {
    #[account(mut)]
    pub battle: Account<'info, Battle>,
    #[account(
        mut,
        seeds = [b"battle_vault", battle.key().as_ref()],
        bump
    )]
    pub battle_vault: SystemAccount<'info>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CommitInitialState<'info> {
    #[account()]
    pub battle: Account<'info, Battle>,
    #[account(
        init,
        payer = player,
        space = 8 + PlayerCommit::INIT_SPACE,
        seeds = [b"commit", battle.key().as_ref(), player.key().as_ref()],
        bump
    )]
    pub player_commit: Account<'info, PlayerCommit>,
    #[account(mut)]
    pub player: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimWinnings<'info> {
    #[account()]
    pub battle: Account<'info, Battle>,
    #[account(
        mut,
        seeds = [b"battle_vault", battle.key().as_ref()],
        bump
    )]
    pub battle_vault: SystemAccount<'info>,
    #[account(
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub winner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(predicted_winner: Pubkey, bet_amount: u64)]
pub struct PlaceBet<'info> {
    #[account()]
    pub battle: Account<'info, Battle>,
    #[account(
        init_if_needed,
        payer = bettor,
        space = 8 + BettingPool::INIT_SPACE,
        seeds = [b"betting_pool", battle.key().as_ref()],
        bump
    )]
    pub betting_pool: Account<'info, BettingPool>,
    #[account(
        init,
        payer = bettor,
        space = 8 + Bet::INIT_SPACE,
        seeds = [b"bet", battle.key().as_ref(), bettor.key().as_ref()],
        bump
    )]
    pub bet: Account<'info, Bet>,
    #[account(
        mut,
        seeds = [b"betting_vault", battle.key().as_ref()],
        bump
    )]
    pub betting_vault: SystemAccount<'info>,
    #[account(mut)]
    pub bettor: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimBetWinnings<'info> {
    #[account()]
    pub battle: Account<'info, Battle>,
    #[account(
        mut,
        seeds = [b"bet", battle.key().as_ref(), bettor.key().as_ref()],
        bump
    )]
    pub bet: Account<'info, Bet>,
    #[account(
        mut,
        seeds = [b"betting_vault", battle.key().as_ref()],
        bump
    )]
    pub betting_vault: SystemAccount<'info>,
    #[account(
        seeds = [b"global_state"],
        bump = global_state.bump
    )]
    pub global_state: Account<'info, GlobalState>,
    #[account(mut)]
    pub bettor: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct GlobalState {
    pub admin: Pubkey,
    pub merkle_root: [u8; 32],
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Battle {
    pub creator: Pubkey,
    pub battle_type: BattleType,
    pub league_amount: u64,
    pub max_players: u8,
    pub current_players: u8,
    #[max_len(6)]
    pub players: Vec<Pubkey>,
    pub start_time: i64,
    pub end_time: i64,
    pub is_active: bool,
    pub total_pool: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct BettingPool {
    pub battle: Pubkey,
    pub total_pool: u64,
    pub bets_on_player_a: u64,
    pub bets_on_player_b: u64,
    pub is_settled: bool,
}

#[account]
#[derive(InitSpace)]
pub struct Bet {
    pub bettor: Pubkey,
    pub battle: Pubkey,
    pub predicted_winner: Pubkey,
    pub amount: u64,
    pub is_claimed: bool,
}

#[account]
#[derive(InitSpace)]
pub struct PlayerCommit {
    pub battle: Pubkey,
    pub player: Pubkey,
    pub wallet_balance_hash: [u8; 32],
    pub timestamp: i64,
    pub is_verified: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq, InitSpace)]
pub enum BattleType {
    OneVsOne,
    Friends,
}

#[event]
pub struct BattleCreated {
    pub battle: Pubkey,
    pub creator: Pubkey,
    pub battle_type: BattleType,
    pub league_amount: u64,
    pub end_time: i64,
}

#[event]
pub struct PlayerJoined {
    pub battle: Pubkey,
    pub player: Pubkey,
    pub total_players: u8,
}

#[event]
pub struct WinningsClaimed {
    pub battle: Pubkey,
    pub winner: Pubkey,
    pub amount: u64,
}

#[event]
pub struct MerkleRootUpdated {
    pub merkle_root: [u8; 32],
    pub admin: Pubkey,
}

#[event]
pub struct BetPlaced {
    pub battle: Pubkey,
    pub bettor: Pubkey,
    pub predicted_winner: Pubkey,
    pub amount: u64,
}

#[event]
pub struct BetWinningsClaimed {
    pub battle: Pubkey,
    pub bettor: Pubkey,
    pub amount: u64,
}

#[event]
pub struct PlayerCommitted {
    pub battle: Pubkey,
    pub player: Pubkey,
    pub wallet_balance_hash: [u8; 32],
    pub timestamp: i64,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Not authorized admin")]
    NotAdmin,
    #[msg("Invalid battle duration")]
    InvalidDuration,
    #[msg("Battle is not active")]
    BattleNotActive,
    #[msg("Battle is full")]
    BattleFull,
    #[msg("Battle has ended")]
    BattleEnded,
    #[msg("Already joined this battle")]
    AlreadyJoined,
    #[msg("Battle has not ended yet")]
    BattleNotEnded,
    #[msg("Not a participant in this battle")]
    NotParticipant,
    #[msg("Invalid merkle proof")]
    InvalidMerkleProof,
    #[msg("Betting is only allowed for 1v1 battles")]
    BettingOnlyFor1v1,
    #[msg("Battle is not full yet")]
    BattleNotFull,
    #[msg("Invalid predicted winner")]
    InvalidPredictedWinner,
    #[msg("Battle participants cannot bet")]
    ParticipantCannotBet,
    #[msg("Not the owner of this bet")]
    NotBetOwner,
    #[msg("Bet winnings already claimed")]
    AlreadyClaimed,
}
