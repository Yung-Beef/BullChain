//! Run `cargo doc --package pallet-bullposting --open` to view this pallet's documentation.

// We make sure this pallet uses `no_std` for compiling to Wasm.
#![cfg_attr(not(feature = "std"), no_std)]



// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

// FRAME pallets require their own "mock runtimes" to be able to run unit tests. This module
// contains a mock runtime specific for testing this pallet's functionality.
#[cfg(test)]
mod mock;

// This module contains the unit tests for this pallet.
// Learn about pallet unit testing here: https://docs.substrate.io/test/unit-testing/
#[cfg(test)]
mod tests;

// Every callable function or "dispatchable" a pallet exposes must have weight values that correctly
// estimate a dispatchable's execution time. The benchmarking module is used to calculate weights
// for each dispatchable and generates this pallet's weight.rs file. Learn more about benchmarking here: https://docs.substrate.io/test/benchmark/
#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::*;

// All pallet logic is defined in its own module and must be annotated by the `pallet` attribute.
#[frame_support::pallet]
pub mod pallet {
    // Import various useful types required by all FRAME pallets.
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    // Other imports
    use codec::MaxEncodedLen;
    use scale_info::prelude::{fmt::Debug, vec::Vec};
    use frame_support::{
        traits::{
            tokens::{fungible, Preservation, Fortitude, Precision},
            fungible::{Inspect, Mutate, MutateHold, MutateFreeze},
        },
        sp_runtime::{
            traits::{CheckedSub, Zero},
            Permill,
            Percent,
        },
        BoundedVec,
    };

    // The `Pallet` struct serves as a placeholder to implement traits, methods and dispatchables
    // (`Call`s) in this pallet.
    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The pallet's configuration trait.
    ///
    /// All our types and constants a pallet depends on must be declared here.
    /// These types are defined generically and made concrete when the pallet is declared in the
    /// `runtime/src/lib.rs` file of your chain.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching runtime event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// A type representing the weights required by the dispatchables of this pallet.
        type WeightInfo: crate::weights::WeightInfo;
        /// A type representing the token used.
        type NativeBalance: fungible::Mutate<Self::AccountId>
        + fungible::hold::Mutate<Self::AccountId, Reason = Self::RuntimeHoldReason>
        + fungible::freeze::Mutate<Self::AccountId, Id = Self::RuntimeFreezeReason>;

        /// A type representing the reason an account's tokens are being held.
        type RuntimeHoldReason: From<HoldReason>;
        /// A type representing the reason an account's tokens are being frozen.
        type RuntimeFreezeReason: From<FreezeReason>;
        /// The ID type for freezes.
		type FreezeIdentifier: Parameter + Member + MaxEncodedLen + Copy;

        /// Determines which reward mechanism is used if a post is determined to be Bullish.
        /// False == FlatReward
        /// True == RewardCoefficient
        #[pallet::constant]
        type RewardStyle: Get<bool>;

        /// Determines the submitter's token reward if their post is determined to be Bullish, independent of their bond.
        /// Due to this, submitters will likely only bond the BondMinimum, as they will earn the same reward regardless,
        /// with a lower slash risk.
        /// NOTE: This will only happen if `RewardStyle == false`
        #[pallet::constant]
        type FlatReward: Get<BalanceOf<Self>>;

        /// Determines the submitter's reward if their post is determined to be Bullish, based on the size of their bond.
        /// A value of 100 is a 1x reward. 200 will give a 2x reward
        /// (eg. you bond 500 tokens, you will receive a 1000 token reward and end with 1500 tokens).
        /// Values between 0 and 100 can be used as well.
        /// NOTE: This will only happen if `RewardStyle == true`
        #[pallet::constant]
        type RewardCoefficient: Get<u32>;

        /// Determines which slashing mechanism is used if a post is determined to be Bearish.
        /// False == FlatSlash
        /// True == SlashCoefficient
        #[pallet::constant]
        type SlashStyle: Get<bool>;

        /// Determines how many of the submitter's bonded tokens are slashed if their post is determined to be Bearish.
        /// If this is set higher than the bond of a post, only the submitter's full bond will be slashed
        /// (eg. if you bond 50 tokens and FlatSlash == 100, you will only be slashed 50).
        /// NOTE: This will only happen if `SlashStyle == false`.
        #[pallet::constant]
        type FlatSlash: Get<BalanceOf<Self>>;

