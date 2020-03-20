#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use rstd::{prelude::*, vec};
use sp_runtime::traits::{Bounded, CheckedAdd, CheckedSub, EnsureOrigin, Zero};
use support::traits::{
    ChangeMembers, Currency, Get, LockIdentifier, LockableCurrency, ReservableCurrency,
    WithdrawReasons,
};
use support::{
    decl_error, decl_event, decl_module, decl_storage, dispatch::DispatchResult, StorageMap,
    StorageValue,
};
use system::{ensure_root, ensure_signed};

type BalanceOf<T> = <<T as Trait>::Currency as Currency<<T as system::Trait>::AccountId>>::Balance;
const LockedId: LockIdentifier = *b"oracle  ";

pub trait Trait: system::Trait {
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    /// Currency type
    type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>
        + ReservableCurrency<Self::AccountId>;

    /// The amount of fee that should be paid to each oracle during each reporting cycle.
    type OracleFee: Get<BalanceOf<Self>>;

    /// The amount that'll be slashed if one oracle missed its reporting window.
    type MissReportSlash: Get<BalanceOf<Self>>;

    /// The minimum amount to stake for an oracle candidate.
    type MinStaking: Get<BalanceOf<Self>>;

    /// The origin that's responsible for slashing malicious oracles.
    type MaliciousSlashOrigin: EnsureOrigin<Self::Origin>;

    /// The maxium count of working oracles.
    type Count: Get<u16>;

    /// The duration in which oracles should report and be paid.
    type ReportInteval: Get<Self::BlockNumber>;

    /// The duration between oracle elections.
    type ElectionEra: Get<Self::BlockNumber>;

    /// The locked time of staked amount.
    type LockedDuration: Get<Self::BlockNumber>;

    /// The actual oracle membership management type. (Usually the `srml_collective::Trait`)
    type ChangeMembers: ChangeMembers<Self::AccountId>;
}

/// Business module should use this trait to
/// communicate with oracle module in order to decouple them.
pub trait OracleMixedIn<T: system::Trait> {
    /// Tell oracle module that an event is reported by a speicifc oracle.
    fn on_witnessed(who: &T::AccountId);
    /// Predicate if one oracle is valid.
    fn is_valid(who: &T::AccountId) -> bool;
}

/// Unbind record for when an oracle is unbinding.
#[derive(PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Unbind<Balance, BlockNumber> {
    amount: Balance,
    era: BlockNumber,
}

/// The ledger of oracle's staked token.
#[derive(PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Ledger<Balance: Default, BlockNumber> {
    active: Balance,
    unbonds: Vec<Unbind<Balance, BlockNumber>>,
}

impl<Balance: Default, BlockNumber> Default for Ledger<Balance, BlockNumber> {
    fn default() -> Self {
        Ledger {
            active: Balance::default(),
            unbonds: vec![],
        }
    }
}

decl_storage! {
    trait Store for Module<T: Trait> as OracleStorage {
        /// Acting oracles.
        Oracles get(oracles): Vec<T::AccountId>;

        /// Staking ledger of oracle/candidates.
        OracleLedger get(oracle_ledger): map hasher(blake2_256) T::AccountId => Ledger<BalanceOf<T>, T::BlockNumber>;

        /// Blockstamp of each oracle's last event report.
        WitnessReport get(witness_report): map hasher(blake2_256) T::AccountId => T::BlockNumber;

        /// Oracle candidates.
        OracleCandidates get(candidates): Vec<T::AccountId>;

        /// Current election era.
        CurrentEra get(current_era): T::BlockNumber;

        /// Oracle reward records.
        OracleLastRewarded get(oracle_last_rewarded): map hasher(blake2_256) T::AccountId => T::BlockNumber;
    }
}

