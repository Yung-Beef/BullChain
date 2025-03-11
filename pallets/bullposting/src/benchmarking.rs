//! Benchmarking setup for pallet-bullposting

use super::*;

#[allow(unused)]
use crate::Pallet as BullPosting;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;
use frame_support::traits::{Get, fungible::{Inspect, Mutate}};
use frame_support::sp_runtime::*;
use crate::benchmarking::traits::{Zero, One};

const SEED: u32 = 0;
const MAX_URL: usize = 2000;
const MAX_VOTERS: u32 = 2000;

fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
	frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}

#[benchmarks]
mod benchmarks {
    use super::*;

    #[benchmark]
    fn try_submit_post<T: Config>() -> Result<(), BenchmarkError> {
		let post: Vec<u8> = [255u8; MAX_URL].to_vec();
		let post_id: [u8; 32] = sp_io::hashing::blake2_256(&post);
		let caller: T::AccountId = whitelisted_caller();
		let balance = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(u32::MAX.into());
		let bond = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(1000u32.into());

		frame_system::Pallet::<T>::set_block_number(One::one());

		<T as pallet::Config>::NativeBalance::set_balance(&caller, balance);

		#[extrinsic_call]
		try_submit_post(RawOrigin::Signed(caller.clone()), post, bond.clone());

		let voting_until = frame_system::Pallet::<T>::block_number() +
            T::VotingPeriod::get();

		assert_last_event::<T>(Event::PostSubmitted {
			id: post_id,
			submitter: caller,
			bond,
			voting_until,
		}.into());
		Ok(())
	}

    #[benchmark]
    fn try_submit_vote<T: Config>() -> Result<(), BenchmarkError> {
		let post: Vec<u8> = [255u8; MAX_URL].to_vec();
		let post_id: [u8; 32] = sp_io::hashing::blake2_256(&post);
		let alice: T::AccountId = account("Alice", 0, SEED);
		let bob: T::AccountId = account("Bob", 0, SEED);
		let balance = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(u32::MAX.into());
		let bond = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(1000u32.into());
		let vote_amount = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(5000u32.into());

		frame_system::Pallet::<T>::set_block_number(One::one());

		<T as pallet::Config>::NativeBalance::set_balance(&alice, balance);
		<T as pallet::Config>::NativeBalance::set_balance(&bob, balance);

		BullPosting::<T>::try_submit_post(RawOrigin::Signed(alice.clone()).into(), post.clone(), bond)?;

        #[extrinsic_call]
		try_submit_vote(RawOrigin::Signed(bob.clone()), post, vote_amount, Direction::Bullish);

		assert_last_event::<T>(Event::VoteSubmitted {
			id: post_id,
			voter: bob,
			vote_amount,
			direction: Direction::Bullish,
		}.into());
		Ok(())
	}

    #[benchmark]
    fn try_update_vote<T: Config>() -> Result<(), BenchmarkError> {
		let post: Vec<u8> = [255u8; MAX_URL].to_vec();
		let post_id: [u8; 32] = sp_io::hashing::blake2_256(&post);
		let alice: T::AccountId = account("Alice", 0, SEED);
		let bob: T::AccountId = account("Bob", 0, SEED);
		let balance = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(u32::MAX.into());
		let bond = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(1000u32.into());
		let vote_amount = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(5000u32.into());
		let new_vote_amount = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(6000u32.into());

		frame_system::Pallet::<T>::set_block_number(One::one());

		<T as pallet::Config>::NativeBalance::set_balance(&alice, balance);
		<T as pallet::Config>::NativeBalance::set_balance(&bob, balance);

		BullPosting::<T>::try_submit_post(RawOrigin::Signed(alice.clone()).into(), post.clone(), bond)?;
		BullPosting::<T>::try_submit_vote(RawOrigin::Signed(bob.clone()).into(), post.clone(), vote_amount, Direction::Bullish)?;

        #[extrinsic_call]
		try_update_vote(RawOrigin::Signed(bob.clone()), post, new_vote_amount, Direction::Bearish);

		assert_last_event::<T>(Event::VoteUpdated {
			id: post_id,
			voter: bob,
			vote_amount: new_vote_amount,
			direction: Direction::Bearish,
		}.into());
		Ok(())
	}

