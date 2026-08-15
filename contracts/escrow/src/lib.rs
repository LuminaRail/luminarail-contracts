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
    InvalidFee = 7,
    AlreadyFunded = 8,
    AlreadyReleased = 9,
    Overflow = 10,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowStatus {
    Created = 0,
    Funded = 1,
    Released = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Escrow {
    pub escrow_id: u64,
    pub depositor: Address,
    pub beneficiary: Address,
    pub asset: Address,
    pub amount: i128,
    pub status: EscrowStatus,
}

#[contracttype]
pub enum DataKey {
    Escrow(u64),
}

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
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
        Ok(escrow)
    }

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
        Ok(escrow)
    }

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
        Ok(escrow)
    }

    pub fn get_escrow(env: Env, escrow_id: u64) -> Result<Escrow, Error> {
        let key = DataKey::Escrow(escrow_id);
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
}