decl_error! {
    pub enum Error for Module<T: Trait> {
        StakingAmountTooSmall,
        UnbindingAmountTooBig,
    }
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where origin: T::Origin {
        fn deposit_event() = default;

        const OracleFee: BalanceOf<T> = T::OracleFee::get();
        const MissReportSlash: BalanceOf<T> = T::MissReportSlash::get();
        const MinStaking: BalanceOf<T> = T::MinStaking::get();
        const Count: u16 = T::Count::get();
        const ElectionEra: T::BlockNumber = T::ElectionEra::get();
        const ReportInteval: T::BlockNumber = T::ReportInteval::get();
        const LockedDuration: T::BlockNumber = T::LockedDuration::get();

        /// bind amount to list as oraclce candidates.
        pub fn bid(origin, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;

            Self::bind(&who, amount)?;
            Self::add_candidates(&who)?;
            Ok(())
        }

        /// slash oracle by third parties.
        pub fn slash_by_vote(origin, who: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
            T::MaliciousSlashOrigin::try_origin(origin)
                .map(|_| ())
                .or_else(ensure_root)
                .map_err(|_| "bad origin")?;
            T::Currency::slash(&who, amount);
            Self::deposit_event(RawEvent::OracleSlashed(who, amount));
            Ok(())
        }

        /// unbind amount.
        pub fn unbind(origin, amount: BalanceOf<T>) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::oracle_unbind(&who, amount)
        }

        /// Actions when finalizing a block:
        ///     1. Slash/reward oracles at end of eacch block.
        ///     2. Start an election at the right moment.
        ///     3. Release dued locked stake.
        fn on_finalize() {
            let block_number = <system::Module<T>>::block_number();
            Self::slash_and_reward_oracles(block_number);

            let current_era = Self::current_era();
            if block_number == current_era + T::ElectionEra::get(){
                Self::elect_oracles();
                <CurrentEra<T>>::put(block_number+T::ElectionEra::get());
            }
            Self::release_locked();
        }

    }
}

impl<T: Trait> Module<T> {
    fn release_locked() {
        let current_height = <system::Module<T>>::block_number();
        let current_oracles = Self::oracles();
        let new_candidates = Self::candidates();

        current_oracles
            .iter()
            .chain(new_candidates.iter())
            .for_each(|who| {
                let mut ledger = Self::oracle_ledger(who);
                let mut released = false;

                let mut total = ledger.active;
                for unbond in &ledger.unbonds {
                    total += unbond.amount;
                }

                ledger.unbonds = ledger
                    .unbonds
                    .into_iter()
                    .filter(|x| {
                        if x.era > current_height {
                            released = true;
                            false
                        } else {
                            true
                        }
                    })
                    .collect();

                if released {
                    let mut still_unbonding = <BalanceOf<T>>::default();
                    for unbond in &ledger.unbonds {
                        still_unbonding += unbond.amount;
                    }
                    let new_total = ledger.active + still_unbonding;

                    T::Currency::set_lock(LockedId, who, new_total, WithdrawReasons::all());
                    <OracleLedger<T>>::insert(who, ledger);
                    Self::deposit_event(RawEvent::OracleStakeReleased(
                        who.clone(),
                        total - new_total,
                    ));
                }
            });
    }

    fn slash_and_reward_oracles(block_number: T::BlockNumber) {
        let current_oracles = Self::oracles();

        current_oracles.iter().for_each(|o| {
            let last_report_height = Self::witness_report(o);
            if block_number > last_report_height + T::ReportInteval::get() {
                Self::slash(o, T::MissReportSlash::get());
            } else if block_number > Self::oracle_last_rewarded(o) + T::ReportInteval::get() {
                T::Currency::deposit_into_existing(o, T::OracleFee::get());
                <OracleLastRewarded<T>>::insert(o, block_number.clone());
                Self::deposit_event(RawEvent::OraclePaid(o.clone(), T::OracleFee::get()));
            }
        });
    }

    fn elect_oracles() {
        let current_oracles = Self::oracles();
        let new_candidates = Self::candidates();
        let mut all_candidates: Vec<T::AccountId> = Vec::new();

        all_candidates.extend(new_candidates);
        all_candidates.extend(current_oracles.clone());

        if all_candidates.len() == 0 {
            return;
        }

        if all_candidates.len() < T::Count::get().into() {
            return;
        }

        let all_candidates: Vec<(&T::AccountId, Ledger<BalanceOf<T>, T::BlockNumber>)> =
            all_candidates
                .iter()
                .map(|a| {
                    let ledger = Self::oracle_ledger(a);
                    (a, ledger)
                })
                .collect();

        let mut all_candidates: Vec<(&T::AccountId, Ledger<BalanceOf<T>, T::BlockNumber>)> =
            all_candidates
                .into_iter()
                .filter(|(_, ledger)| ledger.active > Zero::zero())
                .collect();
        all_candidates.sort_by_key(|(_, ledger)| ledger.active);

        let all_candidates = all_candidates
            .into_iter()
            .map(|(a, _)| a.clone())
            .collect::<Vec<T::AccountId>>();
        let (chosen_candidates, new_candidates) = all_candidates.split_at(T::Count::get().into());

        let mut chosen_candidates = chosen_candidates.to_vec();
        chosen_candidates.sort();

        let new_oracles: Vec<T::AccountId> = chosen_candidates
            .clone()
            .into_iter()
            .filter(|o| !current_oracles.contains(&o))
            .collect();
        let outgoing_oracles: Vec<T::AccountId> = current_oracles
            .into_iter()
            .filter(|o| !new_oracles.contains(&o))
            .collect();

        let current_height = <system::Module<T>>::block_number();
        new_oracles.iter().for_each(|o| {
            <WitnessReport<T>>::insert(o, current_height);
        });
        <Oracles<T>>::put(&chosen_candidates);
        T::ChangeMembers::change_members(&new_oracles, &outgoing_oracles, chosen_candidates);
        <OracleCandidates<T>>::put(new_candidates.to_vec());
    }
}