        /// Determines how much of the submitter's bond is slashed if their post is determined to be Bearish.
        /// A value of 100 will slash 100% of their bond, a value of 50 will slash a 50% of their bond.
        /// If set to a value higher than 100, 100 will be used.
        /// NOTE: This will only happen if `SlashStyle == true`.
        #[pallet::constant]
        type SlashCoefficient: Get<u8>;

        /// Determines for how many blocks the voting period of a post will run based on the block number the post was submitted at.
        /// Votes submitted after the period ends will fail. Once the period ends, voting can be resolved with `try_resolve_voting`.
        #[pallet::constant]
        type VotingPeriod: Get<BlockNumberFor<Self>>;

        /// Determines the minimum amount of tokens that are acceptable to bond when submitting a post.
        /// Calling `try_submit_post` with a bond value lower than this amount will fail.
        #[pallet::constant]
        type BondMinimum: Get<BalanceOf<Self>>;

        /// Determines the minimum amount of tokens that are acceptable to vote with.
        /// Calling `try_submit_vote` or `try_update_vote` with votes smaller than this value will fail.
        #[pallet::constant]
        type VoteMinimum: Get<BalanceOf<Self>>;

        /// Determines the maximum amount of accounts that can vote on a post.
        /// This is used to bound a vector storing all of the accounts that have voted on a particular post,
        /// so performance will slow as the value is increased (assuming the `MaxVoters` limit is actually reached on posts).
        /// Calling `try_submit_vote` on a post that has reached the `MaxVoters` limit will fail.
        #[pallet::constant]
        type MaxVoters: Get<u32>;

        /// Determines the amount of tokens that must be locked in order to submit a post.
        /// This is separate from the post's bond and is not involved in the reward process.
        /// This value should be sufficiently high to prevent storage bloat attacks.
        /// The rent is unlocked once a post is ended (and thus removed from storage).
        #[pallet::constant]
        type StorageRent: Get<BalanceOf<Self>>;

        /// Determines the maximum acceptable length of submitted inputs.
        /// The inputs are simply checked to ensure they are short enough, and then hashed, so this can be quite high in practice.
        /// Calls with an input longer than this value will fail.
        #[pallet::constant]
        type MaxInputLength: Get<u32>;

