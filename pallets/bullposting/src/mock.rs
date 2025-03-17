use crate as pallet_parachain_bullposting;
use frame_support::{
    derive_impl,
    parameter_types,
};
use sp_runtime::BuildStorage;

type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u64;

#[frame_support::runtime]
mod runtime {
	// The main runtime
	#[runtime::runtime]
	// Runtime Types to be generated
	#[runtime::derive(
		RuntimeCall,
		RuntimeEvent,
		RuntimeError,
		RuntimeOrigin,
		RuntimeFreezeReason,
		RuntimeHoldReason,
		RuntimeSlashReason,
		RuntimeLockId,
		RuntimeTask
	)]
	pub struct Test;

	#[runtime::pallet_index(0)]
	pub type System = frame_system::Pallet<Test>;

    #[runtime::pallet_index(1)]
	pub type Balances = pallet_balances::Pallet<Test>;

	#[runtime::pallet_index(2)]
	pub type Bullposting = pallet_parachain_bullposting::Pallet<Test>;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
    type Block = Block;
    type AccountData = pallet_balances::AccountData<Balance>;
}

parameter_types! {
    pub const MaxFreezes: u32 = 10000;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type AccountStore = System;
    type FreezeIdentifier = RuntimeFreezeReason;
    type MaxFreezes = MaxFreezes;
}

type BlockNumber = u64;

parameter_types! {
    // false = FlatReward, true = RewardCoefficient
    pub const RewardStyle: bool = true;
    pub const FlatReward: u32 = 500;
    pub const RewardCoefficient: u32 = 100;
    // false = FlatSlash, true = SlashCoefficient
    pub const SlashStyle: bool = true;
    pub const FlatSlash: u32 = 500;
    pub const SlashCoefficient: u8 = 100;
    pub const VotingPeriod: BlockNumber = 1000;
    pub const BondMinimum: u32 = 50;
    pub const VoteMinimum: u32 = 50;
    pub const MaxVoters: u32 = 2000;
    pub const StorageRent: u32 = 100;
    pub const MaxInputLength: u32 = 2000;
    pub const UnfreezeLimit: u32 = 1000;
}

impl pallet_parachain_bullposting::Config for Test {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = ();
    type NativeBalance = Balances;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type FreezeIdentifier = RuntimeFreezeReason;
    type RewardStyle = RewardStyle;
    type FlatReward = FlatReward;
    type RewardCoefficient = RewardCoefficient;
    type SlashStyle = SlashStyle;
    type FlatSlash = FlatSlash;
    type SlashCoefficient = SlashCoefficient;
    type VotingPeriod = VotingPeriod;
    type BondMinimum = BondMinimum;
    type VoteMinimum = VoteMinimum;
    type MaxVoters = MaxVoters;
    type StorageRent = StorageRent;
    type MaxInputLength = MaxInputLength;
    type UnfreezeLimit = UnfreezeLimit;
}

// Build genesis storage according to the mock runtime.
pub fn new_test_ext() -> sp_io::TestExternalities {
    let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
    let genesis = pallet_balances::GenesisConfig::<Test> { 
        balances: vec![(0, 1001), (1, 1001), (2, 1001), (3, 1001), (10000, 1001)]
    };
    genesis.assimilate_storage(&mut t).unwrap();
    t.into()
}
