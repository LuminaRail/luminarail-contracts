#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidAmount = 3,
    NotFound = 4,
    AlreadyExists = 5,
    InvalidState = 6,
    NotInitialized = 7,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SettlementStatus {
    Pending = 0,
    Executed = 1,
    Failed = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementRecord {
    pub settlement_id: u64,
    pub source: Address,
    pub destination: Address,
    pub asset: Address,
    pub amount: i128,
    pub status: SettlementStatus,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Settlement(u64),
}

#[contract]
pub struct SettlementVaultContract;

#[contractimpl]
impl SettlementVaultContract {
    pub fn initialize(env: Env, admin: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        Ok(())
    }

    pub fn get_admin(env: Env) -> Result<Address, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)
    }

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
