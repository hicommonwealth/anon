use crate::mock::*;
use bulletproofs::{r1cs::Prover, BulletproofGens, PedersenGens};
use curve25519_gadgets::{
	fixed_deposit_tree::builder::FixedDepositTreeBuilder,
	poseidon::{
		builder::{Poseidon, PoseidonBuilder},
		gen_mds_matrix, gen_round_keys, PoseidonSbox,
	},
};
use frame_support::{assert_err, assert_ok, storage::StorageValue, traits::OnFinalize};
use merkle::merkle::keys::{Commitment, Data};
use merlin::Transcript;
use sp_runtime::DispatchError;

fn default_hasher(num_gens: usize) -> Poseidon {
	let width = 6;
	let (full_b, full_e) = (4, 4);
	let partial_rounds = 57;
	PoseidonBuilder::new(width)
		.num_rounds(full_b, full_e, partial_rounds)
		.round_keys(gen_round_keys(width, full_b + full_e + partial_rounds))
		.mds_matrix(gen_mds_matrix(width))
		.bulletproof_gens(BulletproofGens::new(num_gens, 1))
		.sbox(PoseidonSbox::Inverse)
		.build()
}

#[test]
fn should_initialize_successfully() {
	new_test_ext().execute_with(|| {
		assert_ok!(Mixer::initialize(Origin::signed(1)));
		// the mixer creates 4 groups, they should all initialise to 0
		let val = 1_000;
		for i in 0..4 {
			let g = MerkleGroups::get_group(i).unwrap();
			let m = Mixer::get_mixer(i).unwrap();
			assert_eq!(g.leaf_count, 0);
			assert_eq!(g.manager_required, true);
			assert_eq!(m.leaves.len(), 0);
			assert_eq!(m.fixed_deposit_size, val * 10_u64.pow(i))
		}
	})
}

#[test]
fn should_fail_to_deposit_with_insufficient_balance() {
	new_test_ext().execute_with(|| {
		assert_ok!(Mixer::initialize(Origin::signed(1)));
		let mut tree = FixedDepositTreeBuilder::new().build();
		for i in 0..4 {
			let leaf = tree.generate_secrets();
			assert_err!(
				Mixer::deposit(Origin::signed(4), i, vec![Data(leaf)]),
				DispatchError::Module {
					index: 0,
					error: 4,
					message: Some("InsufficientBalance")
				}
			);
		}
	})
}

#[test]
fn should_deposit_into_each_mixer_successfully() {
	new_test_ext().execute_with(|| {
		assert_ok!(Mixer::initialize(Origin::signed(1)));
		let mut tree = FixedDepositTreeBuilder::new().build();
		for i in 0..4 {
			let leaf = tree.generate_secrets();
			let balance_before = Balances::free_balance(1);
			assert_ok!(Mixer::deposit(Origin::signed(1), i, vec![Data(leaf)]));
			let balance_after = Balances::free_balance(1);

			// ensure state updates
			let g = MerkleGroups::get_group(i).unwrap();
			let m = Mixer::get_mixer(i).unwrap();
			assert_eq!(balance_before, balance_after + m.fixed_deposit_size);
			assert_eq!(g.leaf_count, 1);
			assert_eq!(m.leaves.len(), 1);
		}
	})
}

#[test]
fn should_withdraw_from_each_mixer_successfully() {
	new_test_ext().execute_with(|| {
		assert_ok!(Mixer::initialize(Origin::signed(1)));
		let pc_gens = PedersenGens::default();
		let poseidon = default_hasher(40960);

		for i in 0..4 {
			let mut prover_transcript = Transcript::new(b"zk_membership_proof");
			let prover = Prover::new(&pc_gens, &mut prover_transcript);
			let mut ftree = FixedDepositTreeBuilder::new()
				.hash_params(poseidon.clone())
				.depth(32)
				.build();

			let leaf = ftree.generate_secrets();
			ftree.tree.add_leaves(vec![leaf.to_bytes()]);

			assert_ok!(Mixer::deposit(Origin::signed(1), i, vec![Data(leaf)]));
			assert_ok!(MerkleGroups::update_cached_state(Origin::signed(1), i));

			let state = MerkleGroups::cached_state(i).unwrap();
			let root = state.root_hash;
			let (proof, (comms_cr, nullifier_hash, leaf_index_comms_cr, proof_comms_cr)) =
				ftree.prove_zk(root.0, leaf, &ftree.hash_params.bp_gens, prover);

			let comms: Vec<Commitment> = comms_cr.iter().map(|x| Commitment(*x)).collect();
			let leaf_index_comms: Vec<Commitment> = leaf_index_comms_cr.iter().map(|x| Commitment(*x)).collect();
			let proof_comms: Vec<Commitment> = proof_comms_cr.iter().map(|x| Commitment(*x)).collect();

			let m = Mixer::get_mixer(i).unwrap();
			let balance_before = Balances::free_balance(2);
			// withdraw from another account
			assert_ok!(Mixer::withdraw(
				Origin::signed(2),
				i,
				comms,
				Data(nullifier_hash),
				proof.to_bytes(),
				leaf_index_comms,
				proof_comms
			));
			let balance_after = Balances::free_balance(2);
			assert_eq!(balance_before + m.fixed_deposit_size, balance_after);
		}
	})
}
