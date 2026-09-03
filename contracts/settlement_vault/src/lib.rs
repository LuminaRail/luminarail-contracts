#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

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
    use super::*;
    use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env};

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
}
