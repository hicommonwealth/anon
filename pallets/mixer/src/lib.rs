// A runtime module Groups with necessary imports

// Feel free to remove or edit this file as needed.
// If you change the name of this file, make sure to update its references in
// runtime/src/lib.rs If you remove this file, you can remove those references

// For more guidance on Substrate modules, see the example module
// https://github.com/paritytech/substrate/blob/master/frame/example/src/lib.rs

//! # Mixer Pallet
//!
//! The Mixer pallet provides functionality for doing deposits and withdrawals
//! from the mixer.
//!
//! - [`Config`]
//! - [`Call`]
//! - [`Pallet`]
//!
//! ## Overview
//!
//! The Mixer pallet provides functions for:
//!
//! - Depositing some currency into the mixer.
//! - Withdrawing the deposit from the mixer.
//! - Stopping mixer operations.
//! - Transfering the admin of the mixer.
//!
//! ### Terminology
//!
//! - **Mixer**: Cryptocurrency tumbler or mixer is a service offered to mix
//!   potentially identifiable or 'tainted' cryptocurrency funds with others, so
//!   as to obscure the trail back to the fund's original source.
//!
//! ## Interface
//!
//! ### Dispatchable Functions
//!
//! - `deposit` - Deposit a fixed amount of cryptocurrency into the mixer.
//! - `withdraw` - Provide a zero-knowladge proof of the deposit and withdraw
//!   from the mixer.
//! - `set_stopped` - Stops the operation of all mixers.
//! - `transfer_admin` - Transfers the admin role from sender to specified
//!   account.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
pub mod mock;

#[cfg(test)]
pub mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;

use codec::{Decode, Encode};
use frame_support::{
	debug, dispatch, ensure,
	traits::{Currency, ExistenceRequirement::AllowDeath, Get},
	weights::Weight,
};
use frame_system::ensure_signed;
use merkle::{
	utils::{
		keys::{Commitment, ScalarData},
		permissions::ensure_admin,
	},
	Group as GroupTrait, Module as MerkleModule,
};
pub use pallet::*;
use sp_runtime::{
	traits::{AccountIdConversion, Zero},
	ModuleId,
};
use sp_std::prelude::*;
use weights::WeightInfo;

pub use pallet::*;

