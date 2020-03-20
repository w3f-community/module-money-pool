#![cfg(test)]
#![allow(dead_code)]

use super::*;
use primitives::H256;
use support::{
    impl_outer_dispatch, impl_outer_event, impl_outer_origin, parameter_types, weights::Weight,
};
use crate::{GenesisConfig, Module, Trait};
use balances;
use assets;
use std::cell::RefCell;

#[allow(unused_imports)]
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, ConvertInto, IdentityLookup, OnFinalize, OnInitialize},
    Perbill,
};

thread_local! {
    pub(crate) static EXISTENTIAL_DEPOSIT: RefCell<u128> = RefCell::new(0);
    static TRANSFER_FEE: RefCell<u128> = RefCell::new(0);
    static CREATION_FEE: RefCell<u128> = RefCell::new(0);
}

pub mod constants {
    use super::TestRuntime;

    pub const DECIMALS: u128 = 100000000; // satoshi

    pub const ROOT: <TestRuntime as system::Trait>::AccountId = 1;
    pub const ALICE: <TestRuntime as system::Trait>::AccountId = 2;
    pub const BOB: <TestRuntime as system::Trait>::AccountId = 3;
    pub const CHRIS: <TestRuntime as system::Trait>::AccountId = 4;
    #[allow(dead_code)]
    pub const DAVE: <TestRuntime as system::Trait>::AccountId = 5;
    pub const TEAM: <TestRuntime as system::Trait>::AccountId = 6;
    pub const PROFIT_POOL: <TestRuntime as system::Trait>::AccountId = 7;

    pub const COLLECTION_ACCOUNT_ID: <TestRuntime as system::Trait>::AccountId = 999;
    pub const PAWN_SHOP: <TestRuntime as system::Trait>::AccountId = 888;
    pub const LIQUIDATION_ACCOUNT: <TestRuntime as system::Trait>::AccountId = 8;

    pub const NUM_OF_PHASE: u32 = 5;

    pub const RBTC_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC1_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC2_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC3_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC4_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const RSC5_INITIAL_BALANCE: u128 = 1000000 * DECIMALS;
    pub const SBTC_INITIAL_BALANCE: u128 = 0 * DECIMALS;

    pub const RBTC_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 1;
    pub const RSC1_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 2;
    pub const RSC2_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 3;
    pub const RSC3_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 4;
    pub const RSC4_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 5;
    pub const RSC5_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 6;
    pub const SBTC_ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 7;
    pub const ASSET_ID: <TestRuntime as pallet_generic_asset::Trait>::AssetId = 8;
}

use self::constants::*;

// For testing the module, we construct most of a mock runtime. This means
// first constructing a configuration type (`Test`) which `impl`s each of the
// configuration traits of modules we want to use.
#[derive(Clone, Eq, PartialEq)]
pub struct TestRuntime;

impl_outer_origin! {
    pub enum Origin for TestRuntime {}
}

mod deposit_loan {
    pub use crate::Event;
}
impl_outer_event! {
    pub enum TestEvent for TestRuntime {
        deposit_loan<T>,
    }
}
type Balances = balances::Module<TestRuntime>;
type System = system::Module<TestRuntime>;
type Sudo = sudo::Module<TestRuntime>;
type Assets = assets::Module<TestRuntime>;
impl_outer_dispatch! {
    pub enum Call for TestRuntime where origin: Origin {
        balances::Balances,
        system::System,
        sudo::Sudo,
        assets::Assets,
    }
}

