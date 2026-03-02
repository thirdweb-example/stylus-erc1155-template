//! this contract is not audited

use alloc::{string::String, vec::Vec};
use alloy_primitives::{Address, U256};
use alloy_sol_types::sol;
use core::marker::PhantomData;
use stylus_sdk::{
    abi::Bytes,
    prelude::*
};

pub trait Erc1155Params {
    const NAME: &'static str;
    const SYMBOL: &'static str;
    fn uri(id: U256) -> String;
}

sol_storage! {
    pub struct Erc1155<T> {
        mapping(uint256 => mapping(address => uint256)) balances;
        mapping(address => mapping(address => bool)) operator_approvals;
        mapping(uint256 => uint256) total_supply;
        PhantomData<T> phantom;
    }

    pub struct Ownable {
        address owner;
    }
}

sol! {
    event TransferSingle(address indexed operator, address indexed from,
        address indexed to, uint256 id, uint256 value);
    event TransferBatch(address indexed operator, address indexed from,
        address indexed to, uint256[] ids, uint256[] values);
    event ApprovalForAll(address indexed owner, address indexed operator,
            bool approved);
    event URI(string value, uint256 indexed id);
}

sol_interface! {
    interface IERC1155Receiver {
        function onERC1155Received(address,address,uint256,uint256,bytes) external view returns(bytes4);
        function onERC1155BatchReceived(address,address,uint256[],uint256[],bytes) external view returns(bytes4);
    }
}

const RECEIVER_SINGLE: u32 = 0xf23a6e61;
const RECEIVER_BATCH: u32 = 0xbc197c81;

#[public]
impl Ownable {
    pub fn owner(&self) -> Result<Address, String> {
        Ok(self.owner.get())
    }

    pub fn set_owner(&mut self, new_owner: Address) -> Result<(), String> {
        self._check_owner()?;
        self._set_owner(new_owner)?;

        Ok(())
    }
}

impl Ownable {
    pub fn _check_owner(&self) -> Result<(), String> {
        let msg_sender = self.vm().msg_sender();
        let owner = self.owner.get();

        if msg_sender != owner {
            return Err("Not authorized".into());
        }

        Ok(())
    }

    pub fn _set_owner(&mut self, new_owner: Address) -> Result<(), String> {
        if new_owner != Address::ZERO {
            return Err("Zero address".into());
        }

        self.owner.set(new_owner);
        
        Ok(())
    }
}

impl<T: Erc1155Params> Erc1155<T> {
    #[inline(always)]
    fn _is_approved_or_owner(&self, owner: Address) -> bool {
        owner == self.vm().msg_sender()
            || self.operator_approvals.getter(owner).get(self.vm().msg_sender())
    }

    fn require_authorized_to_spend(&self, owner: Address) -> Result<(), String> {
        if self.vm().msg_sender() == owner {
            return Ok(());
        }

        if self.operator_approvals.getter(owner).get(self.vm().msg_sender()) {
            return Ok(());
        }

        return Err("Not approved".into());
    }

    fn _update_balance(
        &mut self,
        from: Address,
        to: Address,
        id: U256,
        value: U256,
    ) -> Result<(), String> {
        let mut fb = self.balances.setter(id);
        // subtract
        if !from.is_zero() {
            let bal = fb.getter(from).get() - value;
            fb.setter(from).set(bal);
        }
        // add
        if !to.is_zero() {
            let bal = fb.getter(to).get() + value;
            fb.setter(to).set(bal);
        }
        Ok(())
    }

    fn _call_receiver_single(
        &self,
        from:   Address,
        to:     Address,
        id:     U256,
        amount: U256,
        data:   Vec<u8>,
    ) -> Result<(), String> {
        if self.vm().code_size(to) > 0 {
            let sender = self.vm().msg_sender();
            let receiver = IERC1155Receiver::new(to);
            let received = receiver
                .on_erc_1155_received(self.vm(), Call::new(), sender, from, id, amount, data.into())
                .map_err(|_| "ERC1155Receiver: low-level call failed")?
                .0;
    
            if u32::from_be_bytes(received) != RECEIVER_SINGLE {
                return Err("Receiver refused".into());
            }
        }

        Ok(())
    }