impl<T: Trait> Module<T> {
    fn slash(who: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
        let mut ledger = Self::oracle_ledger(who);

        let slash_amount = if amount > ledger.active {
            // Remove this oracle
            let current_oracles = Self::oracles();
            <Oracles<T>>::put(
                current_oracles
                    .iter()
                    .filter(|v| v != &who)
                    .collect::<Vec<&T::AccountId>>(),
            );
            T::ChangeMembers::change_members(&[], &[who.clone()], current_oracles);
            ledger.active
        } else {
            amount
        };

        // TODO: Handle imbalance
        T::Currency::slash(who, amount);
        ledger.active = ledger
            .active
            .checked_sub(&slash_amount)
            .ok_or("Error calculating new staking")?;
        <OracleLedger<T>>::insert(who, ledger);

        Self::deposit_event(RawEvent::OracleSlashed(who.clone(), slash_amount));
        Ok(())
    }

    fn oracle_unbind(who: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
        let current_height = <system::Module<T>>::block_number();
        let mut ledger = Self::oracle_ledger(who);

        if amount > ledger.active {
            return Err(Error::<T>::UnbindingAmountTooBig)?;
        }

        let new_unbond = Unbind {
            amount: amount,
            era: current_height + T::LockedDuration::get(),
        };

        ledger.active = ledger
            .active
            .checked_sub(&amount)
            .ok_or("Error calculating new staking")?;
        ledger.unbonds.push(new_unbond);

        <OracleLedger<T>>::insert(who, ledger);
        Self::deposit_event(RawEvent::OracleUnbonded(who.clone(), amount));
        Ok(())
    }

    fn bind(who: &T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
        let mut ledger = Self::oracle_ledger(who);
        let new_staked = ledger
            .active
            .checked_add(&amount)
            .ok_or("Error calculating new staking")?;
        if new_staked < T::MinStaking::get() {
            return Err(Error::<T>::StakingAmountTooSmall)?;
        }

        ledger.active = new_staked;
        <OracleLedger<T>>::insert(who, ledger);
        T::Currency::set_lock(LockedId, &who, amount, WithdrawReasons::all());
        Self::deposit_event(RawEvent::OracleBonded(who.clone(), amount));
        Ok(())
    }

    fn add_candidates(who: &T::AccountId) -> DispatchResult {
        let mut candidates = Self::candidates();
        if !candidates.contains(&who) {
            candidates.push(who.clone());
            <OracleCandidates<T>>::put(candidates);
            Self::deposit_event(RawEvent::CandidatesAdded(who.clone()));
        }
        Ok(())
    }
}

impl<T: Trait> OracleMixedIn<T> for Module<T> {
    fn on_witnessed(who: &T::AccountId) {
        let current_height = <system::Module<T>>::block_number();
        <WitnessReport<T>>::insert(who, current_height);
    }

    fn is_valid(who: &T::AccountId) -> bool {
        let report_height = Self::witness_report(who);
        report_height + T::ReportInteval::get() >= <system::Module<T>>::block_number()
    }
}

decl_event!(
    pub enum Event<T>
    where
        AccountId = <T as system::Trait>::AccountId,
        Balance = BalanceOf<T>,
    {
        /// Amount bonded by one oracle.
        OracleBonded(AccountId, Balance),
        /// Amount unbonded by one oracle.
        OracleUnbonded(AccountId, Balance),
        /// Amount slashed to one oracle.
        OracleSlashed(AccountId, Balance),
        /// Amount paid to one oracle.
        OraclePaid(AccountId, Balance),

        /// Candidate added.
        CandidatesAdded(AccountId),
        /// Candidate remove.
        CandidatesRemoved(AccountId),

        /// Amount unlocked for one oracle.
        OracleStakeReleased(AccountId, Balance),
    }
);
