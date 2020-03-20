#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use oracle::OracleMixedIn;
use rstd::prelude::*;
use rstd::vec;
use sp_runtime::traits::{Bounded, CheckedAdd, CheckedSub, EnsureOrigin, OnFinalize, Zero};
use support::traits::{
    ChangeMembers, Currency, Get, LockIdentifier, LockableCurrency, ReservableCurrency,
    WithdrawReasons,
};
use support::{
    decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap, StorageValue,
};
use system::{ensure_root, ensure_signed};

pub type Price = u128;

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    type OracleMixedIn: OracleMixedIn<Self>;
    type ReportOrigin: EnsureOrigin<Self::Origin, Success = Self::AccountId>;

    type OnChange: OnChange;
}

#[derive(PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct PriceReport<AccountId> {
    reporter: AccountId,
    price: Price,
}

pub const PRICE_PRECISION: u32 = 10000;

pub trait OnChange {
    fn on_change(price: Price) {}
}
impl OnChange for () {}

decl_storage! {
    trait Store for Module<T: Trait> as PriceStorate {
        CurrentPrice get(current_price): Price;
        PriceReports get(price_reports): Vec<PriceReport<T::AccountId>>;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        pub fn report(origin, price: Price) -> DispatchResult {
            let who = T::ReportOrigin::ensure_origin(origin)?;
            Self::add_price(who, price);
            Ok(())
        }

        fn on_finalize() {
            let block_number = <system::Module<T>>::block_number();
            let old_price = Self::current_price();
            let mut prices: Vec<Price> = Self::price_reports().iter().map(|x| x.price).collect();
            if prices.len() > 0 {
                let median_price = median(&mut prices);

                if old_price != median_price {
                    CurrentPrice::put(median_price);
                    T::OnChange::on_change(median_price);
                    Self::deposit_event(RawEvent::PriceChanged(median_price));
                }
            }

            let reports: Vec<PriceReport<T::AccountId>> = Self::price_reports()
                .into_iter()
                .filter(|x| T::OracleMixedIn::is_valid(&x.reporter))
                .clone()
                .collect();

            <PriceReports<T>>::put(reports);
        }
    }
}

impl<T: Trait> Module<T> {
    fn add_price(who: T::AccountId, price: Price) -> DispatchResult {
        let price_reports = Self::price_reports();
        let mut found = false;
        let mut price_reports: Vec<PriceReport<T::AccountId>> = price_reports
            .into_iter()
            .map(|x| {
                if x.reporter == who {
                    let mut new_report = x;
                    new_report.price = price;
                    found = true;
                    new_report
                } else {
                    x
                }
            })
            .collect();

        if !found {
            price_reports.push(PriceReport {
                reporter: who.clone(),
                price: price,
            });
        }

        <PriceReports<T>>::put(price_reports);

        T::OracleMixedIn::on_witnessed(&who);
        Self::deposit_event(RawEvent::PriceReported(who, price));
        Ok(())
    }
}

fn mean(numbers: &Vec<Price>) -> Price {
    let sum: Price = numbers.iter().sum();
    sum as Price / numbers.len() as Price
}

fn median(numbers: &mut Vec<Price>) -> Price {
    numbers.sort();

    if numbers.len() == 1 {
        return numbers[0];
    }

    let mid = numbers.len() / 2;
    if numbers.len() % 2 == 0 {
        mean(&vec![numbers[mid - 1], numbers[mid]]) as Price
    } else {
        numbers[mid]
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
    {
        PriceReported(AccountId, Price),
        PriceChanged(Price),
    }
);
