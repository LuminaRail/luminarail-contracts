#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, token, Address, Env, Symbol,
};

/// Custom errors for Settlement Vault operations.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract instance already initialized.
    AlreadyInitialized = 1,
    /// Invoker lacks administrative or entity authorization.
    Unauthorized = 2,
    /// Transfer amount is zero or negative.
    InvalidAmount = 3,
    /// Settlement record not found.
    NotFound = 4,
    /// Settlement ID already exists.
    AlreadyExists = 5,
    /// Invalid settlement status transition attempt.
    InvalidState = 6,
    /// Contract instance has not been initialized.
    NotInitialized = 7,
}

/// Status lifecycle state for a Settlement record.
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SettlementStatus {
    /// Settlement created and awaiting execution.
    Pending = 0,
    /// Settlement successfully executed and tokens transferred.
    Executed = 1,
    /// Settlement execution failed.
    Failed = 2,
}

/// Data structure representing an atomic settlement instruction.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementRecord {
    /// Unique identifier for the settlement.
    pub settlement_id: u64,
    /// Source funding address.
    pub source: Address,
    /// Destination receiving address.
    pub destination: Address,
    /// Asset contract address.
    pub asset: Address,
    /// Settlement amount.
    pub amount: i128,
    /// Current status of the settlement.
    pub status: SettlementStatus,
}

/// Storage keys for Settlement Vault state.
#[contracttype]
pub enum DataKey {
    /// Instance admin address.
    Admin,
    /// Persistent settlement record keyed by settlement_id.
    Settlement(u64),
}

/// Smart contract vault managing institutional multi-asset payment settlements on Soroban.
#[contract]
pub struct SettlementVaultContract;

