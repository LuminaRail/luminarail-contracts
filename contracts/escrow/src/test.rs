extern crate std;

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Events},
    token::StellarAssetClient,
    Address, Env, Symbol, TryFromVal,
};
use std::vec::Vec as StdVec;

fn setup_test_token<'a>(env: &Env, admin: &Address) -> (Address, StellarAssetClient<'a>) {
    let token_id = env.register_stellar_asset_contract_v2(admin.clone());
    let token_address = token_id.address();
    let client = StellarAssetClient::new(env, &token_address);
    (token_address, client)
}

#[test]
fn test_escrow_full_lifecycle() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, token_admin_client) = setup_test_token(&env, &admin);

    token_admin_client.mint(&depositor, &1000);

    let escrow_id = 101u64;
    let amount = 500i128;

    // Step 1: Create
    let created = client.create_escrow(
        &escrow_id,
        &depositor,
        &beneficiary,
        &token_address,
        &amount,
    );
    assert_eq!(created.status, EscrowStatus::Created);

    // Step 2: Fund
    let funded = client.fund_escrow(&escrow_id, &amount);
    assert_eq!(funded.status, EscrowStatus::Funded);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&depositor), 500);
    assert_eq!(token_client.balance(&contract_id), 500);

    // Step 3: Release
    let released = client.release_escrow(&escrow_id, &admin);
    assert_eq!(released.status, EscrowStatus::Released);

    assert_eq!(token_client.balance(&contract_id), 0);
    assert_eq!(token_client.balance(&beneficiary), 500);
}

#[test]
fn test_invariant_cannot_be_funded_twice() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, token_admin) = setup_test_token(&env, &admin);

    token_admin.mint(&depositor, &2000);

    let escrow_id = 1u64;
    let amount = 500i128;

    client.create_escrow(&escrow_id, &depositor, &beneficiary, &token_address, &amount);
    client.fund_escrow(&escrow_id, &amount);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&contract_id), 500);
    assert_eq!(token_client.balance(&depositor), 1500);

    // Attempt double funding -> must fail with Error::InvalidState
    let err = client.try_fund_escrow(&escrow_id, &amount).unwrap_err().unwrap();
    assert_eq!(err, Error::InvalidState);

    // Assert balances and state remain invariant
    assert_eq!(token_client.balance(&contract_id), 500);
    assert_eq!(token_client.balance(&depositor), 1500);
    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Funded);
}

#[test]
fn test_invariant_released_cannot_be_released_again() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, token_admin) = setup_test_token(&env, &admin);

    token_admin.mint(&depositor, &1000);

    let escrow_id = 1u64;
    let amount = 500i128;

    client.create_escrow(&escrow_id, &depositor, &beneficiary, &token_address, &amount);
    client.fund_escrow(&escrow_id, &amount);
    client.release_escrow(&escrow_id, &admin);

    let token_client = token::Client::new(&env, &token_address);
    assert_eq!(token_client.balance(&beneficiary), 500);
    assert_eq!(token_client.balance(&contract_id), 0);

    // Attempt second release -> must fail with Error::InvalidState
    let err = client.try_release_escrow(&escrow_id, &admin).unwrap_err().unwrap();
    assert_eq!(err, Error::InvalidState);

    // Assert beneficiary balance does not increase twice and status remains Released
    assert_eq!(token_client.balance(&beneficiary), 500);
    assert_eq!(token_client.balance(&contract_id), 0);
    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Released);
}

#[test]
#[should_panic]
fn test_invariant_unauthorized_create_fails() {
    let env = Env::default();
    // mock_all_auths() is intentionally omitted to verify auth enforcement
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, _) = setup_test_token(&env, &admin);

    // Call without mock_all_auths must panic due to unauthorized invoker
    let _ = client.create_escrow(&1, &depositor, &beneficiary, &token_address, &500);
}

#[test]
#[should_panic]
fn test_invariant_unauthorized_fund_fails() {
    let env = Env::default();
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, _) = setup_test_token(&env, &admin);

    // Call without mock_all_auths must panic
    let _ = client.fund_escrow(&1, &500);
}

#[test]
#[should_panic]
fn test_invariant_unauthorized_release_fails() {
    let env = Env::default();
    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Call without mock_all_auths must panic
    let _ = client.release_escrow(&1, &admin);
}

#[test]
fn test_invariant_invalid_amounts_fail() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, token_admin) = setup_test_token(&env, &admin);

    token_admin.mint(&depositor, &1000);

    // Zero amount creation fails
    let err_zero = client
        .try_create_escrow(&1, &depositor, &beneficiary, &token_address, &0)
        .unwrap_err()
        .unwrap();
    assert_eq!(err_zero, Error::InvalidAmount);

    // Negative amount creation fails
    let err_neg = client
        .try_create_escrow(&2, &depositor, &beneficiary, &token_address, &-100)
        .unwrap_err()
        .unwrap();
    assert_eq!(err_neg, Error::InvalidAmount);

    // Setup valid escrow for funding tests
    client.create_escrow(&3, &depositor, &beneficiary, &token_address, &500);

    // Funding with mismatched lower amount fails
    let err_mismatch_low = client.try_fund_escrow(&3, &300).unwrap_err().unwrap();
    assert_eq!(err_mismatch_low, Error::InvalidAmount);

    // Funding with mismatched higher amount fails
    let err_mismatch_high = client.try_fund_escrow(&3, &600).unwrap_err().unwrap();
    assert_eq!(err_mismatch_high, Error::InvalidAmount);
}

