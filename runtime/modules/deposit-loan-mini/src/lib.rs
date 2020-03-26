#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
use codec::{Decode, Encode, Error as CodecErr, HasCompact, Input, Output};

#[allow(unused_imports)]
use sp_std::{
    self,
    collections::btree_map,
    convert::{TryFrom, TryInto},
    prelude::*,
    result,
    cmp,
    fmt::Debug,
    vec,
};

#[allow(unused_imports)]
use sp_runtime::{DispatchError, DispatchResult, RuntimeDebug};

#[allow(unused_imports)]
use sp_runtime::traits::{
    AtLeast32Bit, Bounded, CheckedAdd, CheckedMul, CheckedSub, MaybeDisplay, MaybeSerializeDeserialize, Member,
    One, Saturating, Zero,
};

#[allow(unused_imports)]
use support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{Parameter},
    ensure,
    weights::SimpleDispatchInfo,
};

#[allow(unused_imports)]
use frame_system::{self as system, ensure_root, ensure_signed};

pub trait Trait:
frame_system::Trait + sudo::Trait + timestamp::Trait + generic_asset::Trait
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}


decl_storage! {
    trait Store for Module<T: Trait> as Saving {

        /// module level switch
        Paused get(paused) : bool = false;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        // fn deposit_event() = default;

        #[weight = SimpleDispatchInfo::MaxNormal]
        pub fn pause(origin) -> DispatchResult {
            ensure_root(origin)?;
            Paused::mutate(|v| *v = true);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::MaxNormal]
        pub fn resume(origin) -> DispatchResult {
            ensure_root(origin)?;
            Paused::mutate(|v| *v = false);
            Ok(())
        }
    }
}

decl_event! {
    pub enum Event<T>
    where
    AccountId = <T as frame_system::Trait>::AccountId,
    {
        PausedRing(AccountId),
    }
}
