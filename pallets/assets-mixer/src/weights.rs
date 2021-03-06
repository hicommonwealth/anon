//! Autogenerated weights for pallet_mixer
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 3.0.0
//! DATE: 2021-02-17, STEPS: [20, ], REPEAT: 5, LOW RANGE: [], HIGH RANGE: []
//! EXECUTION: None, WASM-EXECUTION: Interpreted, CHAIN: Some("dev"), DB CACHE:
//! 128

// Executed Command:
// ./target/release/node-template
// benchmark
// --chain
// dev
// --pallet
// pallet_mixer
// --extrinsic
// *
// --steps
// 20
// --repeat
// 5
// --output
// ./pallets/mixer/src/

#![allow(unused_parens)]
#![allow(unused_imports)]

use crate::Config;
use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_mixer.
pub trait WeightInfo {
	fn deposit(n: u32) -> Weight;
	fn withdraw() -> Weight;
	fn set_stopped() -> Weight;
	fn transfer_admin() -> Weight;
	fn on_finalize_uninitialized() -> Weight;
	fn on_finalize_initialized() -> Weight;
}

/// Weight functions for pallet_mixer.
/// For use in production
pub struct Weights<T>(PhantomData<T>);
impl<T: frame_system::Config + Config + merkle::Config> WeightInfo for Weights<T> {
	fn deposit(d: u32) -> Weight {
		(417_168_400_000 as Weight)
			// Standard Error: 241_824_000
			.saturating_add((21_400_442_000 as Weight).saturating_mul(d as Weight))
			.saturating_add(T::DbWeight::get().reads(8 as Weight))
			.saturating_add(T::DbWeight::get().writes(5 as Weight))
	}

	fn withdraw() -> Weight {
		(1_078_562_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(9 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}

	fn set_stopped() -> Weight {
		(36_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(6 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}

	fn transfer_admin() -> Weight {
		(7_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}

	fn on_finalize_uninitialized() -> Weight {
		(53_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().writes(16 as Weight))
	}

	fn on_finalize_initialized() -> Weight {
		(93_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(10 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
}
