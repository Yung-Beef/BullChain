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

        /// Determines whether Bullish post rewards are a flat number or coefficient.
        /// A value of false equals a flat number.
        /// A value of true equals a coefficient.
        #[pallet::constant]
        type RewardStyle: Get<bool>;

        /// The reward given to submitters of Bullish posts, only used if RewardStyle is set to false.
        #[pallet::constant]
        type FlatReward: Get<u32>;

        /// The coefficient used to determine a submitter's reward if their post is voted Bullish.
        /// Only used if RewardStyle is set to true.
        /// A value of 1 (bond 10 tokens, end up with 20 total)
        /// A value of 2 will reward them with 2x their bond (bond 10 tokens, end up with 30 total)
        #[pallet::constant]
        type RewardCoefficient: Get<u32>;

        /// Determines whether the bond for Bearish posts are slashed by a flat number or a coefficient.
        /// A value of false equals a flat number.
        /// A value of true equals a coefficient.
        #[pallet::constant]
        type SlashStyle: Get<bool>;

        /// The amount of tokens slashed from the submitter of a Bearish post, only used if SlashStyle is set to false.
        #[pallet::constant]
        type FlatSlash: Get<u32>;

        /// The coefficient used to determine how much of a a submitter's bond is slashed if their post is voted Bearish.
        /// Only used if SlashStyle is set to true.
        /// A value of 100 will slash 100% of their bond, a value of 50 will slash a 50% of their bond.
        /// If set to a value higher than 100, 100 will be used.
        #[pallet::constant]
        type SlashCoefficient: Get<u8>;

        /// The number of blocks that votes will last.
        #[pallet::constant]
        type VotingPeriod: Get<BlockNumberFor<Self>>;

        /// The minimum bond size accepted, bonds BELOW this number will be rejected.
        #[pallet::constant]
        type BondMinimum: Get<u32>;

        /// The minimum vote size accepted, votes BELOW this number will be rejected.
        #[pallet::constant]
        type VoteMinimum: Get<u32>;

        /// The maximum number of accounts that can vote on a particular post.
        #[pallet::constant]
        type MaxVoters: Get<u32>;

        /// The amount of tokens locked to store a post in storage.
        #[pallet::constant]
        type StorageRent: Get<u32>;

        /// The allowed length of the "URL" (or other) input into functions.
        #[pallet::constant]
        type MaxUrlLength: Get<u32>;

        /// The maximum amount of unfreezes that can be done per `try_end_post`.
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
        pub ended: bool,
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
        /// Vote closed and ended, rewarding or slashing the submitter.
        PostEnded {
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
        PartiallyResolved {
            id: [u8; 32]
        },
        PostResolved {
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
        /// Submitted URL input was too long (acceptable length configured in runtime).
        InputTooLong,
        /// Submitted URL was empty.
        Empty,
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
        /// Voting has ended but nobody has called try_end_voting() yet.
        VotingUnresolved,
        /// Vote already closed and nded.
        PostAlreadyEnded,
        /// The voting period for a post has ended.
        VotingEnded,
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
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_url ([`Error::Empty`])
        /// - If the bondy is below the `BondMinimum` ([`Error::BondTooLow`])
        /// - If post input is higher than the `MaxUrlLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post has been submitted previously ([`Error::PostAlreadyExists`])
        /// - If the submitter does not have sufficient free tokens for their bond and the storage rent ([`Error::InsufficientFreeBalance`])
        #[pallet::call_index(0)]
        pub fn try_submit_post(
            origin: OriginFor<T>,
            post_url: Vec<u8>,
            bond: BalanceOf<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_url.is_empty(), Error::<T>::Empty);

            ensure!(bond >= T::BondMinimum::get().into(), Error::<T>::BondTooLow);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxUrlLength> = BoundedVec::try_from(post_url).map_err(|_| Error::<T>::InputTooLong)?;

            Self::submit_post(who, bounded, bond)?;

            Ok(())
        }

        /// Submits a vote on whether a particular post is bullish or bearish.
        ///
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_url ([`Error::Empty`])
        /// - If the vote is below the VoteMinimum ([`Error::VoteTooLow`])
        /// - If post input is higher than the `MaxUrlLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post does not exist ([`Error::PostDoesNotExist`])
        /// - If the voting has already ended ([`Error::VotingEnded`])
        /// - If the post has already reached `MaxVoters` ([`Error::VotersMaxed`])
        /// - If they have already voted once ([`Error::AlreadyVoted`])
        /// - If the user tries to vote with more than their balance ([`Error::InsufficientFreeBalance`])
        #[pallet::call_index(1)]
        pub fn try_submit_vote(
            origin: OriginFor<T>,
            post_url: Vec<u8>,
            vote_amount: BalanceOf<T>,
            direction: Direction,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_url.is_empty(), Error::<T>::Empty);

            ensure!(vote_amount >= T::VoteMinimum::get().into(), Error::<T>::VoteTooLow);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxUrlLength> = BoundedVec::try_from(post_url).map_err(|_| Error::<T>::InputTooLong)?;

            Self::submit_vote(who, bounded, vote_amount, direction)?;

            Ok(())
        }


        /// Updates an account's vote and freeze accordingly. Only possible before a post is ended.
        ///
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_url ([`Error::Empty`])
        /// - If the vote is below the VoteMinimum ([`Error::VoteTooLow`])
        /// - If post input is higher than the `MaxUrlLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post does not exist ([`Error::PostDoesNotExist`])
        /// - If the voting has already ended ([`Error::VotingEnded`])
        /// - If this particular vote doesn't exist (['Error::VoteDoesNotExist'])
        /// - If the user does not have enough balance for their new vote ([`Error::InsufficientBalance`])
        #[pallet::call_index(2)]
        pub fn try_update_vote(
            origin: OriginFor<T>,
            post_url: Vec<u8>,
            new_vote: BalanceOf<T>,
            direction: Direction
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_url.is_empty(), Error::<T>::Empty);

            ensure!(new_vote >= T::VoteMinimum::get().into(), Error::<T>::VoteTooLow);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxUrlLength> = BoundedVec::try_from(post_url).map_err(|_| Error::<T>::InputTooLong)?;

            Self::update_vote(who, bounded, new_vote, direction)?;
            
            Ok(())
        }


        /// Resolves a post's vote, rewarding or slashing the submitter and enabling try_end_post.
        /// Callable by anyone.
        ///
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_url ([`Error::Empty`])
        /// - If post input is higher than the `MaxUrlLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post does not exist ([`Error::PostDoesNotExist`])
        /// - If the vote is still in progress ([`Error::VotingStillOngoing`])
        /// - If the post has already been ended ([`Error::PostAlreadyEnded`])
        #[pallet::call_index(3)]
        pub fn try_resolve_voting(
            origin: OriginFor<T>,
            post_url: Vec<u8>,
        ) -> DispatchResult {
            let _who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_url.is_empty(), Error::<T>::Empty);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxUrlLength> = BoundedVec::try_from(post_url).map_err(|_| Error::<T>::InputTooLong)?;

            Self::resolve_voting(bounded)?;

            Ok(())
        }

        /// Unlocks the submitter's storage rent and unfreezes all votes on that post.
        /// Callable by anyone.
        ///
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If they submit nothing for the post_url ([`Error::Empty`])
        /// - If post input is higher than the `MaxUrlLength` set in the runtime ([`Error::InputTooLong`])
        /// - If the post does not exist ([`Error::PostDoesNotExist`])
        /// - If the post is unended ([`Error::VotingUnresolved`])
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::try_end_post(T::UnfreezeLimit::get()))]
        pub fn try_end_post(
            origin: OriginFor<T>,
            post_url: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let _who = ensure_signed(origin)?;
            // Ensure the post input is not empty
            ensure!(!post_url.is_empty(), Error::<T>::Empty);

            // Convert the post input into a bounded vec to use in the actual logic, errors if too long
            let bounded: BoundedVec<u8, T::MaxUrlLength> = BoundedVec::try_from(post_url).map_err(|_| Error::<T>::InputTooLong)?;

            let weight_used = Self::end_post(bounded)?;

            Ok(weight_used)
        }
    }


    impl<T: Config> Pallet<T> {
        pub(crate) fn submit_post(
            who: T::AccountId,
            post_url: BoundedVec<u8, T::MaxUrlLength>,
            bond: BalanceOf<T>
        ) -> DispatchResult {
            let id = sp_io::hashing::blake2_256(&post_url);

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
                ended: false,
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
            post_url: BoundedVec<u8, T::MaxUrlLength>,
            vote_amount: BalanceOf<T>,
            direction: Direction,
        ) -> DispatchResult {
            let id = sp_io::hashing::blake2_256(&post_url);

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
            post_url: BoundedVec<u8, T::MaxUrlLength>,
            new_vote: BalanceOf<T>,
            direction: Direction
        ) -> DispatchResult {
            let id = sp_io::hashing::blake2_256(&post_url);

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
            post_url: BoundedVec<u8, T::MaxUrlLength>
        ) -> DispatchResult {
            let id = sp_io::hashing::blake2_256(&post_url);

            // Error if the post does not exist.
            ensure!(Posts::<T>::contains_key(&id), Error::<T>::PostDoesNotExist);
            let post_struct = Posts::<T>::get(&id).expect("Already checked that it exists");
            let submitter = post_struct.submitter.clone();

            // Check if the voting period is over for that post
            // If current block number is lower than the post's voting_until, voting has not ended; error.
            ensure!(frame_system::Pallet::<T>::block_number() >= post_struct.voting_until, Error::<T>::VotingStillOngoing);

            // Error if the post has already been ended.
            ensure!(!post_struct.ended, Error::<T>::PostAlreadyEnded);

            // End the voting and update storage
            let updated_post_struct = Post {
                ended: true,
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

                Self::deposit_event(Event::PostEnded { 
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

                Self::deposit_event(Event::PostEnded { 
                    id,
                    submitter,
                    result,
                    rewarded: Zero::zero(),
                    slashed,
                });
            } else {
                // Does nothing if tie/no votes
                Self::deposit_event(Event::PostEnded { 
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
            post_url: BoundedVec<u8, T::MaxUrlLength>
        ) -> DispatchResultWithPostInfo {
            let id = sp_io::hashing::blake2_256(&post_url);

            // Error if the post does not exist.
            ensure!(Posts::<T>::contains_key(&id), Error::<T>::PostDoesNotExist);
            let post_struct = Posts::<T>::get(&id).expect("Already checked that it exists");

            // Error if the post is not ended yet
            ensure!(post_struct.ended, Error::<T>::VotingUnresolved);

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
                Self::deposit_event(Event::PostResolved {
                    id,
                });
                Ok(Some(T::WeightInfo::try_end_post(unfreeze_count)).into())
            } else {
                Self::deposit_event(Event::PartiallyResolved {
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
