#![no_std]
use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

/// Maximum allowed fee limit in basis points (1000 BPS = 10.00%).
pub const MAX_FEE_BPS: u32 = 1000;
/// Denominator used for basis point calculations (10,000 BPS = 100%).
pub const BPS_DENOMINATOR: u128 = 10_000;

/// Custom errors for Fee Manager contract operations.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Contract instance already initialized.
    AlreadyInitialized = 1,
    /// Invoker lacks administrative authorization.
    Unauthorized = 2,
    /// Amount supplied is negative.
    InvalidAmount = 3,
    /// Fee basis points exceed MAX_FEE_BPS.
    InvalidFee = 7,
    /// Fee Manager contract not initialized.
    NotInitialized = 8,
    /// Arithmetic overflow in calculation.
    Overflow = 10,
}

/// Storage keys for Fee Manager state.
#[contracttype]
pub enum DataKey {
    /// Administrative address.
    Admin,
    /// Active fee in basis points.
    FeeBps,
}

/// Smart contract managing protocol fee calculation rules and bounds.
#[contract]
pub struct FeeManagerContract;

#[contractimpl]
impl FeeManagerContract {
    /// Initializes the fee manager instance with admin and initial BPS.
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

    /// Updates active fee basis points. Requires admin authorization.
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

    /// Reads active fee in basis points. Defaults to 25 BPS (0.25%).
    pub fn get_fee_basis_points(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::FeeBps).unwrap_or(25)
    }

    /// Calculates fee amount for a given input token amount using checked math.
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
    use soroban_sdk::{
        testutils::{Address as _, MockAuth, MockAuthInvoke},
        Address, Env, IntoVal,
    };

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

        client.initialize(&admin, &100);
        let err2 = client.try_set_fee_basis_points(&1001).unwrap_err().unwrap();
        assert_eq!(err2, Error::InvalidFee);
    }

    #[test]
    fn test_fee_manager_boundary_limits() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin, &1); // 1 BPS (0.01%)

        // 10,000 tokens * 1 BPS / 10,000 = 1 token
        assert_eq!(client.calculate_fee(&10_000), 1);

        client.set_fee_basis_points(&1000); // 1000 BPS (10.00%)
                                            // 10,000 tokens * 1000 BPS / 10,000 = 1,000 tokens
        assert_eq!(client.calculate_fee(&10_000), 1000);
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

    #[test]
    fn test_fee_manager_negative_amount_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin, &25);

        let err = client.try_calculate_fee(&-100).unwrap_err().unwrap();
        assert_eq!(err, Error::InvalidAmount);
    }

    #[test]
    fn test_fee_manager_uninitialized_access_rejected() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let err = client.try_set_fee_basis_points(&50).unwrap_err().unwrap();
        assert_eq!(err, Error::NotInitialized);
    }

    // ---------------------------------------------------------------------------
    // Authorization matrix (issue #21)
    //
    // Privileged entrypoints: `initialize`, `set_fee_basis_points`.
    // Matrix tests never rely on `mock_all_auths` for the call under test: they
    // provide exact `env.mock_auths()` entries so every `require_auth` must match
    // the precise (address, fn, args) invocation tree or the call fails.
    // ---------------------------------------------------------------------------

    /// Initialized fee manager fixture. Auth mocking is left ENABLED after
    /// setup; call `enforce_real_auth` before the authorization matrix call
    /// under test.
    struct FeeManagerAuthFixture {
        env: Env,
        contract_id: Address,
        admin: Address,
    }

    fn setup_fee_manager_auth_fixture() -> FeeManagerAuthFixture {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin, &25);

        FeeManagerAuthFixture {
            env,
            contract_id,
            admin,
        }
    }

    /// Disables blanket auth mocking so only explicitly mocked authorizations pass.
    fn enforce_real_auth(env: &Env) {
        env.set_auths(&[]);
    }

    // -- initialize --------------------------------------------------------------

    #[test]
    fn test_fee_manager_auth_initialize_admin_authorized_succeeds() {
        let env = Env::default();
        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        let admin_invoke = MockAuthInvoke {
            contract: &contract_id,
            fn_name: "initialize",
            args: (&admin, &50u32).into_val(&env),
            sub_invokes: &[],
        };
        env.mock_auths(&[MockAuth {
            address: &admin,
            invoke: &admin_invoke,
        }]);

        client.initialize(&admin, &50);
        assert_eq!(client.get_fee_basis_points(), 50);
    }

    #[test]
    fn test_fee_manager_auth_initialize_missing_authorization_rejected() {
        let env = Env::default();
        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);

        assert!(
            client.try_initialize(&admin, &50u32).is_err(),
            "initialize must fail without admin authorization"
        );
        // The admin must not have been recorded.
        assert_eq!(
            client.try_set_fee_basis_points(&50).unwrap_err().unwrap(),
            Error::NotInitialized
        );
    }

    #[test]
    fn test_fee_manager_auth_initialize_unrelated_authorization_rejected() {
        let env = Env::default();
        let contract_id = env.register(FeeManagerContract, ());
        let client = FeeManagerContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let attacker = Address::generate(&env);

        // The attacker authorizes their own (attacker, initialize, ...) call;
        // it must not satisfy the admin's authorization.
        let attacker_invoke = MockAuthInvoke {
            contract: &contract_id,
            fn_name: "initialize",
            args: (&attacker, &50u32).into_val(&env),
            sub_invokes: &[],
        };
        env.mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &attacker_invoke,
        }]);

        assert!(
            client.try_initialize(&admin, &50u32).is_err(),
            "unrelated authorization must not satisfy admin authorization"
        );
    }

    // -- set_fee_basis_points ----------------------------------------------------

    #[test]
    fn test_fee_manager_auth_set_fee_admin_authorized_succeeds() {
        let fixture = setup_fee_manager_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = FeeManagerContractClient::new(&fixture.env, &fixture.contract_id);

        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "set_fee_basis_points",
            args: (&100u32,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.admin,
            invoke: &admin_invoke,
        }]);

        client.set_fee_basis_points(&100);
        assert_eq!(client.get_fee_basis_points(), 100);
    }

    #[test]
    fn test_fee_manager_auth_set_fee_non_admin_rejected() {
        let fixture = setup_fee_manager_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = FeeManagerContractClient::new(&fixture.env, &fixture.contract_id);

        // A non-admin authorizes the exact same invocation.
        let attacker = Address::generate(&fixture.env);
        let attacker_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "set_fee_basis_points",
            args: (&100u32,).into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &attacker_invoke,
        }]);

        assert!(
            client.try_set_fee_basis_points(&100).is_err(),
            "non-admin caller must not be able to set the fee"
        );
        assert_eq!(client.get_fee_basis_points(), 25);
    }

    #[test]
    fn test_fee_manager_auth_set_fee_missing_authorization_rejected() {
        let fixture = setup_fee_manager_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = FeeManagerContractClient::new(&fixture.env, &fixture.contract_id);

        assert!(
            client.try_set_fee_basis_points(&100).is_err(),
            "set_fee_basis_points must fail with no authorization provided"
        );
        assert_eq!(client.get_fee_basis_points(), 25);
    }

    #[test]
    fn test_fee_manager_auth_set_fee_unrelated_authorization_rejected() {
        let fixture = setup_fee_manager_auth_fixture();
        enforce_real_auth(&fixture.env);
        let client = FeeManagerContractClient::new(&fixture.env, &fixture.contract_id);

        // The admin authorizes an unrelated read-only function instead.
        let admin_invoke = MockAuthInvoke {
            contract: &fixture.contract_id,
            fn_name: "get_fee_basis_points",
            args: ().into_val(&fixture.env),
            sub_invokes: &[],
        };
        fixture.env.mock_auths(&[MockAuth {
            address: &fixture.admin,
            invoke: &admin_invoke,
        }]);

        assert!(
            client.try_set_fee_basis_points(&100).is_err(),
            "authorization for an unrelated function must be rejected"
        );
        assert_eq!(client.get_fee_basis_points(), 25);
    }
}