        /// Determines the maximum number of accounts that can have their vote unfrozen when executing `try_end_post`.
        /// If the number of votes on a post exceeds this value, `try_end_post` will need to be called again.
        #[pallet::constant]
        type UnfreezeLimit: Get<u32>;

    }

    pub type BalanceOf<T> =
        <<T as Config>::NativeBalance as fungible::Inspect<<T as frame_system::Config>::AccountId>>::Balance;

    /// Used for the direction of votes and results
    #[derive(Debug, PartialEq, Clone, Encode, Decode, TypeInfo, Default, MaxEncodedLen)]
    pub enum Direction {
        #[default]
        Bullish,
        Bearish,
        Tie,
    }

    /// A reason for the pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
        /// Bond of a post
        #[codec(index = 0)]
        PostBond,
        // Locked for storage rent, unlockable after voting ends
        #[codec(index = 1)]
        StorageRent,
	}

    /// A reason for the pallet freezing funds.
	#[pallet::composite_enum]
	pub enum FreezeReason {
        /// Voting
        #[codec(index = 0)]
        Vote,
	}

    #[derive(MaxEncodedLen, Debug, PartialEq, Clone, Encode, Decode, TypeInfo)]
    #[scale_info(skip_type_params(T))]
    pub struct Post<T: Config> {
        pub submitter: T::AccountId,
        pub bond: BalanceOf<T>,
        pub bull_votes: BalanceOf<T>,
        pub bear_votes: BalanceOf<T>,
        pub voting_until: BlockNumberFor<T>,
        pub resolved: bool,
    }

    /// Stores the post ID as the key and a post struct (with the additional info such as the submitter) as the value
    #[pallet::storage]
    pub type Posts<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], Post<T>>;

    
    /// Stores the vote size per account and post
    #[pallet::storage]
    pub type Votes<T: Config> = StorageDoubleMap<
    _,
    Blake2_128Concat,
    T::AccountId,
    Blake2_128Concat,
    [u8; 32],
    (BalanceOf<T>, Direction),
    ValueQuery,
    >;

    /// Stores the list of voters on each post ID
    #[pallet::storage]
    pub type Voters<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], BoundedVec<T::AccountId, T::MaxVoters>>;

    /// Stores the number of votes on each post ID
    #[pallet::storage]
    pub type VoteCounts<T: Config> =
        StorageMap<_, Blake2_128Concat, [u8; 32], u32>;

    /// Events that functions in this pallet can emit.
    ///
    /// Events are a simple means of indicating to the outside world (such as dApps, chain explorers
    /// or other users) that some notable update in the runtime has occurred. In a FRAME pallet, the
    /// documentation for each event field and its parameters is added to a node's metadata so it
    /// can be used by external interfaces or tools.
    ///
    ///	The `generate_deposit` macro generates a function on `Pallet` called `deposit_event` which
    /// will convert the event type of your pallet into `RuntimeEvent` (declared in the pallet's
    /// [`Config`] trait) and deposit it using [`frame_system::Pallet::deposit_event`].
    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Post submitted successfully.
        PostSubmitted {
            /// The post ID.
            id: [u8; 32],
            /// The account that submitted the post and bonded tokens.
            submitter: T::AccountId,
            /// Amount of bonded tokens.
            bond: BalanceOf<T>,
            /// Duration of voting period.
            voting_until: BlockNumberFor<T>,
        },
        /// Vote submitted successfully.
        VoteSubmitted {
            /// The post ID.
            id: [u8; 32],
            /// The account voting on the post.
            voter: T::AccountId,
            /// The amount of tokens frozen for the vote.
            vote_amount: BalanceOf<T>,
            /// Bullish or bearish vote.
            direction: Direction,
        },
        /// Vote updated successfully.
        VoteUpdated {
            /// The post ID.
            id: [u8; 32],
            /// The account voting on the post.
            voter: T::AccountId,
            /// The amount of tokens frozen for the vote.
            vote_amount: BalanceOf<T>,
            /// Bullish or bearish vote.
            direction: Direction,
        },
        /// Vote resolved, rewarding or slashing the submitter.
        VotingResolved {
            /// The post ID.
            id: [u8; 32],
            /// The account that submitted the post and bonded tokens.
            submitter: T::AccountId,
            /// Bullish means the submitter was rewarded, Bearish means they were slashed
            result: Direction,
            rewarded: BalanceOf<T>,
            slashed: BalanceOf<T>,
        },
        VoteUnfrozen {
            id: [u8; 32],
            account: T::AccountId,
            amount: BalanceOf<T>,
        },
        PostPartiallyEnded {
            id: [u8; 32]
        },
        PostEnded {
            id: [u8; 32]
        }
    }

    /// Errors that can be returned by this pallet.
    ///
    /// Errors tell users that something went wrong so it's important that their naming is
    /// informative. Similar to events, error documentation is added to a node's metadata so it's
    /// equally important that they have helpful documentation associated with them.
    ///
    /// This type of runtime error can be up to 4 bytes in size should you want to return additional
    /// information.
    #[pallet::error]
    pub enum Error<T> {
        /// Submitted input was too long (acceptable length configured in runtime).
        InputTooLong,
        /// Submitted input was empty.
        EmptyInput,
        /// Attempted bond was below the BondMinimum configured in the runtime.
        BondTooLow,
        /// Attempted vote was below the VoteMinimum configured in the runtime.
        VoteTooLow,
        /// Post already submitted.
        PostAlreadyExists,
        /// Insufficient available balance.
        InsufficientFreeBalance,
        /// Post has not been submitted.
        PostDoesNotExist,
        /// The post being voted upon has already reached `MaxVoters`.
        VotersMaxed,
        /// Account already voted on a particular post
        AlreadyVoted,
        /// If you try to unfreeze a vote that was already unfrozen or never happened in the first place.
        VoteDoesNotExist,
        /// Vote still in progress.
        VotingStillOngoing,
        /// Voting has ended but nobody has called try_resolve_voting() yet.
        VotingUnresolved,
        /// The voting period for a post has ended.
        VotingEnded,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        // Only runs during a runtime upgrade
        #[cfg(feature = "try-runtime")]
        fn try_state(_n: BlockNumber) -> Result<(), TryRuntimeError> {
            // Ensure a storages were not wiped.
            ensure!(!Posts::<T>::iter().count().is_zero(), "Posts storage is empty");

            ensure!(!Votes::<T>::iter().count().is_zero(), "Votes storage is empty");

            ensure!(!Voters::<T>::iter().count().is_zero(), "Voters storage is empty");

            ensure!(!VoteCount::<T>::iter().count().is_zero(), "VoteCount storage is empty");

            Ok(())
        }
}


    /// The pallet's dispatchable functions ([`Call`]s).
    ///
    /// Dispatchable functions allows users to interact with the pallet and invoke state changes.
    /// These functions materialize as "extrinsics", which are often compared to transactions.
    /// They must always return a `DispatchResult` and be annotated with a weight and call index.
    ///
    /// The [`call_index`] macro is used to explicitly
    /// define an index for calls in the [`Call`] enum. This is useful for pallets that may
    /// introduce new dispatchables over time. If the order of a dispatchable changes, its index
    /// will also change which will break backwards compatibility.
    ///
    /// The [`weight`] macro is used to assign a weight to each call.
    /// 
    #[pallet::call(weight(<T as Config>::WeightInfo))]
    impl<T: Config> Pallet<T> {
        /// Submits a post to the chain for voting.
        /// If the post is ultimately voted as bullish, they will receive a reward.
        /// If it is voted as bearish, they will be slashed.
        /// Rewards and slashes are configured in the runtime and can be based on the bond, which as a minimum.
        /// A storage rent fee is also held during the voting period, and once it it unlocked the post is cleared from storage.
        /// 
        /// ## Parameters
        /// - `origin`: The origin calling the extrinsic
        /// - `post_input`: The caller's input (essentially a string)
        /// - `bond`: The amount of tokens being bonded by the caller
        ///
        /// ## Errors
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_input ([`Error::EmptyInput`])
        /// - If the bondy is below the `BondMinimum` ([`Error::BondTooLow`])
        /// - If post input is higher than the `MaxInputLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post has been submitted previously ([`Error::PostAlreadyExists`])
        /// - If the submitter does not have sufficient free tokens for their bond and the storage rent ([`Error::InsufficientFreeBalance`])
        #[pallet::call_index(0)]
        pub fn try_submit_post(
            origin: OriginFor<T>,
            post_input: Vec<u8>,
            bond: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_input.is_empty(), Error::<T>::EmptyInput);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxInputLength> = BoundedVec::try_from(post_input).map_err(|_| Error::<T>::InputTooLong)?;

            // Ensure the bond is higher than `BondMinimum`
            ensure!(bond >= T::BondMinimum::get().into(), Error::<T>::BondTooLow);

            Self::submit_post(who, bounded, bond)?;

            Ok(())
        }

        /// Submits a vote on whether a particular post is bullish or bearish. Only possible before a post is ended.
        /// 
        /// ## Parameters
        /// - `origin`: The origin calling the extrinsic
        /// - `post_input`: The caller's input (essentially a string)
        /// - `vote_amount`: The amount of tokens being used to vote by the caller
        /// - `direction`: Whether the caller things the post being voted upon is `Bullish` or `Bearish`
        /// 
        /// ## Errors
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_input ([`Error::EmptyInput`])
        /// - If the vote is below the VoteMinimum ([`Error::VoteTooLow`])
        /// - If post input is higher than the `MaxInputLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post does not exist ([`Error::PostDoesNotExist`])
        /// - If the voting has already ended ([`Error::VotingEnded`])
        /// - If the post has already reached `MaxVoters` ([`Error::VotersMaxed`])
        /// - If they have already voted once ([`Error::AlreadyVoted`])
        /// - If the user tries to vote with more than their balance ([`Error::InsufficientFreeBalance`])
        #[pallet::call_index(1)]
        pub fn try_submit_vote(
            origin: OriginFor<T>,
            post_input: Vec<u8>,
            vote_amount: BalanceOf<T>,
            direction: Direction,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_input.is_empty(), Error::<T>::EmptyInput);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxInputLength> = BoundedVec::try_from(post_input).map_err(|_| Error::<T>::InputTooLong)?;

            // Ensure the vote is higher than `VoteMinimum`
            ensure!(vote_amount >= T::VoteMinimum::get().into(), Error::<T>::VoteTooLow);

            Self::submit_vote(who, bounded, vote_amount, direction)?;

            Ok(())
        }


        /// Updates an account's vote and freeze accordingly. Only possible before a post is ended.
        ///
        /// ## Parameters
        /// - `origin`: The origin calling the extrinsic
        /// - `post_input`: The caller's input (essentially a string)
        /// - `new_vote`: The new amount of tokens being used by the caller to update their previous vote on a particular input
        /// - `direction`: Whether the caller things the post being voted upon is `Bullish` or `Bearish`
        /// 
        /// ## Errors
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_input ([`Error::EmptyInput`])
        /// - If the vote is below the VoteMinimum ([`Error::VoteTooLow`])
        /// - If post input is higher than the `MaxInputLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post does not exist ([`Error::PostDoesNotExist`])
        /// - If the voting has already ended ([`Error::VotingEnded`])
        /// - If this particular vote doesn't exist (['Error::VoteDoesNotExist'])
        /// - If the user does not have enough balance for their new vote ([`Error::InsufficientBalance`])
        #[pallet::call_index(2)]
        pub fn try_update_vote(
            origin: OriginFor<T>,
            post_input: Vec<u8>,
            new_vote: BalanceOf<T>,
            direction: Direction
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_input.is_empty(), Error::<T>::EmptyInput);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxInputLength> = BoundedVec::try_from(post_input).map_err(|_| Error::<T>::InputTooLong)?;

            // Ensure the vote is higher than `VoteMinimum`
            ensure!(new_vote >= T::VoteMinimum::get().into(), Error::<T>::VoteTooLow);

            Self::update_vote(who, bounded, new_vote, direction)?;
            
            Ok(())
        }


        /// Resolves a post's vote, rewarding or slashing the submitter and enabling try_end_post.
        /// Callable by anyone.
        /// 
        /// ## Parameters
        /// - `origin`: The origin calling the extrinsic
        /// - `post_input`: The caller's input (essentially a string)
        /// 
        /// ## Errors
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_input ([`Error::EmptyInput`])
        /// - If post input is higher than the `MaxInputLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post does not exist ([`Error::PostDoesNotExist`])
        /// - If the vote is still in progress ([`Error::VotingStillOngoing`])
        #[pallet::call_index(3)]
        pub fn try_resolve_voting(
            origin: OriginFor<T>,
            post_input: Vec<u8>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_input.is_empty(), Error::<T>::EmptyInput);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxInputLength> = BoundedVec::try_from(post_input).map_err(|_| Error::<T>::InputTooLong)?;

            Self::resolve_voting(bounded)?;

            Ok(())
        }

        /// Unlocks the submitter's storage rent and unfreezes all votes on that post.
        /// Callable by anyone.
        /// 
        /// ## Parameters
        /// - `origin`: The origin calling the extrinsic
        /// - `post_input`: The caller's input (essentially a string)
        ///
        /// ## Errors
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_input ([`Error::EmptyInput`])
        /// - If post input is higher than the `MaxInputLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post does not exist ([`Error::PostDoesNotExist`])
        /// - If the voting is unresolved ([`Error::VotingUnresolved`])
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::try_end_post(T::UnfreezeLimit::get()))]
        pub fn try_end_post(
            origin: OriginFor<T>,
            post_input: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let _who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_input.is_empty(), Error::<T>::EmptyInput);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxInputLength> = BoundedVec::try_from(post_input).map_err(|_| Error::<T>::InputTooLong)?;

            let weight_used = Self::end_post(bounded)?;

            Ok(weight_used)
        }
    }


    impl<T: Config> Pallet<T> {
        pub(crate) fn submit_post(
            who: T::AccountId,
            post_input: BoundedVec<u8, T::MaxInputLength>,
            bond: BalanceOf<T>
        ) -> DispatchResult {
            let id = sp_io::hashing::blake2_256(&post_input);

            // Checks if the post exists
            ensure!(!Posts::<T>::contains_key(&id), Error::<T>::PostAlreadyExists);

            let storage_rent = T::StorageRent::get();

            // Checks if they have enough balance available to be bonded
            let reduc_bal = <<T as Config>::NativeBalance>::
            reducible_balance(&who, Preservation::Preserve, Fortitude::Polite);
            reduc_bal.checked_sub(&bond).ok_or(Error::<T>::InsufficientFreeBalance)?;
            reduc_bal.checked_sub(&storage_rent.into()).ok_or(Error::<T>::InsufficientFreeBalance)?;

            // Bonds the submitter's balance
            <<T as Config>::NativeBalance>::hold(&HoldReason::PostBond.into(), &who, bond)?;

            // Holds the storage rent
            <<T as Config>::NativeBalance>::hold(&HoldReason::StorageRent.into(), &who, storage_rent.into())?;

            let voting_until = frame_system::Pallet::<T>::block_number() +
            T::VotingPeriod::get();

            // Stores the submitter and bond info
            Posts::<T>::insert(&id, Post {
                submitter: who.clone(),
                bond,
                bull_votes: Zero::zero(),
                bear_votes: Zero::zero(),
                voting_until,
                resolved: false,
            });

            // Emit an event.
            Self::deposit_event(Event::PostSubmitted {
                id,
                submitter: who,
                bond, voting_until
            });

            Ok(())
        }

        pub(crate) fn submit_vote(
            who: T::AccountId,
            post_input: BoundedVec<u8, T::MaxInputLength>,
            vote_amount: BalanceOf<T>,
            direction: Direction,
        ) -> DispatchResult {
            let id = sp_io::hashing::blake2_256(&post_input);

            // Error if the post does not exist.
            ensure!(Posts::<T>::contains_key(&id), Error::<T>::PostDoesNotExist);
            let post_struct = Posts::<T>::get(&id).expect("Already checked that it exists");
            
            // Check if voting is still open for that post
            // If current block number is greater than or equal to the ending period of the post's voting, error.
            ensure!(frame_system::Pallet::<T>::block_number() < post_struct.voting_until, Error::<T>::VotingEnded);

            // Ensure MaxVoters has not been reached
            if let Some(voters) = VoteCounts::<T>::get(id) {
                ensure!(voters != T::MaxVoters::get(), Error::<T>::VotersMaxed)
            }

            // Check if they have already voted
            ensure!(!Votes::<T>::contains_key(&who, &id), Error::<T>::AlreadyVoted);

            // Check if they have enough balance for the freeze
            ensure!(vote_amount < <<T as Config>::NativeBalance>::total_balance(&who), Error::<T>::InsufficientFreeBalance);

            // Extend_freeze
            <<T as Config>::NativeBalance>::extend_freeze(&FreezeReason::Vote.into(), &who, vote_amount)?;

            // Store vote for account and post
            Votes::<T>::insert(&who, &id, (vote_amount, &direction));

            // Update the list of voters for this post
            match Voters::<T>::get(id) {
                None => {
                    let mut v: BoundedVec<T::AccountId, T::MaxVoters> = BoundedVec::new();
                    let _ = v.try_push(who.clone());
                    Voters::<T>::insert(&id, v)
                },
                Some(mut v) => {
                    // Will never error as we already checked regarding the number of voters (vector length) to ensure space
                    let _ = v.try_push(who.clone());
                    Voters::<T>::insert(&id, v)
                },
            };

            // Update the number of voters for this post
            match VoteCounts::<T>::get(id) {
                None => { VoteCounts::<T>::insert(id, 1) },
                Some(x) => { VoteCounts::<T>::insert(id, x + 1) },
            }

            // Stores vote info/updates post struct according to vote direction
            let updated_post_struct = match direction {
                Direction::Bullish => {
                    Post {
                        bull_votes: post_struct.bull_votes + vote_amount,
                        ..post_struct
                    }
                },
                Direction::Bearish => {
                    Post {
                        bear_votes: post_struct.bear_votes + vote_amount,
                        ..post_struct
                    }
                },
                Direction::Tie => {
                    post_struct
                }
            };

            Posts::<T>::insert(&id, updated_post_struct);

            // Emit an event.
            Self::deposit_event(Event::VoteSubmitted {
                id,
                voter: who,
                vote_amount,
                direction,
            });

            Ok(())
        }

        pub(crate) fn update_vote(
            who: T::AccountId,
            post_input: BoundedVec<u8, T::MaxInputLength>,
            new_vote: BalanceOf<T>,
            direction: Direction
        ) -> DispatchResult {
            let id = sp_io::hashing::blake2_256(&post_input);

            // Error if the post does not exist.
            ensure!(Posts::<T>::contains_key(&id), Error::<T>::PostDoesNotExist);
            let post_struct = Posts::<T>::get(&id).expect("Already checked that it exists");

            // Check if voting is still open for that post
            // If current block number is greater than or equal to the ending period of the post's voting, error.
            ensure!(frame_system::Pallet::<T>::block_number() < post_struct.voting_until, Error::<T>::VotingEnded);

            // Error if this particular vote no longer exists or never existed.
            ensure!(Votes::<T>::contains_key(&who, &id), Error::<T>::VoteDoesNotExist);

            // Error if they do not have enough balance for the freeze
            ensure!(new_vote < <<T as Config>::NativeBalance>::total_balance(&who), Error::<T>::InsufficientFreeBalance);

            let (previous_amount, previous_direction) = Votes::<T>::take(&who, &id);

            // Extend_freeze
            <<T as Config>::NativeBalance>::extend_freeze(&FreezeReason::Vote.into(), &who, new_vote)?;

            // Store vote
            Votes::<T>::insert(&who, &id, (new_vote, &direction));

            // Updates post struct's vote totals according to vote amount and direction
            // Removes previous directional vote and adds new vote
            let updated_post_struct = match direction {
                Direction::Bullish => {
                    if previous_direction == Direction::Bullish {
                        Post {
                            bull_votes: post_struct.bull_votes - previous_amount + new_vote,
                            ..post_struct
                        }
                    } else {
                        Post {
                            bull_votes: post_struct.bull_votes + new_vote,
                            bear_votes: post_struct.bear_votes - previous_amount,
                            ..post_struct
                        }
                    }
                },
                Direction::Bearish => {
                    if previous_direction == Direction::Bearish {
                        Post {
                            bear_votes: post_struct.bear_votes - previous_amount + new_vote,
                            ..post_struct
                        }
                    } else {
                        Post {
                            bull_votes: post_struct.bull_votes - previous_amount,
                            bear_votes: post_struct.bear_votes + new_vote,
                            ..post_struct
                        }
                    }
                },
                Direction::Tie => {
                    if previous_direction == Direction::Bullish {
                        Post {
                            bull_votes: post_struct.bull_votes - previous_amount,
                            ..post_struct
                        }
                    } else {
                        Post {
                            bear_votes: post_struct.bear_votes - previous_amount,
                            ..post_struct
                        }
                    }
                }
            };

            Posts::<T>::insert(&id, updated_post_struct);

            // Emit an event.
            Self::deposit_event(Event::VoteUpdated {
                id,
                voter: who,
                vote_amount: new_vote,
                direction,
            });

            Ok(())
        }

        pub(crate) fn resolve_voting(
            post_input: BoundedVec<u8, T::MaxInputLength>
        ) -> DispatchResult {
            let id = sp_io::hashing::blake2_256(&post_input);

            // Error if the post does not exist.
            ensure!(Posts::<T>::contains_key(&id), Error::<T>::PostDoesNotExist);
            let post_struct = Posts::<T>::get(&id).expect("Already checked that it exists");
            let submitter = post_struct.submitter.clone();

            // Check if the voting period is over for that post
            // If current block number is lower than the post's voting_until, voting has not ended; error.
            ensure!(frame_system::Pallet::<T>::block_number() >= post_struct.voting_until, Error::<T>::VotingStillOngoing);

            // End the voting and update storage
            let updated_post_struct = Post {
                resolved: true,
                ..post_struct
            };
            Posts::<T>::insert(&id, &updated_post_struct);

            // Reward/slash amount
            let bond = post_struct.bond;

            // Unlock submitter's bond
            <<T as Config>::NativeBalance>::release(&HoldReason::PostBond.into(), &submitter, bond, Precision::BestEffort)?;

            let result: Direction = if updated_post_struct.bull_votes > updated_post_struct.bear_votes {
                Direction::Bullish
            } else if updated_post_struct.bull_votes < updated_post_struct.bear_votes {
                Direction::Bearish
            } else {
                Direction::Tie
            };

            // Reward/slash submitter or do nothing if there is a tie/no votes
            if result == Direction::Bullish {
                // Reward the submitter
                let rewarded = match T::RewardStyle::get() {
                    false => Self::reward_flat(&submitter)?,
                    true => Self::reward_coefficient(&submitter, &bond)?,
                };

                Self::deposit_event(Event::VotingResolved { 
                    id,
                    submitter,
                    result,
                    rewarded,
                    slashed: Zero::zero(),
                });
            } else if result == Direction::Bearish {
                // Slashes the submitter
                let slashed = match T::SlashStyle::get() {
                    false => Self::slash_flat(&submitter, bond)?,
                    true => Self::slash_coefficient(&submitter, &bond)?,
                };

                Self::deposit_event(Event::VotingResolved { 
                    id,
                    submitter,
                    result,
                    rewarded: Zero::zero(),
                    slashed,
                });
            } else {
                // Does nothing if tie/no votes
                Self::deposit_event(Event::VotingResolved { 
                    id,
                    submitter,
                    result: Direction::Tie,
                    rewarded: Zero::zero(),
                    slashed: Zero::zero(),
                });
            }

            Ok(())
        }
        
        // Reward a flat amount
        pub(crate) fn reward_flat(who: &T::AccountId) -> Result<BalanceOf<T>, DispatchError> {
            let reward = T::FlatReward::get().into();

            // Reward the submitter
            <<T as Config>::NativeBalance>::mint_into(&who, reward)?;

            Ok(reward)
        }
        
        // Reward based on a coefficient and how much they bonded
        pub(crate) fn reward_coefficient(who: &T::AccountId, bond: &BalanceOf<T>) -> Result<BalanceOf<T>, DispatchError> {
            let reward = Permill::from_percent(T::RewardCoefficient::get()) * *bond;

            // Reward the submitter
            <<T as Config>::NativeBalance>::mint_into(&who, reward)?;

            Ok(reward)
        }

        // Slash a flat amount
        pub(crate) fn slash_flat(who: &T::AccountId, bond: BalanceOf<T>) -> Result<BalanceOf<T>, DispatchError> {
            let flat_slash = T::FlatSlash::get().into();
            
            // Slash the submitter up to their full bond amount, but not beyond
            if bond < flat_slash {
                <<T as Config>::NativeBalance>::burn_from(&who, bond, Preservation::Protect, Precision::BestEffort, Fortitude::Force)?;
                Ok(bond)
            } else {
                <<T as Config>::NativeBalance>::burn_from(&who, flat_slash, Preservation::Protect, Precision::BestEffort, Fortitude::Force)?;
                Ok(flat_slash)
            }
        }

        // Slash based on a coefficient and how much they bonded
        pub(crate) fn slash_coefficient(who: &T::AccountId, bond: &BalanceOf<T>) -> Result<BalanceOf<T>, DispatchError> {
            let percent = if T::SlashCoefficient::get() <= 100 {
                T::SlashCoefficient::get()
            } else {
                100
            };
            
            let slash = Percent::from_percent(percent) * *bond;
            
            // Slashes the submitter
            <<T as Config>::NativeBalance>::burn_from(&who, slash, Preservation::Protect, Precision::BestEffort, Fortitude::Force)?;
            
            Ok(slash)
        }

        pub(crate) fn end_post(
            post_input: BoundedVec<u8, T::MaxInputLength>
        ) -> DispatchResultWithPostInfo {
            let id = sp_io::hashing::blake2_256(&post_input);

            // Error if the post does not exist.
            ensure!(Posts::<T>::contains_key(&id), Error::<T>::PostDoesNotExist);
            let post_struct = Posts::<T>::get(&id).expect("Already checked that it exists");

            // Error if the voting is unresolved
            ensure!(post_struct.resolved, Error::<T>::VotingUnresolved);

            let mut unfreeze_count = 0u32;

            // Assume all will be unfrozen
            let mut all_unfrozen = true;

            // Call unfreeze_vote() for each voter and remove from `Voters` up to `UnfreezeLimit` or until all voters are removed
            if let Some(mut voters) = Voters::<T>::take(id) {
                while !(unfreeze_count >= T::UnfreezeLimit::get()) {
                    match voters.pop() {
                        Some(voter) => {
                            Self::unfreeze_vote(voter, id)?;
                            unfreeze_count += 1;
                        },
                        None => break
                    }
                }
                if unfreeze_count >= T::UnfreezeLimit::get() {
                    all_unfrozen = false;
                    Voters::<T>::insert(id, voters);
                }
            }

            if all_unfrozen {
                // Unlock the storage rent of the submitter
                <<T as Config>::NativeBalance>::release(&HoldReason::StorageRent.into(), &post_struct.submitter, T::StorageRent::get().into(), Precision::BestEffort)?;

                // Remove from Posts storage
                let _ = Posts::<T>::take(id);

                // Emit an event
                Self::deposit_event(Event::PostEnded {
                    id,
                });
                Ok(Some(T::WeightInfo::try_end_post(unfreeze_count)).into())
            } else {
                Self::deposit_event(Event::PostPartiallyEnded {
                    id,
                });
                Ok(().into())
            }
        }

        pub(crate) fn unfreeze_vote(
            who: T::AccountId,
            id: [u8; 32]
        ) -> DispatchResult {
            // Remove from Votes and get vote amount
            let (amount, _direction) = Votes::<T>::take(&who, id);

            // Remove freeze
            <<T as Config>::NativeBalance>::decrease_frozen(&FreezeReason::Vote.into(), &who, amount.clone())?;

            // Decrease vote count or remove if 0
            if let Some(count) = VoteCounts::<T>::get(id) {
                if count > 1 {
                    VoteCounts::<T>::insert(id, count - 1)
                } else {
                    VoteCounts::<T>::remove(id)
                }
            };

            // Emit an event
            Self::deposit_event(Event::VoteUnfrozen {
                id,
                account: who,
                amount,
            });

            Ok(())
        }
    }
}
