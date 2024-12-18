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
    use codec::{EncodeLike, MaxEncodedLen};
    use scale_info::{prelude::fmt::Debug, StaticTypeInfo};
    use frame_support::traits::tokens::{fungible, Preservation, Fortitude, IdAmount};
    use frame_support::BoundedVec;
    use frame_support::traits::fungible::{Inspect, MutateHold, InspectFreeze, MutateFreeze};
    use frame_support::sp_runtime::traits::{CheckedSub, CheckedAdd};


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
        type WeightInfo: WeightInfo;
        /// A type representing the post submitted to the chain by a user. Will likely be hashed by the client from a string.
        type Post: MaxEncodedLen + EncodeLike + Decode + StaticTypeInfo + Clone + Debug + PartialEq;
        /// A type representing the token used.
        type NativeBalance: fungible::Inspect<Self::AccountId>
        + fungible::Mutate<Self::AccountId>
        + fungible::hold::Inspect<Self::AccountId>
        + fungible::hold::Mutate<Self::AccountId, Reason = Self::RuntimeHoldReason>
        + fungible::freeze::Inspect<Self::AccountId>
        + fungible::freeze::Mutate<Self::AccountId, Id = Self::RuntimeFreezeReason>;

        /// A type representing the reason an account's tokens are being held.
        type RuntimeHoldReason: From<HoldReason>;

        /// A type representing the reason an account's tokens are being frozen.
        type RuntimeFreezeReason: From<FreezeReason>;
        /// The ID type for freezes.
		type FreezeIdentifier: Parameter + Member + MaxEncodedLen + Copy;
        /// The maximum number of individual freeze locks that can exist on an account at any time.
		#[pallet::constant]
		type MaxFreezes: Get<u32>;
    }

    type BalanceOf<T> =
        <<T as Config>::NativeBalance as fungible::Inspect<<T as frame_system::Config>::AccountId>>::Balance;

    /// Used for the direction of votes and results
    #[derive(Debug, PartialEq, Clone, Encode, Decode, TypeInfo)]
    pub enum Direction {
        Bullish,
        Bearish,
    }

    /// A reason for the pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
        /// Submitting a post
        PostBond,
	}

    /// A reason for the pallet freezing funds.
	#[pallet::composite_enum]
	pub enum FreezeReason {
        /// Voting
        Vote,
	}

    // TODO: change i128 to generic, but it needs to be implemented as something bigger than whatever the balance type is
    #[derive(MaxEncodedLen, Debug, PartialEq, Clone, Encode, Decode, TypeInfo)]
    #[scale_info(skip_type_params(T))]

    pub struct Post<T: Config> {
        pub submitter: T::AccountId,
        pub bond: BalanceOf<T>,
        pub votes: i128,
        pub voting_until: BlockNumberFor<T>,
    }

    /// A storage item for this pallet.
    ///
    /// In this template, we are declaring a storage item called `Something` that stores a single
    /// `u32` value. Learn more about runtime storage here: <https://docs.substrate.io/build/runtime-storage/>
    #[pallet::storage]
    pub type Something<T> = StorageValue<_, u32>;


    /// Stores the submitter of a post
    #[pallet::storage]
    pub type Posts<T: Config> =
        StorageMap<_, Blake2_128Concat, <T as pallet::Config>::Post, Post<T>>;

    /// Freeze locks on account balances.
	#[pallet::storage]
	pub type Freezes<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<IdAmount<T::FreezeIdentifier, T::NativeBalance>, T::MaxFreezes>,
		ValueQuery,
	>;

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
        /// A user has successfully set a new value.
        SomethingStored {
            /// The new value set.
            something: u32,
            /// The account who set the new value.
            who: T::AccountId,
        },
        /// A user has submitted a post to the chain
        PostSubmitted {
            post: T::Post,
            submitter: T::AccountId,
            bond: BalanceOf<T>,
            voting_until: BlockNumberFor<T>,
        },
        /// A user has voted on whether a particular post is bullish or bearish
        VoteSubmitted {
            post: T::Post,
            who: T::AccountId,
            vote_amount: BalanceOf<T>,
            direction: Direction,
        },
        /// The vote on a particular post has been closed
        VoteClosed {
            post: T::Post,
            submitter: T::AccountId,
            result: Direction,
        },
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
        /// The value retrieved was `None` as no value was previously set.
        NoneValue,
        /// There was an attempt to increment the value in storage over `u32::MAX`.
        StorageOverflow,
        /// If a post is submitted with a voting period shorter than the period set in the runtime.
        PeriodTooShort,
        /// If someone tries to submit a post that has already been submitted.
        PostAlreadyExists,
        /// If someone tries to submit a post but does not have sufficient free tokens to bond the amount they wanted to bond.
        InsufficientBalance,
        /// If someone tries to vote on a post that has not been submitted.
        PostDoesNotExist,
        /// If someone tries to vote on a post that has already passed the voting period.
        VoteAlreadyClosed,
        /// If there is an overflow while doing checked_add().
        Overflow,
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
    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// An example dispatchable that takes a single u32 value as a parameter, writes the value
        /// to storage and emits an event.
        ///
        /// It checks that the _origin_ for this call is _Signed_ and returns a dispatch
        /// error if it isn't. Learn more about origins here: <https://docs.substrate.io/build/origins/>
        #[pallet::call_index(0)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::do_something())]
        pub fn do_something(origin: OriginFor<T>, something: u32) -> DispatchResult {
            // Check that the extrinsic was signed and get the signer.
            let who = ensure_signed(origin)?;

            // Update storage.
            Something::<T>::put(something);

            // Emit an event.
            Self::deposit_event(Event::SomethingStored { something, who });

            // Return a successful `DispatchResult`
            Ok(())
        }

        /// An example dispatchable that may throw a custom error.
        ///
        /// It checks that the caller is a signed origin and reads the current value from the
        /// `Something` storage item. If a current value exists, it is incremented by 1 and then
        /// written back to storage.
        ///
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If no value has been set ([`Error::NoneValue`])
        /// - If incrementing the value in storage causes an arithmetic overflow
        ///   ([`Error::StorageOverflow`])
        #[pallet::call_index(1)]
        #[pallet::weight(<T as pallet::Config>::WeightInfo::cause_error())]
        pub fn cause_error(origin: OriginFor<T>) -> DispatchResult {
            let _who = ensure_signed(origin)?;

            // Read a value from storage.
            match Something::<T>::get() {
                // Return an error if the value has not been set.
                None => Err(Error::<T>::NoneValue.into()),
                Some(old) => {
                    // Increment the value read from storage. This will cause an error in the event
                    // of overflow.
                    let new = old.checked_add(1).ok_or(Error::<T>::StorageOverflow)?;
                    // Update the value in storage with the incremented result.
                    Something::<T>::put(new);
                    Ok(())
                }
            }
        }

        /// Submits a post to the chain for voting.
        /// If the post is ultimately voted as bullish, they will get 2x their bond.
        /// If it is voted as bearish, they lose their bond.
        ///
        /// It checks that the post has not already been submitted in the past,
        /// and that the submitter has enough free tokens to bond.
        ///
        /// The post is then stored with who submitted it, and their tokens are bonded.
        ///
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If the post has been submitted previously
        /// - If the submitter does not have sufficient free tokens to bond
        #[pallet::call_index(2)]
        #[pallet::weight(Weight::default())]
        pub fn submit_post(
            origin: OriginFor<T>,
            post: T::Post,
            bond: BalanceOf<T>,
            voting_period: BlockNumberFor<T>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Checks if the post exists
            if Posts::<T>::contains_key(post.clone()) {
                return Err(Error::<T>::PostAlreadyExists.into())
            }

            // Checks if they have enough balance available to be bonded
            let reduc_bal = <<T as Config>::NativeBalance>::
            reducible_balance(&who, Preservation::Preserve, Fortitude::Polite);
            reduc_bal.checked_sub(&bond).ok_or(Error::<T>::InsufficientBalance)?;

            // Bonds the balance
            T::NativeBalance::hold(&HoldReason::PostBond.into(), &who, bond)?;

            let voting_until = frame_system::Pallet::<T>::block_number() + voting_period;

            // Stores the submitter and bond info
            Posts::<T>::insert(post.clone(), Post {
                submitter: who.clone(),
                bond,
                votes: 0,
                voting_until,
            });

            // Emit an event.
            Self::deposit_event(Event::PostSubmitted { post, submitter: who, bond, voting_until });

            Ok(())
        }

        /// Submits a vote on whether a particular post is bullish or bearish.
        ///
        /// ## Errors
        ///
        /// The function will return an error under the following conditions:
        ///
        /// - If that post does not exist
        /// - If the user tries to vote with more than their balance
        /// - If the voting period has already closed
        #[pallet::call_index(3)]
        #[pallet::weight(Weight::default())]
        pub fn submit_vote(
            origin: OriginFor<T>,
            post: T::Post,
            vote_amount: BalanceOf<T>,
            direction: Direction,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;

            // Error if the post does not exist.
            if !Posts::<T>::contains_key(post.clone()) {
                return Err(Error::<T>::PostDoesNotExist.into())
            }

            // Check if voting is still open for that post
            let post_struct = Posts::<T>::get(post.clone()).expect("Already checked that it exists");
            // If current block number is higher than the ending period of the post's voting, error.
            if frame_system::Pallet::<T>::block_number() > post_struct.voting_until {
                return Err(Error::<T>::VoteAlreadyClosed.into())
            }

            // Error if they do not have enough balance for the freeze
            if vote_amount > <<T as Config>::NativeBalance>::total_balance(&who) {
                return Err(Error::<T>::InsufficientBalance.into())
            };

            // extend_freeze
            <<T as Config>::NativeBalance>::extend_freeze(&FreezeReason::Vote.into(), &who, vote_amount)?;

            // Stores info

            // Emit an event.

            Ok(())
        }
    }
}
