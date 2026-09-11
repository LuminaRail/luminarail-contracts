#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Env, Symbol,
};

/// Custom errors for Escrow contract operations.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract instance or escrow state already initialized.
    AlreadyInitialized = 1,
    /// Invoker lacks necessary authorization.
    Unauthorized = 2,
    /// Amount supplied is zero or negative.
    InvalidAmount = 3,
    /// Escrow record not found for the given ID.
    NotFound = 4,
    /// Escrow with given ID already exists.
    AlreadyExists = 5,
    /// Escrow state machine transition is invalid.
    InvalidState = 6,
    /// Fee configuration or calculation is invalid.
    InvalidFee = 7,
    /// Escrow has already been funded.
    AlreadyFunded = 8,
    /// Escrow has already been released.
    AlreadyReleased = 9,
    /// Numeric overflow during calculation.
    Overflow = 10,
}

/// Lifecycle status of an Escrow record.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowStatus {
    /// Escrow record created but unfunded.
    Created = 0,
    /// Escrow record funded with token assets.
    Funded = 1,
    /// Escrow record released to beneficiary.
    Released = 2,
}

/// Data structure representing a multi-party escrow agreement.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    /// Unique identifier for the escrow.
    pub escrow_id: u64,
    /// Address of the depositing party.
    pub depositor: Address,
    /// Address of the beneficiary receiving funds upon release.
    pub beneficiary: Address,
    /// Stellar asset contract address.
    pub asset: Address,
    /// Escrow token amount in smallest asset units.
    pub amount: i128,
    /// Current status of the escrow.
    pub status: EscrowStatus,
}

/// Persistent storage key enum for the Escrow contract.
#[contracttype]
pub enum DataKey {
    /// Escrow record keyed by escrow_id.
    Escrow(u64),
}