#[test]
fn test_invariant_invalid_destinations_fail() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);

    // Non-existent escrow lookup fails
    let err_get = client.try_get_escrow(&9999).unwrap_err().unwrap();
    assert_eq!(err_get, Error::NotFound);

    // Non-existent escrow funding fails
    let err_fund = client.try_fund_escrow(&9999, &500).unwrap_err().unwrap();
    assert_eq!(err_fund, Error::NotFound);

    // Non-existent escrow release fails
    let err_release = client.try_release_escrow(&9999, &admin).unwrap_err().unwrap();
    assert_eq!(err_release, Error::NotFound);
}

#[test]
fn test_invariant_release_unfunded_escrow_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, _) = setup_test_token(&env, &admin);

    client.create_escrow(&1, &depositor, &beneficiary, &token_address, &500);

    // Releasing an escrow in Created status must fail with InvalidState
    let err = client.try_release_escrow(&1, &admin).unwrap_err().unwrap();
    assert_eq!(err, Error::InvalidState);
}

#[test]
fn test_invariant_completed_escrow_cannot_return_to_active_state() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, token_admin) = setup_test_token(&env, &admin);

    token_admin.mint(&depositor, &1000);

    let escrow_id = 100u64;
    let amount = 500i128;

    // Transition to terminal state: Created -> Funded -> Released
    client.create_escrow(&escrow_id, &depositor, &beneficiary, &token_address, &amount);
    client.fund_escrow(&escrow_id, &amount);
    client.release_escrow(&escrow_id, &admin);

    let escrow = client.get_escrow(&escrow_id);
    assert_eq!(escrow.status, EscrowStatus::Released);

    // Attempt re-funding released escrow -> fails with InvalidState
    let err_fund = client.try_fund_escrow(&escrow_id, &amount).unwrap_err().unwrap();
    assert_eq!(err_fund, Error::InvalidState);

    // Attempt re-creating escrow with same ID -> fails with AlreadyExists
    let err_create = client
        .try_create_escrow(&escrow_id, &depositor, &beneficiary, &token_address, &amount)
        .unwrap_err()
        .unwrap();
    assert_eq!(err_create, Error::AlreadyExists);

    // Verify status remains strictly Released
    let escrow_final = client.get_escrow(&escrow_id);
    assert_eq!(escrow_final.status, EscrowStatus::Released);
}

#[test]
fn test_escrow_events_emitted() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, token_admin) = setup_test_token(&env, &admin);

    token_admin.mint(&depositor, &1000);

    let escrow_id = 42u64;
    let amount = 500i128;

    // 1. Create
    client.create_escrow(
        &escrow_id,
        &depositor,
        &beneficiary,
        &token_address,
        &amount,
    );

    let all_events = env.events().all();
    let contract_events: StdVec<_> = all_events
        .into_iter()
        .filter(|e| e.0 == contract_id)
        .collect();
    assert_eq!(contract_events.len(), 1);

    let event1 = contract_events.first().unwrap();
    assert_eq!(
        Symbol::try_from_val(&env, &event1.1.get(0).unwrap()).unwrap(),
        Symbol::new(&env, "escrow_created")
    );
    assert_eq!(
        u64::try_from_val(&env, &event1.1.get(1).unwrap()).unwrap(),
        escrow_id
    );

    // 2. Fund
    client.fund_escrow(&escrow_id, &amount);

    let all_events = env.events().all();
    let contract_events: StdVec<_> = all_events
        .into_iter()
        .filter(|e| e.0 == contract_id)
        .collect();
    assert_eq!(contract_events.len(), 1);

    let event2 = contract_events.first().unwrap();
    assert_eq!(
        Symbol::try_from_val(&env, &event2.1.get(0).unwrap()).unwrap(),
        Symbol::new(&env, "escrow_funded")
    );
    assert_eq!(
        u64::try_from_val(&env, &event2.1.get(1).unwrap()).unwrap(),
        escrow_id
    );

    // 3. Release
    client.release_escrow(&escrow_id, &admin);

    let all_events = env.events().all();
    let contract_events: StdVec<_> = all_events
        .into_iter()
        .filter(|e| e.0 == contract_id)
        .collect();
    assert_eq!(contract_events.len(), 1);

    let event3 = contract_events.first().unwrap();
    assert_eq!(
        Symbol::try_from_val(&env, &event3.1.get(0).unwrap()).unwrap(),
        Symbol::new(&env, "escrow_released")
    );
    assert_eq!(
        u64::try_from_val(&env, &event3.1.get(1).unwrap()).unwrap(),
        escrow_id
    );
}

#[test]
fn test_failed_escrow_operations_emit_no_events() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, _) = setup_test_token(&env, &admin);

    // Attempt creation with zero amount -> error
    let _ = client.try_create_escrow(&1, &depositor, &beneficiary, &token_address, &0);

    let contract_events: StdVec<_> = env
        .events()
        .all()
        .into_iter()
        .filter(|e| e.0 == contract_id)
        .collect();
    assert_eq!(contract_events.len(), 0);
}

#[test]
fn test_escrow_duplicate_id_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(EscrowContract, ());
    let client = EscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let depositor = Address::generate(&env);
    let beneficiary = Address::generate(&env);
    let (token_address, _) = setup_test_token(&env, &admin);

    client.create_escrow(&1, &depositor, &beneficiary, &token_address, &100);

    let err = client
        .try_create_escrow(&1, &depositor, &beneficiary, &token_address, &100)
        .unwrap_err()
        .unwrap();

    assert_eq!(err, Error::AlreadyExists);
}