/// Implementation of Mixer pallet
#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The pallet's configuration trait.
	#[pallet::config]
	pub trait Config: frame_system::Config + merkle::Config {
		#[pallet::constant]
		type ModuleId: Get<ModuleId>;
		/// The overarching event type.
		type Event: IsType<<Self as frame_system::Config>::Event> + From<Event<Self>>;
		/// Currency type for taking deposits
		type Currency: Currency<Self::AccountId>;
		/// The overarching group trait
		type Group: GroupTrait<Self::AccountId, Self::BlockNumber, Self::GroupId>;
		/// The max depth of the mixers
		#[pallet::constant]
		type MaxMixerTreeDepth: Get<u8>;
		/// The small deposit length
		#[pallet::constant]
		type DepositLength: Get<Self::BlockNumber>;
		/// Default admin key
		#[pallet::constant]
		type DefaultAdmin: Get<Self::AccountId>;
		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
		// Available mixes sizes (Size is determend by the deposit amount)
		type MixerSizes: Get<Vec<BalanceOf<Self>>>;
	}

	/// Flag indicating if the mixer is initialized.
	#[pallet::storage]
	#[pallet::getter(fn initialised)]
	pub type Initialised<T: Config> = StorageValue<_, bool, ValueQuery>;

	/// The map of mixer groups to their metadata
	#[pallet::storage]
	#[pallet::getter(fn mixer_groups)]
	pub type MixerGroups<T: Config> = StorageMap<_, Blake2_128Concat, T::GroupId, MixerInfo<T>, ValueQuery>;

	/// The vec of group ids
	#[pallet::storage]
	#[pallet::getter(fn mixer_group_ids)]
	pub type MixerGroupIds<T: Config> = StorageValue<_, Vec<T::GroupId>, ValueQuery>;

	/// Administrator of the mixer pallet.
	/// This account that can stop/start operations of the mixer
	#[pallet::storage]
	#[pallet::getter(fn admin)]
	pub type Admin<T: Config> = StorageValue<_, T::AccountId, ValueQuery>;

	/// The TVL per group
	#[pallet::storage]
	#[pallet::getter(fn total_value_locked)]
	pub type TotalValueLocked<T: Config> = StorageMap<_, Blake2_128Concat, T::GroupId, BalanceOf<T>, ValueQuery>;

	// /// Old name generated by `decl_event`.
	// #[deprecated(note = "use `Event` instead")]
	// pub type RawEvent<T: Config> = Event<T>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	#[pallet::metadata(<T as frame_system::Config>::AccountId = "AccountId", <T as merkle::Config>::GroupId = "GroupId")]
	pub enum Event<T: Config> {
		/// New deposit added to the specific mixer
		Deposit(
			<T as merkle::Config>::GroupId,
			<T as frame_system::Config>::AccountId,
			ScalarData,
		),
		/// Withdrawal from the specific mixer
		Withdraw(
			<T as merkle::Config>::GroupId,
			<T as frame_system::Config>::AccountId,
			ScalarData,
		),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Value was None
		NoneValue,
		/// Mixer not found for specified id
		NoMixerForId,
		/// Mixer is not initialized
		NotInitialised,
		/// Mixer is already initialized
		AlreadyInitialised,
		/// User doesn't have enough balance for the deposit
		InsufficientBalance,
		/// Caller doesn't have permission to make a call
		UnauthorizedCall,
		/// Mixer is stopped
		MixerStopped,
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			// We make sure that wee return correct weight for the block accourding to
			// on_finalize
			if Self::initialised() {
				// In case mixer is initialized, we expect the weights for merkle cache update
				<T as Config>::WeightInfo::on_finalize_initialized()
			} else {
				// In case mixer is not initialized, we expect the weights for initialization
				<T as Config>::WeightInfo::on_finalize_uninitialized()
			}
		}

		fn on_finalize(_n: BlockNumberFor<T>) {
			if Self::initialised() {
				// check if any deposits happened (by checked the size of collection at this
				// block) if none happened, carry over previous merkle roots for the cache.
				let mixer_ids = MixerGroupIds::<T>::get();
				for i in 0..mixer_ids.len() {
					let cached_roots = <merkle::Module<T>>::cached_roots(_n, mixer_ids[i]);
					// if there are no cached roots, carry forward the current root
					if cached_roots.len() == 0 {
						let _ = <merkle::Module<T>>::add_root_to_cache(mixer_ids[i], _n);
					}
				}
			} else {
				match Self::initialize() {
					Ok(_) => {}
					Err(e) => {
						debug::native::error!("Error initialising: {:?}", e);
					}
				}
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Deposits the fixed amount into the mixer with id of `mixer_id`
		/// Multiple deposits can be inserted together since `data_points` is an
		/// array.
		///
		/// Fails in case the mixer is stopped or not initialized.
		///
		/// Weights:
		/// - Dependent on argument: `data_points`
		///
		/// - Base weight: 417_168_400_000
		/// - DB weights: 8 reads, 5 writes
		/// - Additional weights: 21_400_442_000 * data_points.len()
		#[pallet::weight(<T as Config>::WeightInfo::deposit(data_points.len() as u32))]
		pub fn deposit(
			origin: OriginFor<T>,
			mixer_id: T::GroupId,
			data_points: Vec<ScalarData>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(Self::initialised(), Error::<T>::NotInitialised);
			ensure!(!<MerkleModule<T>>::stopped(mixer_id), Error::<T>::MixerStopped);
			// get mixer info, should always exist if module is initialised
			let mut mixer_info = Self::get_mixer(mixer_id)?;
			// ensure the sender has enough balance to cover deposit
			let balance = T::Currency::free_balance(&sender);
			// TODO: Multiplication by usize should be possible
			// using this hack for now, though we should optimise with regular
			// multiplication `data_points.len() * mixer_info.fixed_deposit_size`
			let deposit: BalanceOf<T> = data_points
				.iter()
				.map(|_| mixer_info.fixed_deposit_size)
				.fold(Zero::zero(), |acc, elt| acc + elt);
			ensure!(balance >= deposit, Error::<T>::InsufficientBalance);
			// transfer the deposit to the module
			T::Currency::transfer(&sender, &Self::account_id(), deposit, AllowDeath)?;
			// update the total value locked
			let tvl = Self::total_value_locked(mixer_id);
			<TotalValueLocked<T>>::insert(mixer_id, tvl + deposit);
			// add elements to the mixer group's merkle tree and save the leaves
			T::Group::add_members(Self::account_id(), mixer_id.into(), data_points.clone())?;
			mixer_info.leaves.extend(data_points);
			MixerGroups::<T>::insert(mixer_id, mixer_info);

			Ok(().into())
		}

		/// Withdraws a deposited amount from the mixer. Can only withdraw one
		/// deposit. Accepts a proof of membership along with the mixer id.
		///
		/// Fails if the mixer is stopped or not initialized.
		///
		/// Weights:
		/// - Independent of the arguments.
		///
		/// - Base weight: 1_078_562_000_000
		/// - DB weights: 9 reads, 3 writes
		#[pallet::weight(<T as Config>::WeightInfo::withdraw())]
		pub fn withdraw(
			origin: OriginFor<T>,
			mixer_id: T::GroupId,
			cached_block: T::BlockNumber,
			cached_root: ScalarData,
			comms: Vec<Commitment>,
			nullifier_hash: ScalarData,
			proof_bytes: Vec<u8>,
			leaf_index_commitments: Vec<Commitment>,
			proof_commitments: Vec<Commitment>,
		) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;
			ensure!(Self::initialised(), Error::<T>::NotInitialised);
			ensure!(!<MerkleModule<T>>::stopped(mixer_id), Error::<T>::MixerStopped);
			let mixer_info = MixerGroups::<T>::get(mixer_id);
			// check if the nullifier has been used
			T::Group::has_used_nullifier(mixer_id.into(), nullifier_hash)?;
			// Verify the zero-knowledge proof of membership provided
			T::Group::verify_zk_membership_proof(
				mixer_id.into(),
				cached_block,
				cached_root,
				comms,
				nullifier_hash,
				proof_bytes,
				leaf_index_commitments,
				proof_commitments,
			)?;
			// transfer the fixed deposit size to the sender
			T::Currency::transfer(&Self::account_id(), &sender, mixer_info.fixed_deposit_size, AllowDeath)?;
			// update the total value locked
			let tvl = Self::total_value_locked(mixer_id);
			<TotalValueLocked<T>>::insert(mixer_id, tvl - mixer_info.fixed_deposit_size);
			// Add the nullifier on behalf of the module
			T::Group::add_nullifier(Self::account_id(), mixer_id.into(), nullifier_hash)?;
			Ok(().into())
		}

		/// Stops the operation of all the mixers managed by the pallet.
		/// Can only be called by the admin or the root origin.
		///
		/// Weights:
		/// - Independent of the arguments.
		///
		/// - Base weight: 36_000_000
		/// - DB weights: 6 reads, 4 writes
		#[pallet::weight(<T as Config>::WeightInfo::set_stopped())]
		pub fn set_stopped(origin: OriginFor<T>, stopped: bool) -> DispatchResultWithPostInfo {
			// Ensure the caller is admin or root
			ensure_admin(origin, &Self::admin())?;
			// Set the mixer state, `stopped` can be true or false
			let mixer_ids = MixerGroupIds::<T>::get();
			for i in 0..mixer_ids.len() {
				T::Group::set_stopped(Self::account_id(), mixer_ids[i], stopped)?;
			}
			Ok(().into())
		}

		/// Transfers the admin from the caller to the specified `to` account.
		/// Can only be called by the current admin or the root origin.
		///
		/// Weights:
		/// - Independent of the arguments.
		///
		/// - Base weight: 7_000_000
		/// - DB weights: 1 read, 1 write
		#[pallet::weight(<T as Config>::WeightInfo::transfer_admin())]
		pub fn transfer_admin(origin: OriginFor<T>, to: T::AccountId) -> DispatchResultWithPostInfo {
			// Ensures that the caller is the root or the current admin
			ensure_admin(origin, &Self::admin())?;
			// Updating the admin
			Admin::<T>::set(to);
			Ok(().into())
		}
	}
}

