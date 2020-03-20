#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
use codec::{Decode, Encode, Error as codecErr, HasCompact, Input, Output};
use sp_core::H256;
use sp_std::marker::PhantomData;
use sp_std::prelude::*;
use support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, ensure,
    weights::SimpleDispatchInfo,
};
use system::{ensure_root, ensure_signed};

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use hkd32;

mod mock;
mod tests;

pub type TxHash = H256;

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as BTCBridge {
        pub KeyMaterial get(key_material) config() : [u8; 32];
        pub SubKeyMap get(sub_key_map) :
      double_map hasher(twox_128) T::AccountId, hasher(blake2_128) Vec<u8> => H256;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        #[weight = SimpleDispatchInfo::MaxOperational]
        pub fn set_key_material(origin, material: [u8; 32]) -> DispatchResult {
            ensure_root(origin)?;
            KeyMaterial::put(material);
            Ok(())
        }

        pub fn get_subkey_from_path(origin, path_input: Vec<u8>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(KeyMaterial::exists(), <Error<T>>::MissingKeyMaterial.as_str());
            ensure!(hkd32::DELIMITER.is_ascii(), <Error<T>>::UnsupporttedPathDelimiter.as_str());
            if <SubKeyMap<T>>::exists(&who, &path_input) {
                return Ok(());
            } else {
                let key_material = hkd32::KeyMaterial::new(KeyMaterial::get());
                let mut delimiter: [u8; 1] = [0x00];
                // ensure ascii, can't panic
                hkd32::DELIMITER.encode_utf8(&mut delimiter);
                let mut components = path_input.split(|x| -> bool { x == &delimiter[0] });
                let mut path_buf = hkd32::PathBuf::new();
                components.next();
                components.for_each(|x| {
                    path_buf.push(hkd32::Component::new(x).unwrap());
                });
                let derived_key = key_material.derive_subkey(path_buf);
                let derived_hash = H256::from_slice(derived_key.as_bytes());
                <SubKeyMap<T>>::insert(who, path_input, derived_hash);
                return Ok(());
                // match hkd32::PathBuf::from_bytes(&path_input) {
                //     Ok(path) => {
                //         let derived_key = key_material.derive_subkey(path);
                //         let derived_hash = H256::from_slice(derived_key.as_bytes());
                //         <SubKeyMap<T>>::insert(who, path_input, derived_hash);
                //         return Ok(());
                //     },
                //     Err(_) => {
                //         return Err(<Error<T>>::InvalidPathInput.into());
                //     }
                // }
            }
        }
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        MissingKeyMaterial,
        InvalidPathInput,
        UnsupporttedPathDelimiter,
    }
}

decl_event!(
    #[rustfmt::skip]
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
    {
        PhatomEvent(AccountId),
    }
);

impl<T: Trait> Module<T> {}
