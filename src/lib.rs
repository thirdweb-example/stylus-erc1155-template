//! this contract is not audited

#![cfg_attr(not(any(feature = "export-abi", test)), no_main)]
extern crate alloc;

// Modules and imports
mod erc1155;

use alloy_primitives::{U256, Address};
use stylus_sdk::{
    msg, prelude::*
};
use crate::erc1155::{Erc1155, Erc1155Params, Ownable};

struct StylusERC1155Params;
impl Erc1155Params for StylusERC1155Params {
    const NAME: &'static str = "MyStylusERC1155";
    const SYMBOL: &'static str = "SERC1155";

    fn uri(id: U256) -> String {
        format!("{}{}", "ipfs://base_uri/", id) // Update your NFT metadata base URI here
    }
}

sol_storage! {
    #[entrypoint]
    struct MyStylusERC1155 {
        #[borrow]
        Erc1155<StylusERC1155Params> erc1155;
        #[borrow]
        Ownable ownable;
    }
}

#[public]
#[inherit(Erc1155<StylusERC1155Params>, Ownable)]
impl MyStylusERC1155 {
    #[constructor]
    pub fn constructor(&mut self, owner: Address) {
        let _ = self.ownable._set_owner(owner);
    }

    pub fn mint(&mut self, to: Address, id: U256, amount: U256) -> Result<(), String> {
        self.erc1155.mint(to, id, amount)?;
        Ok(())
    }

    pub fn total_supply(&self, id:U256) -> Result<U256, String> {
        Ok(self.erc1155.total_supply.getter(id).get())
    }
}