/// Smart contract managing multi-party asset escrow on Stellar/Soroban.
#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    /// Creates a new escrow record in `Created` status.
    /// Requires authorization from `depositor`.
    pub fn create_escrow(
        env: Env,
        escrow_id: u64,
        depositor: Address,
        beneficiary: Address,
        asset: Address,
        amount: i128,
    ) -> Result<Escrow, Error> {
        depositor.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let key = DataKey::Escrow(escrow_id);
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }

        let escrow = Escrow {
            escrow_id,
            depositor,
            beneficiary,
            asset,
            amount,
            status: EscrowStatus::Created,
        };

        env.storage().persistent().set(&key, &escrow);

        env.events().publish(
            (Symbol::new(&env, "escrow_created"), escrow_id),
            (
                escrow.depositor.clone(),
                escrow.beneficiary.clone(),
                escrow.asset.clone(),
                escrow.amount,
            ),
        );

        Ok(escrow)
    }

    /// Funds an existing `Created` escrow by transferring assets from `depositor` to the contract.
    /// Requires authorization from `depositor`.
    pub fn fund_escrow(env: Env, escrow_id: u64, amount: i128) -> Result<Escrow, Error> {
        let key = DataKey::Escrow(escrow_id);
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;

        escrow.depositor.require_auth();

        if escrow.status != EscrowStatus::Created {
            return Err(Error::InvalidState);
        }

        if amount != escrow.amount {
            return Err(Error::InvalidAmount);
        }

        // Transfer tokens from depositor to escrow contract
        let token_client = token::Client::new(&env, &escrow.asset);
        token_client.transfer(&escrow.depositor, &env.current_contract_address(), &amount);

        escrow.status = EscrowStatus::Funded;
        env.storage().persistent().set(&key, &escrow);

        env.events().publish(
            (Symbol::new(&env, "escrow_funded"), escrow_id),
            (escrow.depositor.clone(), escrow.amount),
        );

        Ok(escrow)
    }

    /// Releases a `Funded` escrow by transferring assets from contract to `beneficiary`.
    /// Requires authorization from `release_authority`.
    pub fn release_escrow(
        env: Env,
        escrow_id: u64,
        release_authority: Address,
    ) -> Result<Escrow, Error> {
        release_authority.require_auth();

        let key = DataKey::Escrow(escrow_id);
        let mut escrow: Escrow = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;

        if escrow.status != EscrowStatus::Funded {
            return Err(Error::InvalidState);
        }

        // Transfer tokens from escrow contract to beneficiary
        let token_client = token::Client::new(&env, &escrow.asset);
        token_client.transfer(
            &env.current_contract_address(),
            &escrow.beneficiary,
            &escrow.amount,
        );

        escrow.status = EscrowStatus::Released;
        env.storage().persistent().set(&key, &escrow);

        env.events().publish(
            (Symbol::new(&env, "escrow_released"), escrow_id),
            (escrow.beneficiary.clone(), escrow.amount),
        );

        Ok(escrow)
    }

    /// Retrieves an escrow record by `escrow_id`.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<Escrow, Error> {
        let key = DataKey::Escrow(escrow_id);
        env.storage().persistent().get(&key).ok_or(Error::NotFound)
    }
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Events, MockAuth, MockAuthInvoke},
        token::StellarAssetClient,
        Address, Env, IntoVal, Symbol, TryFromVal,
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

        // Mint initial tokens to depositor
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

    #[test]
    fn test_escrow_zero_amount_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(EscrowContract, ());
        let client = EscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let (token_address, _) = setup_test_token(&env, &admin);

        let err = client
            .try_create_escrow(&1, &depositor, &beneficiary, &token_address, &0)
            .unwrap_err()
            .unwrap();

        assert_eq!(err, Error::InvalidAmount);
    }

    #[test]
    fn test_escrow_double_funding_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(EscrowContract, ());
        let client = EscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let (token_address, token_admin) = setup_test_token(&env, &admin);

        token_admin.mint(&depositor, &2000);

        client.create_escrow(&1, &depositor, &beneficiary, &token_address, &500);
        client.fund_escrow(&1, &500);

        let err = client.try_fund_escrow(&1, &500).unwrap_err().unwrap();
        assert_eq!(err, Error::InvalidState);
    }

    #[test]
    fn test_escrow_double_release_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(EscrowContract, ());
        let client = EscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let (token_address, token_admin) = setup_test_token(&env, &admin);

        token_admin.mint(&depositor, &1000);

        client.create_escrow(&1, &depositor, &beneficiary, &token_address, &500);
        client.fund_escrow(&1, &500);
        client.release_escrow(&1, &admin);

        let err = client.try_release_escrow(&1, &admin).unwrap_err().unwrap();
        assert_eq!(err, Error::InvalidState);
    }

    #[test]
    fn test_escrow_not_found_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(EscrowContract, ());
        let client = EscrowContractClient::new(&env, &contract_id);

        let err = client.try_get_escrow(&9999).unwrap_err().unwrap();
        assert_eq!(err, Error::NotFound);
    }

    #[test]
    fn test_escrow_funding_amount_mismatch_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(EscrowContract, ());
        let client = EscrowContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let depositor = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let (token_address, token_admin) = setup_test_token(&env, &admin);

        token_admin.mint(&depositor, &1000);

        client.create_escrow(&1, &depositor, &beneficiary, &token_address, &500);

        // Attempting to fund with mismatched amount 300 instead of 500
        let err = client.try_fund_escrow(&1, &300).unwrap_err().unwrap();
        assert_eq!(err, Error::InvalidAmount);
    }

    // ---------------------------------------------------------------------------
    // Authorization matrix (issue #21)
    //
    // Privileged entrypoints: `create_escrow`, `fund_escrow`, `release_escrow`.
    // Matrix tests never rely on `mock_all_auths` for the call under test: they
    // provide exact `env.mock_auths()` entries so every `require_auth` must match
    // the precise (address, fn, args) invocation tree or the call fails.
    // ---------------------------------------------------------------------------

    /// Initialized escrow fixture. Auth mocking is left ENABLED after setup so
    /// tests can create escrows; call `enforce_real_auth` before the
    /// authorization matrix call under test.
    struct EscrowAuthFixture {
        env: Env,
        contract_id: Address,
        depositor: Address,
        beneficiary: Address,
        token_address: Address,
    }

    fn setup_escrow_auth_fixture() -> EscrowAuthFixture {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(EscrowContract, ());

        let depositor = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let (token_address, token_admin) = setup_test_token(&env, &depositor);

        token_admin.mint(&depositor, &5000);

        EscrowAuthFixture {
            env,
            contract_id,
            depositor,
            beneficiary,
            token_address,
        }
    }

    fn create_pending_escrow(fixture: &EscrowAuthFixture, escrow_id: u64, amount: i128) {
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);
        client.create_escrow(
            &escrow_id,
            &fixture.depositor,
            &fixture.beneficiary,
            &fixture.token_address,
            &amount,
        );
    }

    fn fund_pending_escrow(fixture: &EscrowAuthFixture, escrow_id: u64, amount: i128) {
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);
        client.fund_escrow(&escrow_id, &amount);
    }

    /// Disables blanket auth mocking so only explicitly mocked authorizations pass.
    fn enforce_real_auth(env: &Env) {
        env.set_auths(&[]);
    }

    // -- create_escrow -----------------------------------------------------------

    #[test]
    fn test_escrow_auth_create_depositor_authorized_succeeds() {
        let fixture = setup_escrow_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        let escrow_id = 50u64;
        let amount = 500i128;
        let depositor_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "create_escrow",
            args: (
                &escrow_id,
                &fixture.depositor,
                &fixture.beneficiary,
                &fixture.token_address,
                &amount,
            )
                .into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.depositor,
            invoke: &depositor_invoke,
        }]);

        let escrow = client.create_escrow(
            &escrow_id,
            &fixture.depositor,
            &fixture.beneficiary,
            &fixture.token_address,
            &amount,
        );
        assert_eq!(escrow.status, EscrowStatus::Created);
    }

    #[test]
    fn test_escrow_auth_create_non_depositor_rejected() {
        let fixture = setup_escrow_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        let escrow_id = 50u64;
        let amount = 500i128;
        let attacker = Address::generate(&fixture.env);
        // The attacker authorizes a creation attempt naming themselves as the
        // depositor; it must not satisfy the real depositor's authorization.
        let attacker_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "create_escrow",
            args: (
                &escrow_id,
                &attacker,
                &fixture.beneficiary,
                &fixture.token_address,
                &amount,
            )
                .into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &attacker_invoke,
        }]);

        assert!(
            client
                .try_create_escrow(
                    &escrow_id,
                    &fixture.depositor,
                    &fixture.beneficiary,
                    &fixture.token_address,
                    &amount
                )
                .is_err(),
            "non-depositor caller must not be able to create an escrow"
        );
        assert_eq!(
            client.try_get_escrow(&escrow_id).unwrap_err().unwrap(),
            Error::NotFound
        );
    }

    #[test]
    fn test_escrow_auth_create_missing_authorization_rejected() {
        let fixture = setup_escrow_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        let escrow_id = 50u64;
        let amount = 500i128;
        assert!(
            client
                .try_create_escrow(
                    &escrow_id,
                    &fixture.depositor,
                    &fixture.beneficiary,
                    &fixture.token_address,
                    &amount
                )
                .is_err(),
            "create_escrow must fail with no authorization provided"
        );
        assert_eq!(
            client.try_get_escrow(&escrow_id).unwrap_err().unwrap(),
            Error::NotFound
        );
    }

    // -- fund_escrow -------------------------------------------------------------

    #[test]
    fn test_escrow_auth_fund_depositor_authorized_succeeds() {
        let fixture = setup_escrow_auth_fixture();
        let escrow_id = 60u64;
        let amount = 500i128;
        create_pending_escrow(&fixture, escrow_id, amount);
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        // Depositor authorizes both the escrow funding and the nested transfer.
        let transfer_invoke = MockAuthInvoke {
            contract: &fixture.token_address,
            fn_name: "transfer",
            args: (&fixture.depositor, &fixture.contract_id, &amount).into_val(&fixture.env),
            sub_invokes: &[],
        };
        let depositor_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "fund_escrow",
            args: (&escrow_id, &amount).into_val(&fixture.env),
            sub_invokes: &[transfer_invoke],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.depositor,
            invoke: &depositor_invoke,
        }]);

        let escrow = client.fund_escrow(&escrow_id, &amount);
        assert_eq!(escrow.status, EscrowStatus::Funded);

        let token_client = token::Client::new(&fixture.env, &fixture.token_address);
        assert_eq!(token_client.balance(&fixture.contract_id), amount);
    }

    #[test]
    fn test_escrow_auth_fund_non_depositor_rejected() {
        let fixture = setup_escrow_auth_fixture();
        let escrow_id = 60u64;
        let amount = 500i128;
        create_pending_escrow(&fixture, escrow_id, amount);
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        // An unrelated account authorizes the exact funding invocation.
        let attacker = Address::generate(&fixture.env);
        let attacker_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "fund_escrow",
            args: (&escrow_id, &amount).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &attacker_invoke,
        }]);

        assert!(
            client.try_fund_escrow(&escrow_id, &amount).is_err(),
            "non-depositor must not be able to fund an escrow"
        );
        assert_eq!(client.get_escrow(&escrow_id).status, EscrowStatus::Created);
    }

    #[test]
    fn test_escrow_auth_fund_missing_authorization_rejected() {
        let fixture = setup_escrow_auth_fixture();
        let escrow_id = 60u64;
        let amount = 500i128;
        create_pending_escrow(&fixture, escrow_id, amount);
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        assert!(
            client.try_fund_escrow(&escrow_id, &amount).is_err(),
            "fund_escrow must fail with no authorization provided"
        );
        assert_eq!(client.get_escrow(&escrow_id).status, EscrowStatus::Created);
    }

    #[test]
    fn test_escrow_auth_fund_missing_transfer_authorization_rejected() {
        let fixture = setup_escrow_auth_fixture();
        let escrow_id = 60u64;
        let amount = 500i128;
        create_pending_escrow(&fixture, escrow_id, amount);
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        // Depositor authorizes fund_escrow but not the nested token transfer
        // (partial authorization).
        let depositor_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "fund_escrow",
            args: (&escrow_id, &amount).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.depositor,
            invoke: &depositor_invoke,
        }]);

        assert!(
            client.try_fund_escrow(&escrow_id, &amount).is_err(),
            "funding without the nested transfer authorization must be rejected"
        );
        let token_client = token::Client::new(&fixture.env, &fixture.token_address);
        assert_eq!(token_client.balance(&fixture.depositor), 5000);
        assert_eq!(client.get_escrow(&escrow_id).status, EscrowStatus::Created);
    }

    // -- release_escrow ----------------------------------------------------------

    #[test]
    fn test_escrow_auth_release_authority_authorized_succeeds() {
        let fixture = setup_escrow_auth_fixture();
        let escrow_id = 70u64;
        let amount = 500i128;
        create_pending_escrow(&fixture, escrow_id, amount);
        fund_pending_escrow(&fixture, escrow_id, amount);
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        // The depositor acts as the release authority in this scenario.
        let authority_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "release_escrow",
            args: (&escrow_id, &fixture.depositor).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.depositor,
            invoke: &authority_invoke,
        }]);

        let escrow = client.release_escrow(&escrow_id, &fixture.depositor);
        assert_eq!(escrow.status, EscrowStatus::Released);

        let token_client = token::Client::new(&fixture.env, &fixture.token_address);
        assert_eq!(token_client.balance(&fixture.beneficiary), amount);
    }

    #[test]
    fn test_escrow_auth_release_non_authority_rejected() {
        let fixture = setup_escrow_auth_fixture();
        let escrow_id = 70u64;
        let amount = 500i128;
        create_pending_escrow(&fixture, escrow_id, amount);
        fund_pending_escrow(&fixture, escrow_id, amount);
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        // An attacker authorizes the exact release invocation but is not the
        // named release authority.
        let attacker = Address::generate(&fixture.env);
        let attacker_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "release_escrow",
            args: (&escrow_id, &fixture.depositor).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &attacker_invoke,
        }]);

        assert!(
            client
                .try_release_escrow(&escrow_id, &fixture.depositor)
                .is_err(),
            "an address other than the release authority must be rejected"
        );
        assert_eq!(client.get_escrow(&escrow_id).status, EscrowStatus::Funded);
        let token_client = token::Client::new(&fixture.env, &fixture.token_address);
        assert_eq!(token_client.balance(&fixture.beneficiary), 0);
    }

    #[test]
    fn test_escrow_auth_release_missing_authorization_rejected() {
        let fixture = setup_escrow_auth_fixture();
        let escrow_id = 70u64;
        let amount = 500i128;
        create_pending_escrow(&fixture, escrow_id, amount);
        fund_pending_escrow(&fixture, escrow_id, amount);
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        assert!(
            client
                .try_release_escrow(&escrow_id, &fixture.depositor)
                .is_err(),
            "release_escrow must fail with no authorization provided"
        );
        assert_eq!(client.get_escrow(&escrow_id).status, EscrowStatus::Funded);
    }

    #[test]
    fn test_escrow_auth_release_unrelated_authorization_rejected() {
        let fixture = setup_escrow_auth_fixture();
        let escrow_id = 70u64;
        let amount = 500i128;
        create_pending_escrow(&fixture, escrow_id, amount);
        fund_pending_escrow(&fixture, escrow_id, amount);
        enforce_real_auth(&fixture.env);
        let client = EscrowContractClient::new(&fixture.env, &fixture.contract_id);

        // The authority authorizes an unrelated function instead of the release.
        let authority_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "get_escrow",
            args: (&escrow_id,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.depositor,
            invoke: &authority_invoke,
        }]);

        assert!(
            client
                .try_release_escrow(&escrow_id, &fixture.depositor)
                .is_err(),
            "authorization for an unrelated function must be rejected"
        );
        assert_eq!(client.get_escrow(&escrow_id).status, EscrowStatus::Funded);
    }
}
