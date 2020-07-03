use std::cmp;
use crate::math::{ field, polynom };
use crate::utils::{ hasher };
use crate::{ HASH_STATE_WIDTH, HASH_CYCLE_LENGTH };
use super::{ NUM_AUX_CONSTRAINTS };

// TYPES AND INTERFACES
// ================================================================================================
pub struct HashEvaluator {
    trace_length    : usize,
    cycle_length    : usize,
    ark_values      : Vec<[u128; 2 * HASH_STATE_WIDTH]>,
    ark_polys       : Vec<Vec<u128>>,
}

// HASH EVALUATOR IMPLEMENTATION
// ================================================================================================
impl HashEvaluator {
    /// Creates a new HashEvaluator based on the provided `trace_length` and `extension_factor`.
    pub fn new(trace_length: usize, extension_factor: usize) -> HashEvaluator {
        // extend rounds constants by the specified extension factor
        let (ark_polys, ark_evaluations) = hasher::get_extended_constants(extension_factor);

        // transpose round constant evaluations so that constants for each round
        // are stored in a single row
        let cycle_length = HASH_CYCLE_LENGTH * extension_factor;
        let mut ark_values = Vec::with_capacity(cycle_length);
        for i in 0..cycle_length {
            ark_values.push([field::ZERO; 2 * HASH_STATE_WIDTH]);
            for j in 0..(2 * HASH_STATE_WIDTH) {
                ark_values[i][j] = ark_evaluations[j][i];
            }
        }

        return HashEvaluator { trace_length, cycle_length, ark_values, ark_polys };
    }

    /// Evaluates constraints at the specified step and adds the resulting values to `result`.
    pub fn evaluate(&self, current: &[u128], next: &[u128], step: usize, op_flag: u128, result: &mut [u128]) {
        let step = step % self.cycle_length;

        // determine round constants for the current step
        let ark = &self.ark_values[step];

        // evaluate constraints for the hash function and for the rest of the stack
        self.eval_hash(current, next, ark, op_flag, &mut result[NUM_AUX_CONSTRAINTS..]);
        self.eval_rest(current, next, op_flag, &mut result[NUM_AUX_CONSTRAINTS..]);
    }

    /// Evaluates constraints at the specified x coordinate and adds the resulting values to `result`.
    /// Unlike the function above, this function can evaluate constraints for any out-of-domain 
    /// coordinate, but is significantly slower.
    pub fn evaluate_at(&self, current: &[u128], next: &[u128], x: u128, op_flag: u128, result: &mut [u128]) {

        // determine mask and round constants at the specified x coordinate
        let num_cycles =(self.trace_length / HASH_CYCLE_LENGTH) as u128;
        let x = field::exp(x, num_cycles);
        let mut ark = [field::ZERO; 2 * HASH_STATE_WIDTH];
        for i in 0..ark.len() {
            ark[i] = polynom::eval(&self.ark_polys[i], x);
        }

        // evaluate constraints for the hash function and for the rest of the stack
        self.eval_hash(current, next, &ark, op_flag, &mut result[NUM_AUX_CONSTRAINTS..]);
        self.eval_rest(current, next, op_flag, &mut result[NUM_AUX_CONSTRAINTS..]);
    }

    /// Evaluates constraints for a single round of a modified Rescue hash function. Hash state is
    /// assumed to be in the first 6 registers of user stack (aux registers are not affected).
    fn eval_hash(&self, current: &[u128], next: &[u128], ark: &[u128], op_flag: u128, result: &mut [u128]) {

        let mut state_part1 = [field::ZERO; HASH_STATE_WIDTH];
        state_part1.copy_from_slice(&current[..HASH_STATE_WIDTH]);
        let mut state_part2 = [field::ZERO; HASH_STATE_WIDTH];
        state_part2.copy_from_slice(&next[..HASH_STATE_WIDTH]);

        for i in 0..HASH_STATE_WIDTH {
            state_part1[i] = field::add(state_part1[i], ark[i]);
        }
        hasher::apply_sbox(&mut state_part1);
        hasher::apply_mds(&mut state_part1);
    
        hasher::apply_inv_mds(&mut state_part2);
        hasher::apply_sbox(&mut state_part2);
        for i in 0..HASH_STATE_WIDTH {
            state_part2[i] = field::sub(state_part2[i], ark[HASH_STATE_WIDTH + i]);
        }

        for i in 0..cmp::min(result.len(), HASH_STATE_WIDTH) {
            let evaluation = field::sub(state_part2[i], state_part1[i]);
            result[i] = field::add(result[i], field::mul(evaluation, op_flag));
        }
    }

    /// Evaluates constraints for stack registers un-affected by hash transition.
    fn eval_rest(&self, current: &[u128], next: &[u128], op_flag: u128, result: &mut [u128]) {
        for i in HASH_STATE_WIDTH..result.len() {
            let evaluation = field::sub(next[i], current[i]);
            result[i] = field::add(result[i], field::mul(evaluation, op_flag));
        }
    }
}