#![cfg(test)]
#![allow(dead_code)]

use crate::*;
use support::{assert_noop, assert_ok};

#[allow(unused_imports)]
use sp_runtime::{
    testing::Header,
    traits::{BlakeTwo256, IdentityLookup, OnFinalize, OnInitialize},
    Perbill,
};

use crate::mock::{
    constants::*, new_test_ext, Call, ExtBuilder, Origin, DepositLoanTest, SystemTest, TestEvent,
    TestRuntime,
};

#[test]
fn unittest_works() {
    ExtBuilder::default().build().execute_with(|| {});
    dbg!("hello world");
}
