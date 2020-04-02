#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Codec, Decode, Encode};
use sp_runtime::traits::{MaybeDisplay, MaybeFromStr};
use sp_std::vec::Vec;

use ls_biding_primitives::*;

sp_api::decl_runtime_apis! {
    pub trait LSBidingApi<AssetId, Balance, BlockNumber, AccountId> where
        AssetId: Codec,
        Balance: Codec,
        BlockNumber: Codec,
        AccountId: Codec,
    {
        fn get_borrows(size: Option<u64>, offset: Option<u64>) -> Option<Vec<Borrow<AssetId, Balance, BlockNumber, AccountId>>>;
        fn get_loans(size: Option<u64>, offset: Option<u64>) -> Option<Vec<Loan<AssetId, Balance, BlockNumber, AccountId>>>;
    }
}