#[contractimpl]
impl SettlementVaultContract {
    /// Initializes the vault instance with an administrative address.
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    /// Returns the active administrative address.
    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }

    /// Registers a new pending settlement record.
    /// Requires administrative authorization.
    pub fn create_settlement(
        env: Env,
        settlement_id: u64,
        source: Address,
        destination: Address,
        asset: Address,
        amount: i128,
    ) -> Result<SettlementRecord, Error> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }

        let key = DataKey::Settlement(settlement_id);
        if env.storage().persistent().has(&key) {
            return Err(Error::AlreadyExists);
        }

        let record = SettlementRecord {
            settlement_id,
            source,
            destination,
            asset,
            amount,
            status: SettlementStatus::Pending,
        };

        env.storage().persistent().set(&key, &record);

        env.events().publish(
            (Symbol::new(&env, "settlement_created"), settlement_id),
            (
                record.source.clone(),
                record.destination.clone(),
                record.asset.clone(),
                record.amount,
            ),
        );

        Ok(record)
    }

    /// Executes a pending settlement instruction by transferring tokens from `source` to `destination`.
    /// Requires dual authorization from both `admin` and `source`.
    pub fn execute_settlement(env: Env, settlement_id: u64) -> Result<SettlementRecord, Error> {
        let admin = Self::get_admin(env.clone())?;
        admin.require_auth();

        let key = DataKey::Settlement(settlement_id);
        let mut record: SettlementRecord = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::NotFound)?;

        if record.status != SettlementStatus::Pending {
            return Err(Error::InvalidState);
        }

        // Both admin and source must authorize settlement execution
        record.source.require_auth();

        // Perform token transfer from source to destination
        let token_client = token::Client::new(&env, &record.asset);
        token_client.transfer(&record.source, &record.destination, &record.amount);

        record.status = SettlementStatus::Executed;
        env.storage().persistent().set(&key, &record);

        env.events().publish(
            (Symbol::new(&env, "settlement_executed"), settlement_id),
            (
                record.source.clone(),
                record.destination.clone(),
                record.amount,
            ),
        );

        Ok(record)
    }

    /// Retrieves a settlement record by `settlement_id`.
    pub fn get_settlement(env: Env, settlement_id: u64) -> Result<SettlementRecord, Error> {
        let key = DataKey::Settlement(settlement_id);
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
    fn test_vault_full_lifecycle() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let source = Address::generate(&env);
        let destination = Address::generate(&env);
        let (token_address, token_admin) = setup_test_token(&env, &admin);

        token_admin.mint(&source, &5000);

        // 1. Initialize
        client.initialize(&admin);
        assert_eq!(client.get_admin(), admin);

        // 2. Create Settlement
        let created = client.create_settlement(&100, &source, &destination, &token_address, &2000);
        assert_eq!(created.status, SettlementStatus::Pending);

        // 3. Execute Settlement
        let executed = client.execute_settlement(&100);
        assert_eq!(executed.status, SettlementStatus::Executed);

        let token_client = token::Client::new(&env, &token_address);
        assert_eq!(token_client.balance(&source), 3000);
        assert_eq!(token_client.balance(&destination), 2000);
    }

    #[test]
    fn test_vault_events_emitted() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let source = Address::generate(&env);
        let destination = Address::generate(&env);
        let (token_address, token_admin) = setup_test_token(&env, &admin);

        token_admin.mint(&source, &5000);

        client.initialize(&admin);

        let settlement_id = 99u64;

        // 1. Create Settlement
        client.create_settlement(&settlement_id, &source, &destination, &token_address, &2000);

        let all_events = env.events().all();
        let contract_events: StdVec<_> = all_events
            .into_iter()
            .filter(|e| e.0 == contract_id)
            .collect();
        assert_eq!(contract_events.len(), 1);

        let event1 = contract_events.first().unwrap();
        assert_eq!(
            Symbol::try_from_val(&env, &event1.1.get(0).unwrap()).unwrap(),
            Symbol::new(&env, "settlement_created")
        );
        assert_eq!(
            u64::try_from_val(&env, &event1.1.get(1).unwrap()).unwrap(),
            settlement_id
        );

        // 2. Execute Settlement
        client.execute_settlement(&settlement_id);

        let all_events = env.events().all();
        let contract_events: StdVec<_> = all_events
            .into_iter()
            .filter(|e| e.0 == contract_id)
            .collect();
        assert_eq!(contract_events.len(), 1);

        let event2 = contract_events.first().unwrap();
        assert_eq!(
            Symbol::try_from_val(&env, &event2.1.get(0).unwrap()).unwrap(),
            Symbol::new(&env, "settlement_executed")
        );
        assert_eq!(
            u64::try_from_val(&env, &event2.1.get(1).unwrap()).unwrap(),
            settlement_id
        );
    }

    #[test]
    fn test_failed_vault_operations_emit_no_events() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let source = Address::generate(&env);
        let destination = Address::generate(&env);
        let (token_address, _) = setup_test_token(&env, &admin);

        client.initialize(&admin);

        // Attempt zero amount settlement -> error
        let _ = client.try_create_settlement(&1, &source, &destination, &token_address, &0);

        let contract_events: StdVec<_> = env
            .events()
            .all()
            .into_iter()
            .filter(|e| e.0 == contract_id)
            .collect();
        assert_eq!(contract_events.len(), 0);
    }

    #[test]
    fn test_vault_double_initialization_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let err = client.try_initialize(&admin).unwrap_err().unwrap();
        assert_eq!(err, Error::AlreadyInitialized);
    }

    #[test]
    fn test_vault_duplicate_settlement_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let source = Address::generate(&env);
        let destination = Address::generate(&env);
        let (token_address, _) = setup_test_token(&env, &admin);

        client.initialize(&admin);
        client.create_settlement(&1, &source, &destination, &token_address, &100);

        let err = client
            .try_create_settlement(&1, &source, &destination, &token_address, &100)
            .unwrap_err()
            .unwrap();

        assert_eq!(err, Error::AlreadyExists);
    }

    #[test]
    fn test_vault_zero_amount_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let source = Address::generate(&env);
        let destination = Address::generate(&env);
        let (token_address, _) = setup_test_token(&env, &admin);

        client.initialize(&admin);

        let err = client
            .try_create_settlement(&1, &source, &destination, &token_address, &0)
            .unwrap_err()
            .unwrap();

        assert_eq!(err, Error::InvalidAmount);
    }

    #[test]
    fn test_vault_double_execution_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let source = Address::generate(&env);
        let destination = Address::generate(&env);
        let (token_address, token_admin) = setup_test_token(&env, &admin);

        token_admin.mint(&source, &1000);

        client.initialize(&admin);
        client.create_settlement(&1, &source, &destination, &token_address, &500);
        client.execute_settlement(&1);

        let err = client.try_execute_settlement(&1).unwrap_err().unwrap();
        assert_eq!(err, Error::InvalidState);
    }

    // ---------------------------------------------------------------------------
    // Authorization matrix (issue #21)
    //
    // Privileged entrypoints: `initialize`, `create_settlement`,
    // `execute_settlement`. Matrix tests never rely on `mock_all_auths` for the
    // call under test: they provide exact `env.mock_auths()` entries so every
    // `require_auth` must match the precise (address, fn, args) invocation tree
    // or the call fails.
    // ---------------------------------------------------------------------------

    /// Initialized vault fixture. Auth mocking is left ENABLED after setup so
    /// tests can create settlements; call `enforce_real_auth` before the
    /// authorization matrix call under test.
    struct VaultAuthFixture {
        env: Env,
        contract_id: Address,
        admin: Address,
        source: Address,
        destination: Address,
        token_address: Address,
    }

    fn setup_vault_auth_fixture() -> VaultAuthFixture {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let source = Address::generate(&env);
        let destination = Address::generate(&env);
        let (token_address, token_admin) = setup_test_token(&env, &admin);

        token_admin.mint(&source, &5000);
        client.initialize(&admin);

        VaultAuthFixture {
            env,
            contract_id,
            admin,
            source,
            destination,
            token_address,
        }
    }

    fn create_pending_settlement(fixture: &VaultAuthFixture, settlement_id: u64, amount: i128) {
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);
        client.create_settlement(
            &settlement_id,
            &fixture.source,
            &fixture.destination,
            &fixture.token_address,
            &amount,
        );
    }

    /// Disables blanket auth mocking so only explicitly mocked authorizations pass.
    fn enforce_real_auth(env: &Env) {
        env.set_auths(&[]);
    }

    // -- initialize ------------------------------------------------------------

    #[test]
    fn test_vault_auth_initialize_admin_authorized_succeeds() {
        let env = Env::default();
        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let admin_invoke = MockAuthInvoke {
            contract: &contract_id,
            fn_name: "initialize",
            args: (&admin,).into_val(&env),
            sub_invokes: &[],
        };
        env.mock_auths(&[MockAuth {
            address: &admin,
            invoke: &admin_invoke,
        }]);

        client.initialize(&admin);
        assert_eq!(client.get_admin(), admin);
    }

    #[test]
    fn test_vault_auth_initialize_missing_authorization_rejected() {
        let env = Env::default();
        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        assert!(
            client.try_initialize(&admin).is_err(),
            "initialize must fail without admin authorization"
        );
        // The admin must not have been recorded.
        assert_eq!(
            client.try_get_admin().unwrap_err().unwrap(),
            Error::NotInitialized
        );
    }

    #[test]
    fn test_vault_auth_initialize_unrelated_authorization_rejected() {
        let env = Env::default();
        let contract_id = env.register(SettlementVaultContract, ());
        let client = SettlementVaultContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);

        // The attacker authorizes their own (attacker, initialize, (attacker))
        // invocation; it must not satisfy the admin's authorization.
        let attacker_invoke = MockAuthInvoke {
            contract: &contract_id,
            fn_name: "initialize",
            args: (&attacker,).into_val(&env),
            sub_invokes: &[],
        };
        env.mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &attacker_invoke,
        }]);

        assert!(
            client.try_initialize(&admin).is_err(),
            "unrelated authorization must not satisfy admin authorization"
        );
    }

    // -- create_settlement -------------------------------------------------------

    #[test]
    fn test_vault_auth_create_settlement_admin_authorized_succeeds() {
        let fixture = setup_vault_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        let settlement_id = 100u64;
        let amount = 2000i128;
        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "create_settlement",
            args: (
                &settlement_id,
                &fixture.source,
                &fixture.destination,
                &fixture.token_address,
                &amount,
            )
                .into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.admin,
            invoke: &admin_invoke,
        }]);

        let record = client.create_settlement(
            &settlement_id,
            &fixture.source,
            &fixture.destination,
            &fixture.token_address,
            &amount,
        );
        assert_eq!(record.status, SettlementStatus::Pending);
        assert_eq!(
            client.get_settlement(&settlement_id).status,
            SettlementStatus::Pending
        );
    }

    #[test]
    fn test_vault_auth_create_settlement_non_admin_rejected() {
        let fixture = setup_vault_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        let settlement_id = 100u64;
        let amount = 2000i128;
        let attacker = Address::generate(&fixture.env);
        // The attacker fully authorizes the very same invocation, but from a
        // non-admin address.
        let attacker_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "create_settlement",
            args: (
                &settlement_id,
                &fixture.source,
                &fixture.destination,
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
                .try_create_settlement(
                    &settlement_id,
                    &fixture.source,
                    &fixture.destination,
                    &fixture.token_address,
                    &amount
                )
                .is_err(),
            "non-admin caller must not be able to create a settlement"
        );
        // The settlement must not have been created.
        assert_eq!(
            client
                .try_get_settlement(&settlement_id)
                .unwrap_err()
                .unwrap(),
            Error::NotFound
        );
    }

    #[test]
    fn test_vault_auth_create_settlement_missing_authorization_rejected() {
        let fixture = setup_vault_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        let settlement_id = 100u64;
        let amount = 2000i128;
        assert!(
            client
                .try_create_settlement(
                    &settlement_id,
                    &fixture.source,
                    &fixture.destination,
                    &fixture.token_address,
                    &amount
                )
                .is_err(),
            "create_settlement must fail with no authorization provided"
        );
        assert_eq!(
            client
                .try_get_settlement(&settlement_id)
                .unwrap_err()
                .unwrap(),
            Error::NotFound
        );
    }

    #[test]
    fn test_vault_auth_create_settlement_unrelated_authorization_rejected() {
        let fixture = setup_vault_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        let settlement_id = 100u64;
        let amount = 2000i128;
        // Admin authorizes an unrelated read-only function instead.
        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "get_admin",
            args: ().into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.admin,
            invoke: &admin_invoke,
        }]);

        assert!(
            client
                .try_create_settlement(
                    &settlement_id,
                    &fixture.source,
                    &fixture.destination,
                    &fixture.token_address,
                    &amount
                )
                .is_err(),
            "authorization for an unrelated function must be rejected"
        );
    }

    // -- execute_settlement ------------------------------------------------------

    #[test]
    fn test_vault_auth_execute_settlement_admin_and_source_authorized_succeeds() {
        let fixture = setup_vault_auth_fixture();
        let settlement_id = 7u64;
        let amount = 2000i128;
        create_pending_settlement(&fixture, settlement_id, amount);
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        // Source authorizes both the vault execution and the nested token transfer.
        let transfer_invoke = MockAuthInvoke {
            contract: &fixture.token_address,
            fn_name: "transfer",
            args: (&fixture.source, &fixture.destination, &amount).into_val(&fixture.env),
            sub_invokes: &[],
        };
        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&settlement_id,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        let source_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&settlement_id,).into_val(&fixture.env),
            sub_invokes: &[transfer_invoke],
        };
        fixture.env.mock_auths(&[
            MockAuth {
                address: &fixture.admin,
                invoke: &admin_invoke,
            },
            MockAuth {
                address: &fixture.source,
                invoke: &source_invoke,
            },
        ]);

        let record = client.execute_settlement(&settlement_id);
        assert_eq!(record.status, SettlementStatus::Executed);

        let token_client = token::Client::new(&fixture.env, &fixture.token_address);
        assert_eq!(token_client.balance(&fixture.source), 3000);
        assert_eq!(token_client.balance(&fixture.destination), 2000);
    }

    #[test]
    fn test_vault_auth_execute_settlement_missing_admin_authorization_rejected() {
        let fixture = setup_vault_auth_fixture();
        let settlement_id = 7u64;
        let amount = 2000i128;
        create_pending_settlement(&fixture, settlement_id, amount);
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        // Only the source authorizes; the admin authorization is missing.
        let transfer_invoke = MockAuthInvoke {
            contract: &fixture.token_address,
            fn_name: "transfer",
            args: (&fixture.source, &fixture.destination, &amount).into_val(&fixture.env),
            sub_invokes: &[],
        };
        let source_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&settlement_id,).into_val(&fixture.env),
            sub_invokes: &[transfer_invoke],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.source,
            invoke: &source_invoke,
        }]);

        assert!(
            client.try_execute_settlement(&settlement_id).is_err(),
            "execution without admin authorization must be rejected"
        );
        assert_eq!(
            client.get_settlement(&settlement_id).status,
            SettlementStatus::Pending
        );
    }

    #[test]
    fn test_vault_auth_execute_settlement_missing_source_authorization_rejected() {
        let fixture = setup_vault_auth_fixture();
        let settlement_id = 7u64;
        let amount = 2000i128;
        create_pending_settlement(&fixture, settlement_id, amount);
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        // Only the admin authorizes; the source authorization is missing.
        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&settlement_id,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.admin,
            invoke: &admin_invoke,
        }]);

        assert!(
            client.try_execute_settlement(&settlement_id).is_err(),
            "execution without source authorization must be rejected"
        );
        assert_eq!(
            client.get_settlement(&settlement_id).status,
            SettlementStatus::Pending
        );
    }

    #[test]
    fn test_vault_auth_execute_settlement_missing_transfer_authorization_rejected() {
        let fixture = setup_vault_auth_fixture();
        let settlement_id = 7u64;
        let amount = 2000i128;
        create_pending_settlement(&fixture, settlement_id, amount);
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        // Admin and source both authorize execute_settlement, but the source
        // does not authorize the nested token transfer (partial authorization).
        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&settlement_id,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        let source_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&settlement_id,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[
            MockAuth {
                address: &fixture.admin,
                invoke: &admin_invoke,
            },
            MockAuth {
                address: &fixture.source,
                invoke: &source_invoke,
            },
        ]);

        assert!(
            client.try_execute_settlement(&settlement_id).is_err(),
            "execution without the nested transfer authorization must be rejected"
        );
        // No funds may have moved.
        let token_client = token::Client::new(&fixture.env, &fixture.token_address);
        assert_eq!(token_client.balance(&fixture.source), 5000);
        assert_eq!(token_client.balance(&fixture.destination), 0);
    }

    #[test]
    fn test_vault_auth_execute_settlement_unauthorized_source_rejected() {
        let fixture = setup_vault_auth_fixture();
        let settlement_id = 7u64;
        let amount = 2000i128;
        create_pending_settlement(&fixture, settlement_id, amount);
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        // An attacker authorizes the exact same invocation, but they are not
        // the settlement source.
        let attacker = Address::generate(&fixture.env);
        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&settlement_id,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        let attacker_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&settlement_id,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[
            MockAuth {
                address: &fixture.admin,
                invoke: &admin_invoke,
            },
            MockAuth {
                address: &attacker,
                invoke: &attacker_invoke,
            },
        ]);

        assert!(
            client.try_execute_settlement(&settlement_id).is_err(),
            "an address other than the settlement source must be rejected"
        );
        let token_client = token::Client::new(&fixture.env, &fixture.token_address);
        assert_eq!(token_client.balance(&fixture.source), 5000);
        assert_eq!(
            client.get_settlement(&settlement_id).status,
            SettlementStatus::Pending
        );
    }

    #[test]
    fn test_vault_auth_execute_settlement_wrong_args_rejected() {
        let fixture = setup_vault_auth_fixture();
        let settlement_id = 7u64;
        let amount = 2000i128;
        create_pending_settlement(&fixture, settlement_id, amount);
        enforce_real_auth(&fixture.env);
        let client = SettlementVaultContractClient::new(&fixture.env, &fixture.contract_id);

        // Authorizations recorded for a different settlement id must not
        // satisfy the execution of this settlement.
        let wrong_id = 99u64;
        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "execute_settlement",
            args: (&wrong_id,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.admin,
            invoke: &admin_invoke,
        }]);

        assert!(
            client.try_execute_settlement(&settlement_id).is_err(),
            "authorization with mismatched arguments must be rejected"
        );
        assert_eq!(
            client.get_settlement(&settlement_id).status,
            SettlementStatus::Pending
        );
    }
}