    #[benchmark]
    fn try_resolve_voting<T: Config>() -> Result<(), BenchmarkError> {
		let post: Vec<u8> = [250u8; MAX_URL].to_vec();
		let post_id: [u8; 32] = sp_io::hashing::blake2_256(&post);
		let alice: T::AccountId = account("Alice", 0, SEED);
		let bob: T::AccountId = account("Bob", 0, SEED);
		let balance = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(u32::MAX.into());
		let bond = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(1000u32.into());
		let vote_amount = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(5000u32.into());

		frame_system::Pallet::<T>::set_block_number(One::one());

		<T as pallet::Config>::NativeBalance::set_balance(&alice, balance);
		<T as pallet::Config>::NativeBalance::set_balance(&bob, balance);

		BullPosting::<T>::try_submit_post(RawOrigin::Signed(alice.clone()).into(), post.clone(), bond)?;
		BullPosting::<T>::try_submit_vote(RawOrigin::Signed(bob.clone()).into(), post.clone(), vote_amount, Direction::Bullish)?;

		let new_block_num = frame_system::Pallet::<T>::block_number() +
		T::VotingPeriod::get() + One::one();

		frame_system::Pallet::<T>::set_block_number(new_block_num);

        #[extrinsic_call]
		try_resolve_voting(RawOrigin::Signed(bob.clone()), post);
		
		assert_last_event::<T>(Event::PostEnded {
			id: post_id,
			submitter: alice,
			result: Direction::Bullish,
			rewarded: bond,
			slashed: Zero::zero(),
		}.into());

		Ok(())
	}

	#[benchmark]
    fn try_end_post<T: Config>(
		x: Linear<1, MAX_VOTERS>
	) -> Result<(), BenchmarkError> {
		let post: Vec<u8> = [255u8; MAX_URL].to_vec();
		let post_id: [u8; 32] = sp_io::hashing::blake2_256(&post);
		let alice: T::AccountId = account("Alice", 0, SEED);
		let bob: T::AccountId = account("Bob", 0, SEED);
		let balance = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(u32::MAX.into());
		let bond = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(1000u32.into());
		let vote_amount = <T as pallet::Config>::NativeBalance::minimum_balance().saturating_add(5000u32.into());

		frame_system::Pallet::<T>::set_block_number(One::one());

		<T as pallet::Config>::NativeBalance::set_balance(&alice, balance);
		<T as pallet::Config>::NativeBalance::set_balance(&bob, balance);

		BullPosting::<T>::try_submit_post(RawOrigin::Signed(alice.clone()).into(), post.clone(), bond)?;

		// Add a bunch of votes
		for i in 0..x {
			let acc: T::AccountId = account("filler", i, SEED);
			<T as pallet::Config>::NativeBalance::set_balance(&acc, balance);
			BullPosting::<T>::try_submit_vote(RawOrigin::Signed(acc).into(), post.clone(), vote_amount, Direction::Bullish)?;
		}

		let new_block_num = frame_system::Pallet::<T>::block_number() +
		T::VotingPeriod::get() + One::one();

		frame_system::Pallet::<T>::set_block_number(new_block_num);

		BullPosting::<T>::try_resolve_voting(RawOrigin::Signed(bob.clone()).into(), post.clone())?;

        #[extrinsic_call]
		try_end_post(RawOrigin::Signed(bob.clone()), post);

		// assert that the post is partially or fully resolved
		if x >= 1000 {
			assert_last_event::<T>(Event::PartiallyResolved {
				id: post_id
			}.into())
		} else {
			assert!(!Posts::<T>::contains_key(post_id));
		}
		Ok(())
	}

	impl_benchmark_test_suite!(BullPosting, crate::mock::new_test_ext(), crate::mock::Test);
}