/// Type alias for the balances_pallet::Balance type
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Info about the mixer and it's leaf data
#[derive(Encode, Decode, PartialEq)]
pub struct MixerInfo<T: Config> {
	pub minimum_deposit_length_for_reward: T::BlockNumber,
	pub fixed_deposit_size: BalanceOf<T>,
	pub leaves: Vec<ScalarData>,
}

impl<T: Config> core::default::Default for MixerInfo<T> {
	fn default() -> Self {
		Self {
			/// Minimum duration the deposit has stayed in the mixer for user to
			/// be eligable for reward
			///
			/// NOTE: Currently not used
			minimum_deposit_length_for_reward: Zero::zero(),
			/// Deposit size for the mixer
			fixed_deposit_size: Zero::zero(),
			/// All the leaves
			leaves: Vec::new(),
		}
	}
}

impl<T: Config> MixerInfo<T> {
	pub fn new(min_dep_length: T::BlockNumber, dep_size: BalanceOf<T>, leaves: Vec<ScalarData>) -> Self {
		Self {
			minimum_deposit_length_for_reward: min_dep_length,
			fixed_deposit_size: dep_size,
			leaves,
		}
	}
}

impl<T: Config> Module<T> {
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	pub fn get_mixer(mixer_id: T::GroupId) -> Result<MixerInfo<T>, dispatch::DispatchError> {
		let mixer_info = MixerGroups::<T>::get(mixer_id);
		// ensure mixer_info has non-zero deposit, otherwise mixer doesn't
		// really exist for this id
		ensure!(mixer_info.fixed_deposit_size > Zero::zero(), Error::<T>::NoMixerForId); // return the mixer info
		Ok(mixer_info)
	}

