#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    pallet_prelude::*,
    traits::{Get, Time},
};
use frame_system::pallet_prelude::*;
use sp_runtime::{
    traits::Saturating,
    transaction_validity::{
        InvalidTransaction, TransactionPriority, TransactionSource, TransactionValidity,
        ValidTransaction,
    },
};

mod mock;
mod tests;

use pallet::*;

pub trait OracleFeed<Moment> {
    /// Gets most recent feeded value, returns none if no feed exists
    fn get_most_recent_feed() -> Option<Vec<u8>>;

    /// Gets most feeded value at time, returns none if no value exists
    fn get_feed_at_time(time: Moment) -> Option<Vec<u8>>;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    pub(crate) type MomentOf<T> = <<T as Config>::Time as Time>::Moment;
    pub(crate) type OracleEvent<T> = BoundedVec<u8, <T as Config>::MaxEventSize>;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Overarching Event type
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

        /// Account that is allowed to feed values
        #[pallet::constant]
        type OperatorAccount: Get<Self::AccountId>;

        /// Maximum number of arguements in a side effect (should be 5 for this particular exercise)
        #[pallet::constant]
        type MaxEventSize: Get<u32>;

        /// Provides the current time
        type Time: Time;

        /// Amount of time until feeded values are stale and can be removed from storage
        #[pallet::constant]
        type StaleTime: Get<MomentOf<Self>>;

        /// A configuration for base priority of unsigned transactions.
        ///
        /// This is exposed so that it can be tuned for particular runtime, when
        /// multiple modules send unsigned transactions.
        #[pallet::constant]
        type UnsignedPriority: Get<TransactionPriority>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::error]
    pub enum Error<T> {
        NotOperatorAccount,
    }

    /// Time that most recent feeded value was added to storage
    #[pallet::storage]
    pub type MostRecentTime<T: Config> = StorageValue<_, MomentOf<T>, ValueQuery>;

    /// Maps time of feeded value to the bytes that represent the data
    #[pallet::storage]
    pub type OracleEvents<T: Config> =
        StorageMap<_, Twox64Concat, MomentOf<T>, OracleEvent<T>, OptionQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Event is successfully feeded
        EventFeeded { time: MomentOf<T> },
        /// Event is removed from storage due to being stale
        EventRemoved { time: MomentOf<T> },
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Feeds arbitrary data onchain, only authorized account can feed
        #[pallet::weight(0)]
        pub fn feed_event(origin: OriginFor<T>, event: OracleEvent<T>) -> DispatchResult {
            let feeder = ensure_signed(origin)?;
            ensure!(
                feeder == T::OperatorAccount::get(),
                Error::<T>::NotOperatorAccount
            );
            let now = T::Time::now();

            MostRecentTime::<T>::put(now);
            OracleEvents::<T>::insert(now, event);
            Self::deposit_event(Event::<T>::EventFeeded { time: now });
            Ok(())
        }

        /// Removes feeded events from storage, this is an unsigned transaction meaning anyone can submit it.
        /// Only valid if transaction is indeed stale
        #[pallet::weight(0)]
        pub fn remove_stale_event(origin: OriginFor<T>, time: MomentOf<T>) -> DispatchResult {
            ensure_none(origin)?;

            OracleEvents::<T>::remove(time);
            Self::deposit_event(Event::<T>::EventRemoved { time });
            Ok(())
        }
    }

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;

        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            match call {
                Call::remove_stale_event { time } => {
                    let now = T::Time::now();
                    if &now.saturating_sub(T::StaleTime::get()) > time
                        && OracleEvents::<T>::contains_key(time)
                    {
                        ValidTransaction::with_tag_prefix("Oracle")
                            .priority(T::UnsignedPriority::get())
                            .and_provides((<frame_system::Pallet<T>>::block_number(), time))
                            .longevity(64_u64)
                            .propagate(true)
                            .build()
                    } else {
                        InvalidTransaction::Stale.into()
                    }
                }
                _ => InvalidTransaction::Call.into(),
            }
        }
    }
}

impl<T: Config> OracleFeed<MomentOf<T>> for Pallet<T> {
    fn get_most_recent_feed() -> Option<Vec<u8>> {
        let fresh_time = MostRecentTime::<T>::get();
        OracleEvents::<T>::get(fresh_time).map(|x| x.into())
    }

    fn get_feed_at_time(time: MomentOf<T>) -> Option<Vec<u8>> {
        OracleEvents::<T>::get(time).map(|x| x.into())
    }
}
