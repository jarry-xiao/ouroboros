use std::collections::VecDeque;

use borsh::BorshSerialize;
use ouroboros::InstructionNode;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_program_test::ProgramTest;
use solana_sdk::{signer::Signer, transaction::Transaction};

pub fn ouroboros_test(id: Pubkey) -> ProgramTest {
    let ctx = ProgramTest::new("ouroboros", id, None);
    ctx
}

#[tokio::test]
async fn test_ouroboros() {
    let program_id = Pubkey::new_unique();
    let mut ctx = ouroboros_test(program_id.clone())
        .start_with_context()
        .await;

    let mut simulation = VecDeque::new();
    simulation.push_back(InstructionNode {
        stack_depth: 0,
        compute_units: 20_000,
        accounts: vec![],
    });
    simulation.push_back(InstructionNode {
        stack_depth: 1,
        compute_units: 20_000,
        accounts: vec![],
    });
    simulation.push_back(InstructionNode {
        stack_depth: 2,
        compute_units: 7000,
        accounts: vec![],
    });
    simulation.push_back(InstructionNode {
        stack_depth: 2,
        compute_units: 7000,
        accounts: vec![],
    });
    simulation.push_back(InstructionNode {
        stack_depth: 1,
        compute_units: 20_000,
        accounts: vec![],
    });

    let mut transaction = Transaction::new_with_payer(
        &[Instruction {
            program_id: program_id.clone(),
            data: simulation.try_to_vec().unwrap(),
            accounts: vec![
                AccountMeta {
                    pubkey: program_id,
                    is_signer: false,
                    is_writable: false,
                },
                AccountMeta::new(ctx.payer.pubkey(), true),
            ],
        }],
        Some(&ctx.payer.pubkey()),
    );
    transaction.sign(
        &[&ctx.payer],
        ctx.banks_client.get_latest_blockhash().await.unwrap(),
    );
    ctx.banks_client
        .process_transaction(transaction)
        .await
        .unwrap();
}