parameter_types! {
    pub const BlockHashCount: u64 = 250;
    pub const MaximumBlockWeight: u32 = 1024;
    pub const MaximumBlockLength: u32 = 2 * 1024;
    pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl system::Trait for TestRuntime {
    type Origin = Origin;
    type Index = u64;
    type BlockNumber = u64;
    type Hash = H256;
    type Call = ();
    type Hashing = BlakeTwo256;
    type AccountId = u64;
    type Lookup = IdentityLookup<Self::AccountId>;
    type Header = Header;
    type Event = ();
    type BlockHashCount = BlockHashCount;
    type MaximumBlockWeight = MaximumBlockWeight;
    type MaximumBlockLength = MaximumBlockLength;
    type AvailableBlockRatio = AvailableBlockRatio;
    type Version = ();
    type ModuleToIndex = ();
}
parameter_types! {
    pub const ExistentialDeposit: u128 = 0;
    pub const TransferFee: u128 = 0;
    pub const CreationFee: u128 = 0;
}
impl balances::Trait for TestRuntime {
    type Balance = u128;
    // type OnFreeBalanceZero = ();
    type OnNewAccount = ();
    type Event = ();
    type TransferPayment = ();
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    // type TransferFee = TransferFee;
    type CreationFee = CreationFee;
    type OnReapAccount = ();
}

parameter_types! {
    pub const MinimumPeriod: u64 = 1000;
}
impl timestamp::Trait for TestRuntime {
    type Moment = u64;
    type OnTimestampSet = ();
    type MinimumPeriod = MinimumPeriod;
}
impl sudo::Trait for TestRuntime {
    type Event = ();
    type Proposal = Call;
}

parameter_types! {
    pub const REPORTER: <TestRuntime as system::Trait>::AccountId = 1;
}

impl price::Trait for TestRuntime {
    type Event = ();
    type OracleMixedIn = (); // OracleMixedIn 为trait
    type ReportOrigin = ();
    type OnChange = ();
}

impl pallet_generic_asset::Trait for TestRuntime {
    type Event = ();
    type Balance = u128;
    type AssetId = u32;
}
impl assets::Trait for TestRuntime {
    type Event = ();
    type OnAssetMint = ();
    type OnAssetCreate = ();
    type OnAssetTransfer = ();
    type OnAssetBurn = ();
    type BeforeAssetMint = ();
    type BeforeAssetCreate = ();
    type BeforeAssetTransfer = ();
    type BeforeAssetBurn = ();
}
impl Trait for TestRuntime {
    type Event = ();
}

pub type DepositLoanTest = Module<TestRuntime>;

pub type SystemTest = system::Module<TestRuntime>;

pub struct ExtBuilder {}

impl Default for ExtBuilder {
    fn default() -> Self {
        Self {}
    }
}

impl ExtBuilder {
    pub fn build(self) -> runtime_io::TestExternalities {
        new_test_ext()
    }
}

// This function basically just builds a genesis storage key/value store according to
// our desired mockup.
pub fn new_test_ext() -> runtime_io::TestExternalities {
    let mut t = system::GenesisConfig::default()
        .build_storage::<TestRuntime>()
        .unwrap();

    sudo::GenesisConfig::<TestRuntime> { key: ROOT }
        .assimilate_storage(&mut t)
        .unwrap();

    pallet_generic_asset::GenesisConfig::<TestRuntime> {
        next_asset_id: 9,
        staking_asset_id: 0,
        spending_asset_id: 0,
        assets: vec![],
        initial_balance: 0,
        endowed_accounts: vec![],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    assets::GenesisConfig::<TestRuntime> {
        symbols: vec![
            (SBTC_ASSET_ID, "SBTC".as_bytes().to_vec()),
            (RBTC_ASSET_ID, "RBTC".as_bytes().to_vec()),
            (RSC1_ASSET_ID, "RSC1".as_bytes().to_vec()),
            (RSC2_ASSET_ID, "RSC2".as_bytes().to_vec()),
            (RSC3_ASSET_ID, "RSC3".as_bytes().to_vec()),
            (RSC4_ASSET_ID, "RSC4".as_bytes().to_vec()),
            (RSC5_ASSET_ID, "RSC5".as_bytes().to_vec()),
            (ASSET_ID, "RIO".as_bytes().to_vec()),    // TODO: change name
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();

    GenesisConfig::<TestRuntime> {
        current_btc_price: 8000_0000,
        collateral_asset_id: SBTC_ASSET_ID,
        loan_asset_id: ASSET_ID,
        global_ltv_limit: 6500,
        global_liquidation_threshold: 9000,
        global_warning_threshold: 8000,
        next_loan_id: 1,
        next_loan_package_id: 1,
        pawn_shop: PAWN_SHOP,
        profit_pool: PROFIT_POOL,
        penalty_rate: 200,
        liquidation_account: LIQUIDATION_ACCOUNT,
        minimum_collateral: 2_000_0000,
        liquidation_penalty: 1300,
        collection_account_id: COLLECTION_ACCOUNT_ID,
        collection_asset_id: SBTC_ASSET_ID,
        profit_asset_id: ASSET_ID,
        reserved_mint_asset_id: ASSET_ID,
        reserved_mint_wallet: BOB,
        share_asset_id: RBTC_ASSET_ID,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    t.into()
}
