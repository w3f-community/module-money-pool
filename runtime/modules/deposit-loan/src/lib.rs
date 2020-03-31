// use new_oracle to get btc price
// Notice：the btc price used here is consered as two assets exchange ratio.
// let current_price = <new_oracle::Module<T>>::current_price(&token);
// let price: u64 = TryInto::<u64>::try_into(current_price).unwrap_or(0);

#![cfg_attr(not(feature = "std"), no_std)]

#[allow(unused_imports)]
use codec::{Decode, Encode, Error as CodecErr, HasCompact, Input, Output};

#[allow(unused_imports)]
use sp_std::{
    self, cmp,
    collections::btree_map,
    convert::{TryFrom, TryInto},
    fmt::Debug,
    prelude::*,
    result, vec,
};

#[allow(unused_imports)]
use sp_runtime::traits::{
    AtLeast32Bit, Bounded, CheckedAdd, CheckedMul, CheckedSub, MaybeDisplay,
    MaybeSerializeDeserialize, Member, One, Saturating, Zero,
};

#[allow(unused_imports)]
use sp_runtime::{DispatchError, DispatchResult, RuntimeDebug};

#[allow(unused_imports)]
use support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::Parameter, ensure,
    weights::SimpleDispatchInfo,
};

#[allow(unused_imports)]
use frame_system::{self as system, ensure_root, ensure_signed};

mod mock;
mod tests;

const SEC_PER_DAY: u32 = 86400;
const DAYS_PER_YEAR: u32 = 365;
pub const INTEREST_RATE_PREC: u32 = 10000_0000;
pub const LTV_PREC: u32 = 10000;
pub const PRICE_PREC: u32 = 10000;

pub type PriceInUSDT = u64;
pub type LoanId = u64;
// pub type CreditLineId = u64;
pub type LTV = u64;
pub type LoanResult<T = ()> = result::Result<T, DispatchError>;

#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum LoanHealth {
    Well,
    Warning(LTV),
    Liquidating(LTV),
}
impl Default for LoanHealth {
    fn default() -> Self {
        Self::Well
    }
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug)]
pub struct CollateralLoan<Balance> {
    pub collateral_amount: Balance,
    pub loan_amount: Balance,
}

#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug)]
pub struct Loan<AccountId, Balance> {
    pub id: LoanId,
    pub who: AccountId,
    pub collateral_balance_original: Balance,
    pub collateral_balance_available: Balance,
    pub loan_balance_total: Balance,
    pub status: LoanHealth,
}

impl<AccountId, Balance> Loan<AccountId, Balance>
where
    Balance: Encode
        + Decode
        + Parameter
        + Member
        + AtLeast32Bit
        + Default
        + Copy
        + MaybeSerializeDeserialize
        + Debug,
    //  Moment: Parameter + Default + SimpleArithmetic + Copy,
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord + Default,
{
    pub fn get_ltv(
        collateral_amount: Balance,
        loan_amount: Balance,
        btc_price: PriceInUSDT,
    ) -> LTV {
        let btc_price_in_balance = <Balance as TryFrom<u128>>::try_from(btc_price as u128)
            .ok()
            .unwrap();
        let ltv = (loan_amount * Balance::from(PRICE_PREC) * Balance::from(LTV_PREC))
            / (collateral_amount * btc_price_in_balance);
        TryInto::<LTV>::try_into(ltv).ok().unwrap()
    }
}

pub trait Trait:
    frame_system::Trait + timestamp::Trait + generic_asset::Trait + new_oracle::Trait
{
    type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
}

