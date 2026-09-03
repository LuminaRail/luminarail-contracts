#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, token, Address, Env};

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
}
