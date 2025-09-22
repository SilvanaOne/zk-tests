#[test_only]
module sum::state_tests;

use sui::test_scenario as ts;
use sum::main::{
    create_state_return_id_for_test,
    // add_to_state,
    get_state,
    State
};

#[test]
fun test_state_initial_sum() {
    let mut scenario = ts::begin(@0xa);
    let ctx = ts::ctx(&mut scenario);

    let _ = create_state_return_id_for_test(ctx);
    ts::next_tx(&mut scenario, @0xa);

    let state = ts::take_shared<State>(&scenario);
    assert!(get_state(&state) == 0, 0);
    ts::return_shared(state);

    ts::end(scenario);
}

// #[test]
// fun test_state_add_to_state() {
//     let mut scenario = ts::begin(@0xb);
//     let ctx = ts::ctx(&mut scenario);

//     let _ = create_state_return_id_for_test(ctx);
//     ts::next_tx(&mut scenario, @0xb);

//     // First mutation
//     let mut state1 = ts::take_shared<State>(&scenario);
//     add_to_state(&mut state1, 5);
//     assert!(get_state(&state1) == 5, 1);
//     ts::return_shared(state1);

//     ts::next_tx(&mut scenario, @0xb);

//     // Second mutation
//     let mut state2 = ts::take_shared<State>(&scenario);
//     add_to_state(&mut state2, 7);
//     assert!(get_state(&state2) == 12, 2);
//     ts::return_shared(state2);

//     ts::end(scenario);
// }