	pub fn initialize() -> dispatch::DispatchResult {
		ensure!(!Self::initialised(), Error::<T>::AlreadyInitialised);

		// Get default admin from trait params
		let default_admin = T::DefaultAdmin::get();
		// Initialize the admin in storage with default one
		Admin::<T>::set(default_admin);
		let depth: u8 = <T as Config>::MaxMixerTreeDepth::get();

		// Getting the sizes from the config
		let sizes = T::MixerSizes::get();
		let mut mixer_ids = Vec::new();

		// Iterating over configured sizes and initializing the mixers
		for size in sizes.into_iter() {
			// Creating a new merkle group and getting the id back
			let mixer_id: T::GroupId = T::Group::create_group(Self::account_id(), true, depth)?;
			// Creating mixer info data
			let mixer_info = MixerInfo::<T> {
				fixed_deposit_size: size,
				minimum_deposit_length_for_reward: T::DepositLength::get(),
				leaves: Vec::new(),
			};
			// Saving the mixer group to storage
			MixerGroups::<T>::insert(mixer_id, mixer_info);
			mixer_ids.push(mixer_id);
		}

		// Setting the mixer ids
		MixerGroupIds::<T>::set(mixer_ids);

		Initialised::<T>::set(true);
		Ok(())
	}
}
