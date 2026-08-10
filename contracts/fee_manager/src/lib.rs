#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

pub const MAX_FEE_BPS: u32 = 1000; // 10.00% maximum fee limit
pub const BPS_DENOMINATOR: u128 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
    Unauthorized = 2,
    InvalidAmount = 3,
    InvalidFee = 7,
    NotInitialized = 8,
    Overflow = 10,
}

#[contracttype]
pub enum DataKey {
    Admin,
    FeeBps,
}

#[contract]
pub struct FeeManagerContract;

#[contractimpl]
impl FeeManagerContract {
    pub fn initialize(env: Env, admin: Address, initial_bps: u32) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }

        if initial_bps > MAX_FEE_BPS {
            return Err(Error::InvalidFee);
        }

        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::FeeBps, &initial_bps);
        Ok(())
    }

    pub fn set_fee_basis_points(env: Env, basis_points: u32) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;

        admin.require_auth();

        if basis_points > MAX_FEE_BPS {
            return Err(Error::InvalidFee);
        }

        env.storage()
            .instance()
            .set(&DataKey::FeeBps, &basis_points);
        Ok(())
    }

    pub fn get_fee_basis_points(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::FeeBps).unwrap_or(25) // Fallback default 25 bps (0.25%)
    }

    pub fn calculate_fee(env: Env, amount: i128) -> Result<i128, Error> {
        if amount < 0 {
            return Err(Error::InvalidAmount);
        }
        if amount == 0 {
            return Ok(0);
        }

        let bps = Self::get_fee_basis_points(env) as u128;
        let amt = amount as u128;

        let fee_scaled = amt.checked_mul(bps).ok_or(Error::Overflow)?;
        let fee = fee_scaled
            .checked_div(BPS_DENOMINATOR)
            .ok_or(Error::Overflow)?;

        Ok(fee as i128)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn test_fee_manager_initialization() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &50);
        assert_eq!(client.get_fee_basis_points(), 50);

        let fee = client.calculate_fee(&10_000);
        assert_eq!(fee, 50);
    }

    #[test]
    fn test_fee_manager_set_fee() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        client.initialize(&admin, &25);
        client.set_fee_basis_points(&100); // 1.00%
        assert_eq!(client.get_fee_basis_points(), 100);

        let fee = client.calculate_fee(&50_000);
        assert_eq!(fee, 500);
    }

    #[test]
    fn test_fee_manager_exceed_max_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);

        let err = client.try_initialize(&admin, &1001).unwrap_err().unwrap();
        assert_eq!(err, Error::InvalidFee);
    }

    #[test]
    fn test_fee_manager_zero_amount() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin, &25);

        assert_eq!(client.calculate_fee(&0), 0);
    }

    #[test]
    fn test_fee_manager_overflow_protection() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin, &1000);

        let large_amount = i128::MAX;
        let err = client
            .try_calculate_fee(&large_amount)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, Error::Overflow);
    }
}
