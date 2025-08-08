use indexed_merkle_map::{
    Field, Hash, IndexedMerkleMap, InsertWitness, UpdateWitness,
};
use borsh::{BorshDeserialize, BorshSerialize};

pub struct AccountManager {
    pub map: IndexedMerkleMap,
}

impl AccountManager {
    pub fn new(height: usize) -> Self {
        Self {
            map: IndexedMerkleMap::new(height),
        }
    }

    pub fn get_root(&self) -> Hash {
        self.map.root()
    }

    pub fn process_action(&mut self, account_num: u32, add_value: u32) -> Result<AccountAction, String> {
        let key = Field::from_u32(account_num);
        
        // Check if account exists using get_option
        if let Some(existing_value) = self.map.get_option(&key) {
            // Update existing account - add to current balance
            let old_value_u32 = existing_value.to_u256().to_words()[0] as u32;
            let new_value_u32 = old_value_u32.wrapping_add(add_value);
            let new_field_value = Field::from_u32(new_value_u32);
            
            // Generate witness for update
            let witness = self.map.update_and_generate_witness(key, new_field_value, true)
                .map_err(|e| format!("Failed to update: {:?}", e))?
                .ok_or_else(|| "Failed to generate update witness".to_string())?;
            
            Ok(AccountAction::Update(witness))
        } else {
            // Insert new account with initial value
            let new_field_value = Field::from_u32(add_value);
            
            // Generate witness for insert
            let witness = self.map.insert_and_generate_witness(key, new_field_value, true)
                .map_err(|e| format!("Failed to insert: {:?}", e))?
                .ok_or_else(|| "Failed to generate insert witness".to_string())?;
            
            Ok(AccountAction::Insert(witness))
        }
    }
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub enum AccountAction {
    Insert(InsertWitness),
    Update(UpdateWitness),
}

impl AccountAction {
    pub fn old_root(&self) -> Hash {
        match self {
            AccountAction::Insert(w) => w.old_root,
            AccountAction::Update(w) => w.old_root,
        }
    }

    pub fn new_root(&self) -> Hash {
        match self {
            AccountAction::Insert(w) => w.new_root,
            AccountAction::Update(w) => w.new_root,
        }
    }
}

#[derive(Clone, Debug, BorshSerialize, BorshDeserialize)]
pub struct AccountOperation {
    pub account_num: u32,
    pub add_value: u32,
}

impl AccountOperation {
    pub fn new(account_num: u32, add_value: u32) -> Self {
        Self { account_num, add_value }
    }
}