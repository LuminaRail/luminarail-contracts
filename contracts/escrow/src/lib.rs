#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

#[contract]
pub struct EscrowContract;

#[contractimpl]
impl EscrowContract {
    pub fn create_escrow(_env: Env) -> Symbol {
        symbol_short!("created")
    }

    pub fn release_escrow(_env: Env) -> Symbol {
        symbol_short!("released")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_escrow_lifecycle() {
        let env = Env::default();
        let contract_id = env.register(EscrowContract, ());
        let client = EscrowContractClient::new(&env, &contract_id);

        assert_eq!(client.create_escrow(), symbol_short!("created"));
        assert_eq!(client.release_escrow(), symbol_short!("released"));
    }
}