    #[inline(never)]
    fn _call_receiver_batch(
        &self,
        from:   Address,
        to:     Address,
        ids:     Vec<U256>,
        amounts: Vec<U256>,
        data:   Vec<u8>,
    ) -> Result<(), String> {
        if self.vm().code_size(to) > 0 {
            let sender = self.vm().msg_sender();
            let receiver = IERC1155Receiver::new(to);
            let received = receiver
                .on_erc_1155_batch_received(self.vm(), Call::new(), sender, from, ids, amounts, data.into())
                .map_err(|_| "ERC1155Receiver: low-level call failed")?
                .0;

            if u32::from_be_bytes(received) != RECEIVER_BATCH {
                return Err("Receiver refused".into());
            }
        }

        Ok(())
    }

    pub fn mint(
        &mut self,
        to:     Address,
        id:     U256,
        amount: U256
    ) -> Result<(), String> {
        self._update_balance(Address::ZERO, to, id, amount)?;
    
        let ts = self.total_supply.getter(id).get() + amount;
        self.total_supply.setter(id).set(ts);
    
        self.vm().log(TransferSingle {
            operator: self.vm().msg_sender(),
            from:     Address::ZERO,
            to,
            id,
            value:    amount,
        });
    
        Ok(())
    }
}

#[public]
impl<T: Erc1155Params> Erc1155<T> {
    pub fn name() -> Result<String, String> {
        Ok(T::NAME.into())
    }

    pub fn symbol() -> Result<String, String> {
        Ok(T::SYMBOL.into())
    }

    pub fn uri(&self, id: U256) -> Result<String, String> {
        Ok(T::uri(id))
    }

    pub fn safe_transfer_from(
        &mut self,
        from: Address,
        to: Address,
        id: U256,
        value: U256,
        data: Vec<u8>,
    ) -> Result<(), String> {
        self.require_authorized_to_spend(from);
        self._update_balance(from, to, id, value)?;
        let sender = self.vm().msg_sender();
        self.vm().log(TransferSingle {
            operator: sender, from, to, id, value
        });
        self._call_receiver_single(from, to, id, value, data)?;
        Ok(())
    }

    pub fn safe_batch_transfer_from(
        &mut self,
        from:    Address,
        to:      Address,
        ids:     Vec<U256>,
        amounts: Vec<U256>,
        data:    Bytes,
    ) -> Result<(), String> {
        if ids.len() != amounts.len() {
            return Err("length mismatch".into());
        }
        self._is_approved_or_owner(from);
    
        for (i, id) in ids.iter().enumerate() {
            self._update_balance(from, to, *id, amounts[i])?;
        }

        let sender = self.vm().msg_sender();
        self.vm().log(TransferBatch {
            operator: sender,
            from,
            to,
            ids:     ids.clone(),
            values:  amounts.clone(),
        });
    
        self._call_receiver_batch(from, to, ids, amounts, data.to_vec())?;
        Ok(())
    }

    pub fn balance_of(&self, owner: Address, id: U256) -> Result<U256, String> {
        let bal = self
            .balances
            .getter(id)
            .getter(owner)
            .get();
    
        Ok(bal)
    }

    pub fn balance_of_batch(
        &self,
        owners: Vec<Address>,
        ids:    Vec<U256>,
    ) -> Result<Vec<U256>, String> {
        if owners.len() != ids.len() {
            return Err("length mismatch".into());
        }
        let mut out = Vec::with_capacity(ids.len());
        for (i, id) in ids.iter().enumerate() {
            let bal = self
                .balances
                .getter(*id)
                .getter(owners[i])
                .get();
            out.push(bal);
        }
        Ok(out)
    }

    pub fn set_approval_for_all(&mut self, operator: Address, approved: bool) -> Result<(), String> {
        let owner = self.vm().msg_sender();
        self.operator_approvals
            .setter(owner)
            .insert(operator, approved);

        self.vm().log(ApprovalForAll {
            owner,
            operator,
            approved,
        });
        Ok(())
    }

    pub fn is_approved_for_all(&self, owner: Address, operator: Address) -> Result<bool, String> {
        Ok(self.operator_approvals.getter(owner).get(operator))
    }

    // TODO: supports interface
}