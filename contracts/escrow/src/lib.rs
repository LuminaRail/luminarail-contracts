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
mod test;

