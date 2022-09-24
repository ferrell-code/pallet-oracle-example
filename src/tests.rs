#![cfg(test)]

use super::{
    Call, Error, Event, InvalidTransaction, OracleFeed, TransactionSource, ValidateUnsigned,
};
use crate::mock::*;
use frame_support::{assert_err, assert_noop, assert_ok, bounded_vec};

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 1000;

fn run_to_block(n: u64) {
    while System::block_number() < n {
        System::set_block_number(System::block_number() + 1);
        Timestamp::set_timestamp((System::block_number() as u64 * BLOCK_TIME) + INIT_TIMESTAMP);
    }
}

#[test]
fn feed_call_works() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Oracle::feed_event(Origin::signed(BOB), bounded_vec![1, 1]),
            Error::<Runtime>::NotOperatorAccount
        );
        assert_eq!(Oracle::get_most_recent_feed(), None);

        assert_ok!(Oracle::feed_event(
            Origin::signed(ALICE),
            bounded_vec![1, 1]
        ));
        assert_eq!(Oracle::get_most_recent_feed().unwrap(), vec![1, 1]);
        System::assert_last_event(Event::<Runtime>::EventFeeded { time: 0 }.into());

        run_to_block(5);
        assert_ok!(Oracle::feed_event(
            Origin::signed(ALICE),
            bounded_vec![2, 2]
        ));
        assert_eq!(Oracle::get_most_recent_feed().unwrap(), vec![2, 2]);
        System::assert_last_event(Event::<Runtime>::EventFeeded { time: 35_000 }.into());
    });
}

#[test]
fn remove_call_validation_works() {
    new_test_ext().execute_with(|| {
        run_to_block(2);
        assert_ok!(Oracle::feed_event(
            Origin::signed(ALICE),
            bounded_vec![1, 1]
        ));
        System::assert_last_event(Event::<Runtime>::EventFeeded { time: 32_000 }.into());
        run_to_block(5);

        // Is not stale yet, so validation fails
        let call = Call::remove_stale_event { time: 32_000 };
        // invalid feed
        let call2 = Call::remove_stale_event { time: 32_001 };

        assert_err!(
            <Oracle as ValidateUnsigned>::validate_unsigned(TransactionSource::External, &call),
            InvalidTransaction::Stale
        );
        assert_eq!(Oracle::get_feed_at_time(32_000).unwrap(), vec![1, 1]);
        run_to_block(13);

        assert_err!(
            <Oracle as ValidateUnsigned>::validate_unsigned(TransactionSource::External, &call2),
            InvalidTransaction::Stale
        );
        assert_ok!(<Oracle as ValidateUnsigned>::validate_unsigned(
            TransactionSource::External,
            &call
        ));
        assert_ok!(Oracle::remove_stale_event(Origin::none(), 32_000));

        assert_eq!(Oracle::get_feed_at_time(31_000), None);
        System::assert_last_event(Event::<Runtime>::EventRemoved { time: 32_000 }.into());
    });
}