decl_storage! {
    trait Store for Module<T: Trait> as Saving {

        /// module level switch
        Paused get(paused) : bool = false;

        /// the asset that user saves into our program
        CollectionAssetId get(collection_asset_id) config() : T::AssetId;

        /// the account where user saves go and it can be either a normal account which held by or a totally random account
        /// probably need to be supervised by the public
        CollectionAccountId get(collection_account_id) build(|config: &GenesisConfig<T>| {
            config.collection_account_id.clone()
        }) : T::AccountId;

        /// User will get dtoken when make saving
        /// This will be used to calculate the amount when redeem.
        pub UserDtoken get(user_dtoken) : linked_map hasher(blake2_256) T::AccountId => T::Balance;

        // Total market dtoken generated
        pub MarketDtoken get(market_dtoken) config(): T::Balance;

        // Total dtoken amount
        pub TotalDtoken get(total_dtoken) config(): T::Balance;

        /// time of last distribution of interest
        BonusTime get(bonus_time) : T::Moment;

        /// Annualized interest rate of loan
        pub LoanInterestRateCurrent get(loan_interest_rate_current) config(): T::Balance;

        /// use "ProfitAsset" for bonus
        ProfitAssetId get(profit_asset_id) config() : T::AssetId;

        /// use a specific account as "ProfitPool"
        /// might be supervised by the public
        ProfitPool get(profit_pool) config() : T::AccountId;

        /// the account that user makes loans from, (and assets are all burnt from this account by design)
        PawnShop get(pawn_shop) config() : T::AccountId;

        /// the asset that user uses as collateral when making loans
        CollateralAssetId get(collateral_asset_id) config() : T::AssetId;

        /// the asset that defi
        LoanAssetId get(loan_asset_id) config() : T::AssetId;

        /// the maximum LTV that a loan package can be set initially
        pub GlobalLTVLimit get(global_ltv_limit) config() : LTV;

        /// when a loan's LTV reaches or is above this threshold, this loan must be been liquidating
        pub GlobalLiquidationThreshold get(global_liquidation_threshold) config() : LTV;

        /// when a loan's LTV reaches or is above this threshold, a warning event will be fired and there should be a centralized system monitoring on this
        pub GlobalWarningThreshold get(global_warning_threshold) config() : LTV;

        /// increase monotonically
        NextLoanId get(next_loan_id) config() : LoanId;

        /// currently running loans
        pub Loans get(get_loan_by_id) : linked_map hasher(blake2_256) LoanId => Loan<T::AccountId, T::Balance>;

        /// loan id aggregated by account
        pub LoansByAccount get(loans_by_account) : map hasher(blake2_256) T::AccountId => Vec<LoanId>;

        /// total balance of loan asset in circulation
        pub TotalLoan get(total_loan) : T::Balance;

        /// total balance of collateral asset locked in the pawnshop
        pub TotalCollateral get(total_collateral) : T::Balance;

        /// when a loan is overdue, a small portion of its collateral will be cut as penalty
        pub PenaltyRate get(penalty_rate) config() : u32;

        /// the official account take charge of selling the collateral asset of liquidating loans
        LiquidationAccount get(liquidation_account) config() : T::AccountId;

        /// loans which are in liquidating, these loans will not be in "Loans" & "LoansByAccount"
        pub LiquidatingLoans get(liquidating_loans) : Vec<LoanId>;

        /// a global cap of loan balance, no caps at all if None
        pub LoanCap get(loan_cap) : Option<T::Balance>;

        /// for each loan, the amount of collateral asset must be greater than this
        pub MinimumCollateral get(minimum_collateral) config() : T::Balance;

        pub LiquidationPenalty get(liquidation_penalty) config() : u32;

        pub SavingInterestRate get(saving_interest_rate) config() : T::Balance;
    }

    add_extra_genesis {
        config(collection_account_id): T::AccountId;
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        fn on_initialize(height: T::BlockNumber) {
            if !Self::paused() {
                Self::on_each_block(height);
                Self::calculate_loan_interest_rate();
            }
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn pause(origin) -> DispatchResult {
            ensure_root(origin)?;
            Paused::mutate(|v| *v = true);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn resume(origin) -> DispatchResult {
            ensure_root(origin)?;
            Paused::mutate(|v| *v = false);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_collection_asset_id(origin, asset_id: T::AssetId) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(<generic_asset::Module<T>>::asset_id_exists(asset_id), "invalid collection asset id");
            <CollectionAssetId<T>>::put(asset_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_collection_account(origin, account_id: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            <CollectionAccountId<T>>::put(account_id.clone());
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_collateral_asset_id(origin, asset_id: T::AssetId) -> LoanResult {
            ensure_root(origin)?;
            <CollateralAssetId<T>>::put(asset_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_global_ltv_limit(origin, limit: LTV) -> LoanResult {
            ensure_root(origin)?;
            GlobalLTVLimit::put(limit);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_loan_asset_id(origin, asset_id: T::AssetId) -> LoanResult {
            ensure_root(origin)?;
            <LoanAssetId<T>>::put(asset_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_global_liquidation_threshold(origin, threshold: LTV) -> LoanResult {
            ensure_root(origin)?;
            GlobalWarningThreshold::put(threshold);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_global_warning_threshold(origin, threshold: LTV) -> LoanResult {
            ensure_root(origin)?;
            GlobalLiquidationThreshold::put(threshold);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_loan_cap(origin, balance: T::Balance) -> LoanResult {
            ensure_root(origin)?;
            if balance.is_zero() {
                <LoanCap<T>>::kill();
            } else {
                <LoanCap<T>>::put(balance);
            }
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_liquidation_account(origin, account_id: T::AccountId) -> LoanResult {
            ensure_root(origin)?;
            <LiquidationAccount<T>>::put(account_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_profit_asset_id(origin, asset_id: T::AssetId) -> DispatchResult {
            ensure_root(origin)?;
            ensure!(<generic_asset::Module<T>>::asset_id_exists(asset_id), "invalid collection asset id");
            <ProfitAssetId<T>>::put(asset_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_profit_pool(origin, account_id: T::AccountId) -> DispatchResult {
            ensure_root(origin)?;
            <ProfitPool<T>>::put(account_id);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn set_penalty_rate(origin, rate: u32) -> LoanResult {
            ensure_root(origin)?;
            PenaltyRate::put(rate);
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn staking(origin, asset_id: T::AssetId, amount: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            ensure!(<CollectionAssetId<T>>::get() == asset_id, "can't collect this asset");
            ensure!(<generic_asset::Module<T>>::free_balance(&asset_id, &who) >= amount, "insufficient balance");
            Self::create_staking(who.clone(), asset_id, amount)?;
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn sudo_staking(origin, asset_id: T::AssetId, amount: T::Balance, delegatee: T::AccountId) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            ensure_root(origin)?;
            ensure!(<CollectionAssetId<T>>::get() == asset_id, "can't collect this asset");
            ensure!(<generic_asset::Module<T>>::free_balance(&asset_id, &delegatee) >= amount, "insufficient balance");
            Self::create_staking(delegatee.clone(), asset_id, amount)?;
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn redeem(origin, iou_asset_id: T::AssetId, iou_asset_amount: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            let collection_asset_id = Self::collection_asset_id();
            let collection_account_id = Self::collection_account_id();
            // ensure!(!collection_asset_id.is_zero(), "fail to find collection asset id");
            ensure!(<generic_asset::Module<T>>::free_balance(&collection_asset_id, &collection_account_id) >= iou_asset_amount, "Not enough to redeem");
            ensure!(collection_asset_id == iou_asset_id, "collection asset id different from iou asset id");

            Self::make_redeem(
                &who,
                &collection_asset_id,
                &collection_account_id,
                iou_asset_amount,
            )?;
            Ok(())
        }

        #[weight = SimpleDispatchInfo::FixedNormal(0)]
        pub fn sudo_redeem(origin, iou_asset_id: T::AssetId, iou_asset_amount: T::Balance, delegatee: T::AccountId) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            ensure_root(origin)?;
            let collection_asset_id = Self::collection_asset_id();
            let collection_account_id = Self::collection_account_id();
            // ensure!(!collection_asset_id.is_zero(), "fail to find collection asset id");
            ensure!(<generic_asset::Module<T>>::free_balance(&collection_asset_id, &collection_account_id) >= iou_asset_amount, "Not enough to redeem");
            ensure!(collection_asset_id == iou_asset_id, "collection asset id different from iou asset id");

            Self::make_redeem(
                &delegatee,
                &collection_asset_id,
                &collection_account_id,
                iou_asset_amount,
            )?;
            Ok(())
        }

        /// a user can apply for a loan choosing one active loan package, providing the collateral and loan amount he wants,
        #[weight = SimpleDispatchInfo::FixedNormal(10)]
        pub fn apply_loan(origin, collateral_amount: T::Balance, loan_amount: T::Balance) -> LoanResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            Self::apply_for_loan(who.clone(), collateral_amount, loan_amount)
        }

        /// a user repay a loan he has made before, by providing the loan id and he should make sure there is enough related assets in his account
        #[weight = SimpleDispatchInfo::FixedNormal(10)]
        pub fn repay_loan(origin, loan_id: LoanId) -> LoanResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            Self::repay_for_loan(who.clone(), loan_id)
        }

        /// when a liquidating loan has been handled well, platform mananger should call "mark_liquidated" to update the chain
        /// loan id is the loan been handled and auction_balance is what the liquidation got by selling the collateral asset
        /// auction_balance will be first used to make up the loan, then what so ever left will be returned to the loan's owner account
        #[weight = SimpleDispatchInfo::FixedNormal(10)]
        pub fn mark_liquidated(origin, loan_id: LoanId, auction_balance: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let liquidation_account = ensure_signed(origin)?;
            ensure!(liquidation_account == Self::liquidation_account(), "liquidation account only");
            ensure!(<Loans<T>>::contains_key(loan_id), "loan doesn't exists");

            Self::mark_loan_liquidated(&Self::get_loan_by_id(loan_id), liquidation_account, auction_balance)
        }

        /// when user got a warning of high-risk LTV, user can lower the LTV by add more collateral
        #[weight = SimpleDispatchInfo::FixedNormal(10)]
        pub fn add_collateral(origin, loan_id: LoanId, amount: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            ensure!(<Loans<T>>::contains_key(loan_id), "loan doesn't exists");
            let loan = Self::get_loan_by_id(loan_id);
            ensure!(who == loan.who, "adding collateral to other's loan is not allowed");

            Self::add_loan_collateral(&loan, loan.who.clone(), amount)
        }

        /// as long as the LTV of this loan is below the "GlobalLTVLimit", user can keep drawing TBD from this loan
        #[weight = SimpleDispatchInfo::FixedNormal(10)]
        pub fn draw(origin, loan_id: LoanId, amount: T::Balance) -> DispatchResult {
            ensure!(!Self::paused(), "module is paused");
            let who = ensure_signed(origin)?;
            Self::draw_from_loan(who, loan_id, amount)
        }
    }
}

impl<T: Trait> Module<T> {
    pub fn create_staking(
        who: T::AccountId,
        asset_id: T::AssetId,
        balance: T::Balance,
    ) -> DispatchResult {
        ensure!(!balance.is_zero(), "saving can't be zero");

        let market_dtoken_amount = Self::market_dtoken();
        let total_dtoken_amount = Self::total_dtoken();
        let collection_account_id = Self::collection_account_id();

        let mut user_dtoken = T::Balance::from(0);

        <generic_asset::Module<T>>::make_transfer_with_event(
            &asset_id,
            &who,
            &collection_account_id,
            balance,
        )?;

        let ltv_prec_in_balance = T::Balance::from(LTV_PREC);
        if total_dtoken_amount.is_zero() {
            user_dtoken = balance;
        } else {
            user_dtoken = balance
                .checked_mul(&market_dtoken_amount)
                .expect("overflow!")
                / total_dtoken_amount;
        }

        if <UserDtoken<T>>::contains_key(who.clone()) {
            <UserDtoken<T>>::mutate(who.clone(), |v| {
                *v = v.checked_add(&user_dtoken).expect("overflow");
            });
        } else {
            <UserDtoken<T>>::insert(&who, user_dtoken);
        }

        let market_dtoken = market_dtoken_amount.checked_add(&user_dtoken).unwrap();
        let total_dtoken = market_dtoken_amount.checked_add(&balance).unwrap();

        <MarketDtoken<T>>::put(market_dtoken);
        <TotalDtoken<T>>::put(total_dtoken);

        Ok(())
    }

    fn make_redeem(
        who: &T::AccountId,
        collection_asset_id: &T::AssetId,
        collection_account_id: &T::AccountId,
        amount: T::Balance,
    ) -> DispatchResult {
        let market_dtoken_amount = Self::market_dtoken();
        let total_dtoken_amount = Self::total_dtoken();

        let user_dtoken_amount = Self::user_dtoken(&who);
        let user_will_get = user_dtoken_amount / (market_dtoken_amount / total_dtoken_amount);

        ensure!(user_will_get >= amount, "redeem too much assets!");
        Self::make_redeem_all(&who).unwrap_or_default();
        Self::create_staking(who.clone(), *collection_asset_id, user_will_get - amount)
            .unwrap_or_default();
        Ok(())
    }

    fn make_redeem_all(who: &T::AccountId) -> DispatchResult {
        let market_dtoken_amount = Self::market_dtoken();
        let total_dtoken_amount = Self::total_dtoken();
        let collection_asset_id = Self::collection_asset_id();
        let collection_account_id = Self::collection_account_id();

        let user_dtoken_amount = Self::user_dtoken(&who);

        let user_will_get = user_dtoken_amount / (market_dtoken_amount / total_dtoken_amount);

        ensure!(
            <generic_asset::Module<T>>::free_balance(&collection_asset_id, &collection_account_id)
                >= user_will_get,
            "saving balance is short"
        );

        ensure!(
            Self::total_dtoken() >= user_dtoken_amount,
            "total dtoken is short"
        );
        ensure!(
            Self::market_dtoken() >= user_dtoken_amount,
            "market dtoken is short"
        );

        let total_dtoken = Self::total_dtoken() - user_will_get;
        let market_dtoken = Self::market_dtoken() - user_dtoken_amount;

        <MarketDtoken<T>>::put(market_dtoken);
        <TotalDtoken<T>>::put(total_dtoken);

        <generic_asset::Module<T>>::make_transfer_with_event(
            &collection_asset_id,
            &collection_account_id,
            &who,
            user_will_get,
        )?;
        Ok(())
    }

    fn apply_for_loan(
        who: T::AccountId,
        collateral_amount: T::Balance,
        loan_amount: T::Balance,
    ) -> DispatchResult {
        let collection_asset_id = Self::collection_asset_id();
        let collection_account_id = Self::collection_account_id();
        ensure!(
            <generic_asset::Module<T>>::free_balance(&collection_asset_id, &collection_account_id)
                >= loan_amount,
            "Not enough to loan"
        );

        let collateral_asset_id = Self::collateral_asset_id();
        let token = <generic_asset::Module<T>>::symbols(collateral_asset_id);
        let current_price = <new_oracle::Module<T>>::current_price(&token);
        let btc_price: u64 = TryInto::<u64>::try_into(current_price).unwrap_or(0);

        let loan_asset_id = Self::loan_asset_id();
        let collateral_asset_id = Self::collateral_asset_id();

        let shop = <PawnShop<T>>::get();
        let loan_cap = <LoanCap<T>>::get();
        let total_loan = <TotalLoan<T>>::get();

        if loan_cap.is_some() && total_loan >= loan_cap.unwrap() {
            return Err(Error::<T>::ReachLoanCap)?;
        }

        match Self::get_collateral_loan(collateral_amount, loan_amount) {
            Err(err) => Err(err),
            Ok(CollateralLoan {
                collateral_amount: actual_collateral_amount,
                loan_amount: actual_loan_amount,
            }) => {
                ensure!(
                    collateral_amount >= Self::minimum_collateral(),
                    "not reach min collateral amount"
                );

                // transfer collateral to pawnshop
                <generic_asset::Module<T>>::make_transfer_with_event(
                    &collateral_asset_id,
                    &who,
                    &shop,
                    actual_collateral_amount,
                )?;

                let loan_id = Self::get_next_loan_id();

                let collateral_balance_available = actual_collateral_amount
                    - loan_amount
                        / <T::Balance as TryFrom<u128>>::try_from(btc_price as u128)
                            .ok()
                            .unwrap();

                let loan = Loan {
                    id: loan_id,
                    who: who.clone(),
                    collateral_balance_original: actual_collateral_amount,
                    collateral_balance_available: collateral_balance_available,
                    loan_balance_total: actual_loan_amount,
                    status: Default::default(),
                };

                <generic_asset::Module<T>>::make_transfer_with_event(
                    &collection_asset_id,
                    &collection_account_id,
                    &who,
                    loan_amount,
                )?;

                <Loans<T>>::insert(loan_id, loan.clone());
                <LoansByAccount<T>>::mutate(&who, |v| {
                    v.push(loan_id);
                });
                <TotalLoan<T>>::mutate(|v| *v += actual_loan_amount);
                <TotalCollateral<T>>::mutate(|v| *v += actual_collateral_amount);

                Self::deposit_event(RawEvent::LoanCreated(loan));
                Ok(())
            }
        }
    }

    pub fn get_collateral_loan(
        collateral_amount: T::Balance,
        loan_amount: T::Balance,
    ) -> Result<CollateralLoan<T::Balance>, DispatchError> {
        if collateral_amount.is_zero() && loan_amount.is_zero() {
            return Err(Error::<T>::InvalidCollateralLoanAmounts)?;
        }
        let collateral_asset_id = Self::collateral_asset_id();

        // get current btc price
        let token = <generic_asset::Module<T>>::symbols(collateral_asset_id);
        let current_price = <new_oracle::Module<T>>::current_price(&token);
        let btc_price: u64 = TryInto::<u64>::try_into(current_price).unwrap_or(0);
        let btc_price_in_balance = <T::Balance as TryFrom<u128>>::try_from(btc_price as u128)
            .ok()
            .unwrap();

        let price_prec_in_balance = T::Balance::from(PRICE_PREC);
        let ltv_prec_in_balance = T::Balance::from(LTV_PREC);

        let ltv = GlobalLTVLimit::get();
        let ltv_in_balance = <T::Balance as TryFrom<u64>>::try_from(ltv).ok().unwrap();

        if collateral_amount.is_zero() {
            let must_collateral_amount = loan_amount * ltv_prec_in_balance * price_prec_in_balance
                / (btc_price_in_balance * ltv_in_balance);
            return Ok(CollateralLoan {
                collateral_amount: must_collateral_amount,
                loan_amount: loan_amount,
            });
        }

        if loan_amount.is_zero() {
            let can_loan_amount = (collateral_amount * btc_price_in_balance * ltv_in_balance)
                / (ltv_prec_in_balance * price_prec_in_balance);
            return Ok(CollateralLoan {
                collateral_amount: collateral_amount,
                loan_amount: can_loan_amount,
            });
        }

        if (loan_amount * ltv_prec_in_balance) * price_prec_in_balance
            / (collateral_amount * btc_price_in_balance)
            >= ltv_in_balance
        {
            Err(Error::<T>::OverLTVLimit)?
        } else {
            Ok(CollateralLoan {
                collateral_amount,
                loan_amount,
            })
        }
    }

    pub fn repay_for_loan(who: T::AccountId, loan_id: LoanId) -> DispatchResult {
        let loan_asset_id = Self::loan_asset_id();
        let collateral_asset_id = Self::collateral_asset_id();
        let collection_account_id = Self::collection_account_id();
        let pawn_shop = Self::pawn_shop();

        ensure!(<Loans<T>>::contains_key(loan_id), "invalid loan id");
        let loan = <Loans<T>>::get(loan_id);
        ensure!(loan.who == who, "not owner of the loan");

        ensure!(
            <generic_asset::Module<T>>::free_balance(&loan_asset_id, &who)
                >= loan.loan_balance_total,
            "not enough asset to repay"
        );
        ensure!(
            <generic_asset::Module<T>>::free_balance(&collateral_asset_id, &pawn_shop)
                >= loan.collateral_balance_available,
            "not enough collateral asset in shop"
        );
        ensure!(
            !Self::check_loan_in_liquidation(&loan_id),
            "loan is in liquidation"
        );

        <LoansByAccount<T>>::mutate(&who, |v| {
            *v = v
                .clone()
                .into_iter()
                .filter(|ele| *ele != loan_id)
                .collect::<Vec<LoanId>>();
        });

        let revert_callback = || {
            <Loans<T>>::insert(&loan.id, &loan);
            <LoansByAccount<T>>::mutate(&who, |v| {
                v.push(loan.id);
            });
            <TotalLoan<T>>::mutate(|v| *v += loan.loan_balance_total);
            <TotalCollateral<T>>::mutate(|v| *v += loan.collateral_balance_available);
        };

        <generic_asset::Module<T>>::make_transfer_with_event(
            &loan_asset_id,
            &who,
            &collection_account_id,
            loan.loan_balance_total,
        )
        .or_else(|err| -> DispatchResult {
            revert_callback();
            Err(err)
        })?;
        <generic_asset::Module<T>>::make_transfer_with_event(
            &collateral_asset_id,
            &pawn_shop,
            &who,
            loan.collateral_balance_original,
        )
        .or_else(|err| -> DispatchResult {
            revert_callback();
            <generic_asset::Module<T>>::make_transfer_with_event(
                &loan_asset_id,
                &collection_account_id,
                &who,
                loan.loan_balance_total,
            )?;
            Err(err)
        })?;

        <Loans<T>>::remove(&loan.id);
        <TotalLoan<T>>::mutate(|v| *v -= loan.loan_balance_total);
        // <TotalCollateral<T>>::mutate(|v| *v -= loan.collateral_balance_available);
        <TotalCollateral<T>>::mutate(|v| *v -= loan.collateral_balance_original);

        Self::deposit_event(RawEvent::LoanRepaid(
            loan_id,
            loan.loan_balance_total,
            loan.collateral_balance_available,
        ));
        Ok(())
    }

    fn check_loan_in_liquidation(loan_id: &LoanId) -> bool {
        LiquidatingLoans::get().contains(loan_id)
    }

    pub fn mark_loan_liquidated(
        loan: &Loan<T::AccountId, T::Balance>,
        liquidation_account: T::AccountId,
        auction_balance: T::Balance,
    ) -> DispatchResult {

        ensure!(
            Self::check_loan_in_liquidation(&loan.id),
            "loan id not in liquidating"
        );

        let pawnshop = Self::pawn_shop();
        let collateral_asset_id = Self::collateral_asset_id();
        let collection_asset_id = Self::collection_asset_id();
        let collection_account_id = Self::collection_account_id();
        let loan_asset_id = Self::loan_asset_id();

        ensure!(
            <generic_asset::Module<T>>::free_balance(&loan_asset_id, &liquidation_account)
                >= auction_balance,
            "not enough asset to liquidate"
        );

        ensure!(
            auction_balance >= loan.loan_balance_total, "Not enough for loan liquidate"
        );

        <generic_asset::Module<T>>::make_transfer_with_event(
            &loan_asset_id,
            &liquidation_account,
            &collection_account_id,
            loan.loan_balance_total,
        )?;

        let leftover = auction_balance.checked_sub(&loan.loan_balance_total);

        if leftover.is_some() && leftover.unwrap() > T::Balance::zero() {
            let penalty_rate = Self::liquidation_penalty();
            let penalty =
                leftover.unwrap() * T::Balance::from(penalty_rate) / 100.into();

                <generic_asset::Module<T>>::make_transfer_with_event(
                &loan_asset_id,
                &collection_account_id,
                &Self::profit_pool(),   // TODO: can change to team account
                penalty,
            )
            .or_else(|err| -> DispatchResult {
                <generic_asset::Module<T>>::make_transfer_with_event(
                    &loan_asset_id,
                    &pawnshop,
                    &liquidation_account,
                    loan.loan_balance_total,
                )?;
                Err(err)
            })?;
            // part of the penalty will transfer to the loan owner
            <generic_asset::Module<T>>::make_transfer_with_event(
                &loan_asset_id,
                &collection_account_id,
                &loan.who,
                leftover.unwrap() - penalty,
            )
            .or_else(|err| -> DispatchResult {
                <generic_asset::Module<T>>::make_transfer_with_event(
                    &loan_asset_id,
                    &Self::profit_pool(),
                    &liquidation_account,
                    penalty,
                )?;

                // TODO: ensure pawnshop have enough collateral_asset
                <generic_asset::Module<T>>::make_transfer_with_event(
                    &collateral_asset_id,
                    &pawnshop,
                    &liquidation_account,
                    loan.collateral_balance_original,
                )?;
                Err(err)
            })?;
        }
        <Loans<T>>::remove(&loan.id);
        <LoansByAccount<T>>::mutate(&loan.who, |v| {
            *v = v
                .clone()
                .into_iter()
                .filter(|ele| ele != &loan.id)
                .collect::<Vec<LoanId>>();
        });
        LiquidatingLoans::mutate(|v| {
            *v = v
                .clone()
                .into_iter()
                .filter(|ele| ele != &loan.id)
                .collect::<Vec<LoanId>>();
        });
        Self::deposit_event(RawEvent::Liquidated(
            loan.id,
            loan.collateral_balance_original,
            loan.collateral_balance_available,
            auction_balance,
            loan.loan_balance_total,
        ));

        Ok(())
    }

    pub fn add_loan_collateral(
        loan: &Loan<T::AccountId, T::Balance>,
        from: T::AccountId,
        amount: T::Balance,
    ) -> DispatchResult {
        let pawnshop = Self::pawn_shop();
        let collateral_asset_id = Self::collection_asset_id();

        ensure!(
            <generic_asset::Module<T>>::free_balance(&collateral_asset_id, &from) >= amount,
            "not enough collateral asset in free balance"
        );

        <generic_asset::Module<T>>::make_transfer_with_event(
            &collateral_asset_id,
            &from,
            &pawnshop,
            amount,
        )?;

        <Loans<T>>::mutate(loan.id, |l| {
            l.collateral_balance_original += amount;
            l.collateral_balance_available += amount;
        });

        <TotalCollateral<T>>::mutate(|c| {
            *c += amount;
        });

        Self::deposit_event(RawEvent::AddCollateral(loan.id, amount));

        Ok(())
    }

    fn check_loan_health(
        loan: &Loan<T::AccountId, T::Balance>,
        btc_price: u64,
        liquidation: LTV,
        warning: LTV,
    ) -> LoanHealth {
        let current_ltv = <Loan<T::AccountId, T::Balance>>::get_ltv(
            loan.collateral_balance_available,
            loan.loan_balance_total,
            btc_price,
        );

        if current_ltv >= liquidation {
            return LoanHealth::Liquidating(current_ltv);
        }

        if current_ltv >= warning {
            return LoanHealth::Warning(current_ltv);
        }

        LoanHealth::Well
    }

    fn liquidate_loan(loan_id: LoanId, liquidating_ltv: LTV) {
        <Loans<T>>::mutate(loan_id, |v| {
            v.status = LoanHealth::Liquidating(liquidating_ltv)
        });
        if LiquidatingLoans::exists() {
            LiquidatingLoans::mutate(|v| v.push(loan_id));
        } else {
            let ll: Vec<LoanId> = vec![loan_id];
            LiquidatingLoans::put(ll);
        }
    }

    pub fn draw_from_loan(
        who: T::AccountId,
        loan_id: LoanId,
        amount: T::Balance,
    ) -> DispatchResult {
        ensure!(<Loans<T>>::contains_key(loan_id), "invalid loan id");
        let loan = Self::get_loan_by_id(loan_id);
        ensure!(loan.who == who, "can't draw from others loan");

        let collateral_asset_id = Self::collateral_asset_id();
        let token = <generic_asset::Module<T>>::symbols(collateral_asset_id);
        let current_price = <new_oracle::Module<T>>::current_price(&token);
        let btc_price: u64 = TryInto::<u64>::try_into(current_price).unwrap_or(0);

        let global_ltv = Self::global_ltv_limit();
        let available_credit = loan.collateral_balance_available
            * T::Balance::from(btc_price as u32)
            * T::Balance::from(global_ltv as u32)
            / T::Balance::from(LTV_PREC)
            / T::Balance::from(PRICE_PREC);

        ensure!(amount <= available_credit, "short of available credit");

        <Loans<T>>::mutate(loan_id, |v| {
            v.loan_balance_total = v.loan_balance_total + amount;
        });

        <Loans<T>>::mutate(loan_id, |v| {
            v.collateral_balance_available =
                v.collateral_balance_available - amount / T::Balance::from(btc_price as u32);
        });

        <TotalLoan<T>>::mutate(|v| *v += amount);

        Self::deposit_event(RawEvent::LoanDrawn(loan_id, amount));

        Ok(())
    }

    fn _pause(linum: u32) {
        Paused::mutate(|v| {
            *v = true;
        });
        Self::deposit_event(RawEvent::Paused(
            linum,
            <frame_system::Module<T>>::block_number(),
            <frame_system::Module<T>>::extrinsic_index().unwrap(),
        ));
    }

    fn on_each_block(_height: T::BlockNumber) {
        let collateral_asset_id = Self::collateral_asset_id();
        let liquidation_thd = Self::global_liquidation_threshold();
        let warning_thd = Self::global_warning_threshold();

        let token = <generic_asset::Module<T>>::symbols(collateral_asset_id);
        let current_price = <new_oracle::Module<T>>::current_price(&token);
        let btc_price: u64 = TryInto::<u64>::try_into(current_price).unwrap_or(0);

        for (loan_id, loan) in <Loans<T>>::enumerate() {
            if Self::check_loan_in_liquidation(&loan_id) {
                continue;
            }

            match Self::check_loan_health(&loan, btc_price, liquidation_thd, warning_thd) {
                LoanHealth::Well => {}
                LoanHealth::Warning(ltv) => {
                    if loan.status != LoanHealth::Warning(ltv) {
                        <Loans<T>>::mutate(&loan.id, |v| v.status = LoanHealth::Warning(ltv));
                        Self::deposit_event(RawEvent::Warning(loan_id, ltv));
                    }
                }

                LoanHealth::Liquidating(l) => {
                    Self::liquidate_loan(loan_id, l);
                    Self::deposit_event(RawEvent::Liquidating(
                        loan_id,
                        loan.who.clone(),
                        loan.collateral_balance_available,
                        loan.loan_balance_total,
                    ));
                }
            }
        }
    }

    fn calculate_loan_interest_rate() {
        let collection_asset_id = Self::collection_asset_id();
        let collection_account_id = Self::collection_account_id();
        let total_loan = Self::total_loan();
        let total_loan = TryInto::<u128>::try_into(total_loan).ok().unwrap();

        let total_deposit =
            <generic_asset::Module<T>>::free_balance(&collection_asset_id, &collection_account_id)
                + Self::total_loan();
        let total_deposit = TryInto::<u128>::try_into(total_deposit).ok().unwrap();

        let current_time = <timestamp::Module<T>>::get();
        <BonusTime<T>>::put(current_time);

        if !(total_deposit + total_loan).is_zero() {
            let utilization_rate_x = total_loan
                .checked_mul(10_u128.pow(8))
                .expect("saving share overflow")
                / (total_deposit + total_loan);

            // This is the real interest rate * 10^8
            let loan_interest_rate_current = if utilization_rate_x < 4000_00000 {
                (utilization_rate_x + 5000_0000) / 10
            } else if utilization_rate_x >= 8000_0000 {
                (30 * utilization_rate_x.pow(6)
                    + 10 * utilization_rate_x.pow(3) * 10_u128.pow(24)
                    + 6 * 10_u128.pow(48))
                    / 10_u128.pow(42)
            } else {
                (20 * utilization_rate_x * 10_u128.pow(8)) / 10_u128.pow(2)
            };

            let loan_interest_rate_current: T::Balance =
                TryFrom::<u128>::try_from(loan_interest_rate_current)
                    .ok()
                    .unwrap();

            let last_bonus_time: T::Moment = Self::bonus_time();

            let time_duration = TryInto::<u32>::try_into(current_time - last_bonus_time)
                .ok()
                .unwrap();
            let total_loan: T::Balance = TryFrom::<u128>::try_from(total_loan).ok().unwrap();

            let interest_generated = T::Balance::from(time_duration)
                * total_loan
                * loan_interest_rate_current
                / T::Balance::from(SEC_PER_DAY)
                / T::Balance::from(DAYS_PER_YEAR);

            let profit_pool = Self::profit_pool();
            let profit_asset = Self::profit_asset_id();

            for (loan_id, loan) in <Loans<T>>::enumerate() {
                let amount = interest_generated * loan.loan_balance_total
                    / (total_loan * T::Balance::from(10_u32.pow(8)));

                Self::draw_from_loan(loan.who.clone(), loan_id, amount).unwrap_or_default();

                <generic_asset::Module<T>>::make_transfer_with_event(
                    &collection_asset_id,
                    &loan.who,
                    &collection_account_id,
                    amount,
                )
                .unwrap_or_default();

                <TotalDtoken<T>>::mutate(|v| {
                    *v = v.checked_add(&amount).expect("Overflow of market dtoken");
                });
            }

            <LoanInterestRateCurrent<T>>::put(loan_interest_rate_current);
            let current_interest_rate = interest_generated
                / T::Balance::from(total_deposit as u32)
                * T::Balance::from(DAYS_PER_YEAR)
                * T::Balance::from(SEC_PER_DAY)
                * T::Balance::from(10_u32.pow(8));

            <SavingInterestRate<T>>::put(current_interest_rate);
        }
    }

    fn get_next_loan_id() -> LoanId {
        NextLoanId::mutate(|v| {
            let org = *v;
            *v += 1;
            org
        })
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        TotalCollateralUnderflow,
        ReachLoanCap,
        InvalidCollateralLoanAmounts,
        OverLTVLimit,
    }
}

decl_event!(
    #[rustfmt::skip]
    pub enum Event<T>
    where
        AccountId = <T as frame_system::Trait>::AccountId,
        Balance = <T as generic_asset::Trait>::Balance,
        Loan = Loan<<T as frame_system::Trait>::AccountId, <T as generic_asset::Trait>::Balance>,
        CollateralBalanceOriginal = <T as generic_asset::Trait>::Balance,
        CollateralBalanceAvailable = <T as generic_asset::Trait>::Balance,
        AuctionBalance = <T as generic_asset::Trait>::Balance,
        TotalLoanBalance = <T as generic_asset::Trait>::Balance,
        LineNumber = u32,
        BlockNumber = <T as frame_system::Trait>::BlockNumber,
        ExtrinsicIndex = u32,
    {
        LoanCreated(Loan),
        LoanDrawn(LoanId, Balance),
        LoanRepaid(LoanId, Balance, Balance),
        // Expired(LoanId, AccountId, Balance, Balance),
        // Extended(LoanId, AccountId),
        Warning(LoanId, LTV),
        Paused(LineNumber, BlockNumber, ExtrinsicIndex),

        Liquidating(LoanId, AccountId, CollateralBalanceAvailable, TotalLoanBalance),
        Liquidated(
            LoanId,
            CollateralBalanceOriginal,
            CollateralBalanceAvailable,
            AuctionBalance,
            TotalLoanBalance
        ),

        AddCollateral(LoanId, Balance),
    }
);
