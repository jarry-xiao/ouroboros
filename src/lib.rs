use std::collections::VecDeque;

use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::instruction::{AccountMeta, Instruction};
use solana_program::keccak::hashv;
use solana_program::log::sol_log_compute_units;
use solana_program::msg;
use solana_program::pubkey::Pubkey;

use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
};

#[track_caller]
#[inline(always)]
pub fn assert_with_msg(v: bool, err: impl Into<ProgramError>, msg: &str) -> ProgramResult {
    if v {
        Ok(())
    } else {
        let caller = std::panic::Location::caller();
        msg!("{}. \n{}", msg, caller);
        Err(err.into())
    }
}

#[derive(Debug, Clone, BorshDeserialize, BorshSerialize)]
pub struct InstructionNode {
    pub stack_depth: u8,
    pub compute_units: u32,
    // Account indices
    pub accounts: Vec<u8>,
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let mut execution_tree = VecDeque::<InstructionNode>::try_from_slice(instruction_data)
        .map_err(|_| {
            msg!("Failed to decode stack");
            ProgramError::InvalidInstructionData
        })?;

    // Spin for a while to burn compute
    let (root, hash) = match execution_tree.pop_front() {
        Some(root) => {
            msg!(
                "Executing instruction at stack depth {} and budget {}",
                root.stack_depth,
                root.compute_units
            );
            sol_log_compute_units();
            // TODO: Figure out how to price this more accurately
            let mut hash = [0_u8; 32];
            for _ in 0..root.compute_units / 5000 {
                hash = hashv(&[&hash]).0;
            }
            sol_log_compute_units();
            (root, hash)
        }
        None => return Ok(()),
    };

    // Loop through CPI branches
    while !execution_tree.is_empty() {
        let node = execution_tree.pop_front().ok_or_else(|| {
            msg!("Failed to pop node from execution tree");
            ProgramError::InvalidInstructionData
        })?;
        let account_indices = node.accounts.clone();
        let stack_depth = node.stack_depth;
        assert_with_msg(
            stack_depth > root.stack_depth,
            ProgramError::InvalidInstructionData,
            "Stack depth must be greater than root stack depth",
        )?;
        let mut current_stack_depth = node.stack_depth as i8;
        let mut child_branch = VecDeque::<InstructionNode>::new();
        child_branch.push_back(node);
        while !execution_tree.is_empty() {
            // Peek the next node to see if it is a child of the current node
            let node = execution_tree
                .get(0)
                .ok_or_else(|| {
                    msg!("Stack is empty");
                    ProgramError::InvalidInstructionData
                })?
                .clone();
            assert_with_msg(
                current_stack_depth - node.stack_depth as i8 >= -1,
                ProgramError::InvalidInstructionData,
                &format!(
                    "Invalid stack depth. Current stack depth is {}, but next node has stack depth {}",
                    current_stack_depth,
                    node.stack_depth
                ),
            )?;
            if node.stack_depth > stack_depth {
                // We only add nodes to the child branch if they are at a deeper stack depth
                execution_tree.pop_front();
                current_stack_depth = node.stack_depth as i8;
                child_branch.push_back(node);
            } else {
                // Otherwise we break out of the loop
                break;
            }
        }

        // Recursively invoke the child branch
        let account_infos = account_indices
            .iter()
            .map(|i| accounts[*i as usize].clone())
            .collect::<Vec<_>>();
        solana_program::program::invoke(
            &Instruction {
                program_id: program_id.clone(),
                accounts: account_infos
                    .iter()
                    .map(|a| AccountMeta {
                        pubkey: *a.key,
                        is_signer: a.is_signer,
                        is_writable: a.is_writable,
                    })
                    .chain(vec![AccountMeta {
                        pubkey: *program_id,
                        is_signer: false,
                        is_writable: false,
                    }])
                    .collect::<Vec<_>>(),
                data: child_branch.try_to_vec()?,
            },
            &account_infos,
        )?;
    }

    msg!(
        "Instruction at stack depth {} consumed {} compute units. Hash: {}",
        root.stack_depth,
        root.compute_units,
        bs58::encode(hash).into_string()
    );
    Ok(())
}
